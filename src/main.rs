use std::env;
use zbus::fdo::Result;

pub mod config;
pub mod ewwface;
pub mod generator;
pub mod notifdaemon;
pub mod socktools;
pub mod utils;

fn print_help() {
    println!("end-rs {}", env!("CARGO_PKG_VERSION"));
    println!("Usage: end-rs [OPTIONS] <COMMAND> <args>");
    println!();
    println!("Options:");
    println!("  -h, --help - Print this help message");
    println!("  -v, --version - Print version information");
    println!("Commands:");
    println!("  daemon - Start the notification daemon");
    println!("  close <id> - Close a notification with the given ID");
    println!("  history <open|close|toggle> - Open, close or toggle the notification history");
    println!("  action <id> <action> - Perform an action on a notification with the given ID");
    println!();
    println!("  generate [css|yuck|all] - Generate the eww config files");
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::parse_config();
    let args = env::args().collect::<Vec<String>>();

    if args.len() < 2 {
        print_help();
        return Ok(());
    }

    let arg = &args[1];
    if arg == "-h" || arg == "--help" {
        print_help();
        return Ok(());
    } else if arg == "-v" || arg == "--version" {
        println!("end-rs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    } else if arg == "generate" {
        if args.len() < 3 {
            print_help();
            return Ok(());
        }
        let arg = &args[2];
        let mut css = false;
        let mut yuck = false;
        if arg == "css" {
            css = true;
        } else if arg == "yuck" {
            yuck = true;
        } else if arg == "all" {
            css = true;
            yuck = true;
        } else {
            print_help();
            return Ok(());
        }
        generator::generate_files(css, yuck).await?;
        println!("Files generated");
    } else if arg == "daemon" {
        println!("Notification Daemon running...");
        socktools::run_daemon(cfg).await?;
    } else {
        socktools::send_message(args[1..].to_vec()).await?;
        println!("Message sent");
    }

    Ok(())
}
