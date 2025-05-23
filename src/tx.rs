use anyhow::Result;
use base64::Engine;
use crate::song::Song;

pub struct TXPlayList {
    _id: i64,
    pub song_list: Vec<Song>
}

impl TXPlayList {
    pub async fn new(id: i64) -> Result<TXPlayList> {
        let client = reqwest::ClientBuilder::new()
            .build()?;
        let resp: String = client.get(Self::get_api_url(id))
            .header("Referer", Self::get_referer(id))
            .header("Origin", Self::get_origin())
            .send()
            .await?
            .text()
            .await?;
        let json: serde_json::Value = serde_json::from_str(&resp)?;
        let cd_list: Vec<serde_json::Value> = json["cdlist"][0]["songlist"].as_array().unwrap().to_vec();
        // let song_list = cd_list.iter().map(|song| {song["id"].as_str().unwrap().to_string().parse::<i32>().unwrap()}).collect();
        let mut song_list: Vec<Song> = vec![];
        for song in cd_list.iter() {
            let name = song["name"].as_str().unwrap().to_string();
            let song_id = song["mid"].as_str().unwrap().to_string();
            song_list.push(Song::new(
                name,
                song_id
            ));
        }
        Ok(TXPlayList {
            _id: id,
            song_list
        })
    }

    fn get_referer(id: i64) -> String {
        format!("https://y.qq.com/n/yqq/playsquare/{id}.html")
    }

    fn get_origin() -> &'static str {
        "https://y.qq.com"
    }

    fn get_api_url(id: i64) -> String {
        format!("https://c.y.qq.com/qzone/fcg-bin/fcg_ucc_getcdinfo_byids_cp.fcg?type=1&json=1&utf8=1&onlysong=0&new_format=1&disstid={id}&loginUin=0&hostUin=0&format=json&inCharset=utf8&outCharset=utf-8&notice=0&platform=yqq.json&needNewCode=0")
    }
}

pub trait TXLyric {
    async fn get_lyric(&self, client: reqwest::Client) -> Result<String>;
}

impl TXLyric for Song {
    async fn get_lyric(&self, client: reqwest::Client) -> Result<String> {
        let resp = client.get(
            format!("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={}&g_tk=5381&loginUin=0&hostUin=0&format=json&inCharset=utf8&outCharset=utf-8&platform=yqq", self.id)
        )
            .header(
                "Referer",
                "https://y.qq.com/portal/player.html"
            )
            .send()
            .await?;
        let json = serde_json::from_str::<serde_json::Value>(&resp.text().await?)?;
        let base64_lyric = json["lyric"].as_str().unwrap().to_string();
        let lyric = String::from_utf8(base64::prelude::BASE64_STANDARD.decode(base64_lyric.as_bytes())?)?;

        Ok(lyric)
    }
}