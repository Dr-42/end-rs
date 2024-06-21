use zbus::fdo::Result;

mod config;
mod daemon;
mod ewwface;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cfg = config::parse_config();
    if args.len() > 1 {
        if args[1] == "close" {
            let close_id = args[2].parse().expect("Invalid ID");
            daemon::close_notification(close_id).await?;
        } else {
            println!("Invalid argument");
            return Ok(());
        }
    }
    daemon::launch_daemon(cfg).await
}
