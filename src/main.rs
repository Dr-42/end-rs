use ewwface::{eww_create_reply_widget, eww_open_window, eww_update_value};
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use zbus::connection;
use zbus::fdo::Result;

pub mod config;
pub mod ewwface;
pub mod notifdaemon;
pub mod socktools;
pub mod utils;
use crate::notifdaemon::NotificationDaemon;

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
    } else if arg == "daemon" {
        // Initialize daemon-specific structures
        let connection = connection::Connection::session().await?;
        let daemon = NotificationDaemon {
            notifications: Arc::new(Mutex::new(HashMap::new())),
            notifications_history: Arc::new(Mutex::new(Vec::new())),
            config: Arc::new(Mutex::new(cfg)),
            next_id: 0,
            connection: Arc::new(Mutex::new(connection)),
        };

        let conn = connection::Builder::session()?
            .name("org.freedesktop.Notifications")?
            .serve_at("/org/freedesktop/Notifications", daemon)?
            .build()
            .await?;

        println!("Notification Daemon running...");
        let conn = Arc::new(Mutex::new(conn));
        socktools::run_daemon(conn).await?;
        loop {
            sleep(Duration::from_secs(1)).await;
        }
    } else if arg == "action" {
        if args.len() < 4 {
            eprintln!("Invalid number of arguments for action");
            return Err(zbus::fdo::Error::Failed(
                "Invalid number of arguments for action".to_string(),
            ));
        }
        if args[3] == "inline-reply" {
            println!("Opening inline reply window");
            let id = args[2].parse::<u32>().unwrap();
            let eww_widget_str = &eww_create_reply_widget(id);
            println!("{}", eww_widget_str);
            eww_update_value(&cfg, "reply-text", "");
            eww_update_value(&cfg, "reply-widget-content", eww_widget_str);
            let _ = eww_open_window(&cfg, "notification-reply");
        } else {
            socktools::send_message(args[1..].to_vec()).await?;
            println!("Action sent");
        }
    } else {
        socktools::send_message(args[1..].to_vec()).await?;
        println!("Message sent");
    }

    Ok(())
}
