use zbus::fdo::Result;

mod config;
mod daemon;
mod ewwface;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::parse_config();
    daemon::launch_daemon(cfg).await
}
