use std::fs::{exists, write};
use std::sync::Arc;
use anyhow::Result;
use crate::config::Config;
use crate::tx::{Song, TXPlayList};

mod config;
mod tx;

#[tokio::main]
async fn main() -> Result<()> {
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

    let mut set: tokio::task::JoinSet<Result<(String,String)>> = tokio::task::JoinSet::new();

    let lx_api_url = Arc::new(config.lx_api_url.clone());

    println!("{need_to_download:#?}");
    
    for i in need_to_download {
        let copy = lx_api_url.clone();
        let copy2 = config.lx_api_key.clone();
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

            Ok(
                (url, i.name)
            )
        });
    }

    let client = reqwest::ClientBuilder::new().build()?;
    while let Some(result) = set.join_next().await {
        let (url, name) = result??;

        let data = client.get(url).send().await?.bytes().await?;

        write(format!("{}/{}.mp3", config.dir, name.replace(" ", "_")), data)?
    }

    Ok(())
}
