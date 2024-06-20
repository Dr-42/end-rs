use zbus::fdo::Result;

mod config;
mod daemon;
mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    daemon::launch_daemon().await
}
