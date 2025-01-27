use std::fs::{exists, write};
use std::process::exit;
use std::sync::Arc;
use anyhow::Result;
use tokio_cron_scheduler::{Job, JobScheduler};
use crate::config::Config;
use crate::tx::{Song, TXPlayList};

mod config;
mod tx;

#[tokio::main]
async fn main() -> Result<()> {
    let mut sched = JobScheduler::new().await?;

    sched.add(
        Job::new_async("every 5 minutes", |_uuid, _locked| {
            Box::pin(
                async move {
                    println!("start");
                    match run().await {
                        Ok(()) => (),
                        Err(error) => println!("Error: {error}"),
                    }
                }
            )
        })?
    ).await?;

    sched.shutdown_on_ctrl_c();

    sched.start().await?;

    match run().await {
        Ok(()) => (),
        Err(error) => println!("Error: {error}"),
    }
    
    ctrlc::set_handler(|| {
        exit(1)
    })?;

    loop {
        if let Some(time) = sched.time_till_next_job().await? {
            tokio::time::sleep(time).await
        }
    }
}

async fn run() -> Result<()> {
    let config = Config::get();
    let play_list = TXPlayList::new(config.play_id).await?;

    let mut need_to_download: Vec<Song> = vec![];

    for i in play_list.song_list {
        if exists(format!("{}/{}.mp3",config.dir, i.name.replace(" ", "_")))? {
            continue
        } else {
            need_to_download.push(i);
        }
    }

    println!("{need_to_download:#?}");

    let mut set: tokio::task::JoinSet<Result<()>> = tokio::task::JoinSet::new();

    let lx_api_url = Arc::new(config.lx_api_url.clone());

    for i in need_to_download {
        let copy = lx_api_url.clone();
        let copy2 = config.lx_api_key.clone();
        let dir = config.dir.clone();
        set.spawn(async move {
            let client = reqwest::ClientBuilder::new()
                .danger_accept_invalid_certs(true)
                .build()?;
            let data = client.get(
                format!("{}/url/tx/{}/320k", copy, i.id)
            )
                .header("X-Request-Key", copy2)
                .send()
                .await?;
            let json = serde_json::from_str::<serde_json::Value>(&data.text().await?)?;

            let url = json["url"].as_str().unwrap().to_string();

            let data = client.get(url).send().await?.bytes().await?;

            write(format!("{}/{}.mp3", dir, i.name.replace(" ", "_")), data)?;

            Ok(())
        });
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    }

    Ok(())
}