use zbus::fdo::Result;

mod config;
mod daemon;
mod ewwface;
mod utils;

fn print_help() {
    println!(
        "Usage: end-rs - eww notification daemon in rust {}",
        env!("CARGO_PKG_VERSION")
    );
    println!("Usage: end-rs [OPTIONS] [COMMAND]");
    println!();
    println!("Available options:");
    println!("  --help    Show this help message");
    println!("  --version Show version information");
    println!();
    println!("Available commands:");
    println!("  close <ID>  Close notification with ID");
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let cfg = config::parse_config();
    if args.len() > 1 {
        if args[1] == "--help" {
            print_help();
            return Ok(());
        } else if args[1] == "--version" {
            println!("end-rs {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        } else if args[1] == "close" {
            let close_id = args[2].parse().expect("Invalid ID");
            daemon::close_notification(close_id).await?;
        } else {
            println!("Invalid argument");
            print_help();
            return Ok(());
        }
    }
    daemon::launch_daemon(cfg).await
}
