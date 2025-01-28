use std::fs::{exists, write};
use std::process::exit;
use std::sync::{Arc};
use anyhow::Result;
use tokio_cron_scheduler::{Job, JobScheduler};
use crate::config::Config;
use crate::tx::{Song, TXPlayList};

mod config;
mod tx;

#[tokio::main]
async fn main() -> Result<()> {
    let mut sched = JobScheduler::new().await?;

    let is_running = Arc::new(tokio::sync::Mutex::new(false));

    let cloned_running = is_running.clone();

    sched.add(
        Job::new_async("every 5 minutes", {
            move |_uuid, _locked| {
                let is_running = cloned_running.clone();
                Box::pin(async move {
                    if let Ok(mut running) = is_running.try_lock() {
                        *running = true;
                        println!("start");
                        match run().await {
                            Ok(()) => (),
                            Err(error) => println!("Error: {error}"),
                        }
                        *running = false;
                    } else {
                        println!("Previous job is still running, skipping this run.");
                    }
                })
            }
        })?
    ).await?;

    sched.add(
        Job::new_one_shot_async(
            std::time::Duration::from_secs(0),
            {
                move |_uuid, _locked| {
                    let is_running = is_running.clone();
                    Box::pin(async move {
                        if let Ok(mut running) = is_running.try_lock() {
                            *running = true;
                            println!("start");
                            match run().await {
                                Ok(()) => (),
                                Err(error) => println!("Error: {error}"),
                            }
                            *running = false;
                        } else {
                            print!("Previous job is still running, skipping this run.");
                        }
                    })
                }
        })?
    ).await?;

    sched.shutdown_on_ctrl_c();

    sched.start().await?;


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
    // Config
    let config = Config::get();
    // Play List
    let play_list = TXPlayList::new(config.play_id).await?;

    let mut need_to_download: Vec<Song> = vec![];

    for i in play_list.song_list {
        if exists(format!("{}/{}.mp3",config.dir, sanitise_file_name::sanitise(&i.name)))? {
            continue
        } else {
            need_to_download.push(i);
        }
    }

    // Init ProgressBar
    let progress_bar = Arc::new(indicatif::ProgressBar::new(need_to_download.len() as u64));
    progress_bar.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")?
            .progress_chars("█▇▆▅▄▃▂▁  ")
    );

    progress_bar.enable_steady_tick(std::time::Duration::from_millis(100));

    let mut set: tokio::task::JoinSet<Result<()>> = tokio::task::JoinSet::new();

    let lx_api_url = Arc::new(config.lx_api_url.clone());

    for i in need_to_download {
        let copy = lx_api_url.clone();
        let copy2 = config.lx_api_key.clone();
        let pb = progress_bar.clone();
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

            pb.set_message(sanitise_file_name::sanitise(&i.name));

            write(format!("{}/{}.mp3", dir, sanitise_file_name::sanitise(&i.name)), data)?;

            pb.inc(1);

            Ok(())
        });
        tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    }
    
    set.join_all().await;
    
    println!("finished");

    Ok(())
}