use std::fs::{exists, read_to_string, write};
use std::path::Path;
use std::process::exit;
use std::sync::{Arc};
use anyhow::Result;
use id3::TagLike;
use tokio_cron_scheduler::{Job, JobScheduler};
use crate::config::Config;
use crate::song::Song;
use crate::tx::{TXLyric, TXPlayList};

mod config;
mod tx;
mod song;

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
    
    // Read the ignore file if it exists
    let ignore_path = Path::new(".ignore");
    let ignored_songs: Vec<String> = if ignore_path.exists() {
        let content = read_to_string(ignore_path)?;
        content.lines().map(|line| line.trim().to_string()).collect()
    } else {
        Vec::new()
    };

    for i in play_list.song_list {
        // Check if the song exists in the filesystem
        if exists(format!("{}/{}.mp3", config.dir, sanitise_file_name::sanitise(&i.name)))? {
            continue;
        } 
        // Check if the song is in the ignore list
        else if ignored_songs.contains(&i.name) {
            println!("Skipping ignored song: {}", i.name);
            continue;
        } 
        else {
            need_to_download.push(i);
        }
    }

    println!("{need_to_download:#?}");

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

            let mut data = client.get(url).send().await?.bytes().await?.to_vec();

            pb.set_message(sanitise_file_name::sanitise(&i.name));

            let mut tag = id3::Tag::async_read_from(&mut std::io::Cursor::new(&data[..])).await?;

            let lyrics = i.get_lyric(client).await?;

            tag.remove_all_lyrics();
            tag.add_frame(id3::frame::Lyrics {
                lang: "unknown".to_string(),
                description: "lyrics".to_string(),
                text: lyrics,
            });

            tag.write_to(&mut std::io::Cursor::new(&mut data[..]), id3::Version::Id3v24)?;

            write(format!("{}/{}.mp3", dir, sanitise_file_name::sanitise(&i.name)), data)?;

            pb.inc(1);

            Ok(())
        });
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    
    set.join_all().await;
    
    println!("finished");

    Ok(())
}