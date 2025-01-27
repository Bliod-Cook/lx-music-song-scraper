use std::process::exit;
use clap::Parser;

pub struct Config {
    pub dir: String,
    pub play_id: i64,
    pub lx_api_url: String,
    pub lx_api_key: String,
}

#[derive(Parser, Debug)]
struct Args {
    #[arg(short, default_missing_value = "./music")]
    dir: String,
    #[arg(short)]
    play_id: i64,
}

impl Config {
    pub fn get() -> Config {
        dotenvy::dotenv().ok();
        let args = Args::parse();
        // Load From DotEnv
        let lx_api_url = std::env::var("LX_API_URL").unwrap_or_else(|e|  {
            println!("{e}");
            exit(0)
        });
        let lx_api_key = std::env::var("LX_API_KEY").unwrap_or_else(|e|  {
            println!("{e}");
            exit(0)
        });
        Config {
            dir: args.dir,
            play_id: args.play_id,
            lx_api_url,
            lx_api_key,
        }
    }
}