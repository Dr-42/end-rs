#![allow(clippy::too_many_arguments)]
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, Mutex};
use tokio::time::sleep;
use zbus::connection;
use zbus::fdo::Result;
use zbus::Connection;

pub mod config;
pub mod ewwface;
pub mod notifdaemon;
pub mod utils;

use crate::notifdaemon::NotificationDaemon;

async fn handle_connection(stream: UnixStream, tx: mpsc::Sender<String>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap() > 0 {
        tx.send(line.clone()).await.unwrap();
        line.clear();
    }
}

async fn run_daemon(conn: Arc<Mutex<Connection>>) -> Result<()> {
    let path = "/tmp/rust_ipc_socket";
    if Path::new(path).exists() {
        std::fs::remove_file(path).map_err(|e| {
            eprintln!("Failed to remove file: {}", e);
            zbus::fdo::Error::Failed("Failed to remove file".to_string())
        })?
    }

    let listener = UnixListener::bind(path).map_err(|e| {
        eprintln!("Failed to bind to socket: {}", e);
        zbus::fdo::Error::Failed("Failed to bind to socket".to_string())
    })?;

    let (tx, mut rx) = mpsc::channel::<String>(100);

    let conn = Arc::clone(&conn);

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let conn = conn.lock().await;
            println!("Received: {}", message);
            let message: DaemonActions = serde_json::from_str(&message).unwrap();
            let dest: Option<&str> = None;

            match message {
                DaemonActions::CloseNotification(id) => {
                    conn.call_method(
                        Some("org.freedesktop.Notifications"),
                        "/org/freedesktop/Notifications",
                        Some("org.freedesktop.Notifications"),
                        "CloseNotification",
                        &(id),
                    )
                    .await
                    .unwrap();
                }
                DaemonActions::OpenHistory => {
                    conn.call_method(
                        Some("org.freedesktop.Notifications"),
                        "/org/freedesktop/Notifications",
                        Some("org.freedesktop.Notifications"),
                        "OpenHistory",
                        &(),
                    )
                    .await
                    .unwrap();
                }
                DaemonActions::CloseHistory => {
                    conn.call_method(
                        Some("org.freedesktop.Notifications"),
                        "/org/freedesktop/Notifications",
                        Some("org.freedesktop.Notifications"),
                        "CloseHistory",
                        &(),
                    )
                    .await
                    .unwrap();
                }
                DaemonActions::ToggleHistory => {
                    conn.call_method(
                        Some("org.freedesktop.Notifications"),
                        "/org/freedesktop/Notifications",
                        Some("org.freedesktop.Notifications"),
                        "ToggleHistory",
                        &(),
                    )
                    .await
                    .unwrap();
                }
                DaemonActions::ActionInvoked(id, action) => {
                    conn.emit_signal(
                        dest,
                        "/org/freedesktop/Notifications",
                        "org.freedesktop.Notifications",
                        "ActionInvoked",
                        &(id, action),
                    )
                    .await
                    .unwrap();
                    conn.emit_signal(
                        dest,
                        "/org/freedesktop/Notifications",
                        "org.freedesktop.Notifications",
                        "NotificationClosed",
                        &(id, 3_u32),
                    )
                    .await
                    .unwrap();
                }
            };
        }
    });

    loop {
        let (stream, _) = listener.accept().await.map_err(|e| {
            eprintln!("Failed to accept connection: {}", e);
            zbus::fdo::Error::Failed("Failed to accept connection".to_string())
        })?;
        let tx = tx.clone();
        tokio::spawn(async move {
            handle_connection(stream, tx).await;
        });
    }
}

#[derive(Serialize, Deserialize)]
enum DaemonActions {
    CloseNotification(u32),
    OpenHistory,
    CloseHistory,
    ToggleHistory,
    ActionInvoked(u32, String),
}

async fn send_message(args: Vec<String>) -> Result<()> {
    let path = "/tmp/rust_ipc_socket";

    if let Ok(mut stream) = UnixStream::connect(path).await {
        if args.is_empty() {
            eprintln!("No arguments provided");
            return Err(zbus::fdo::Error::Failed(
                "No arguments provided".to_string(),
            ));
        }

        let message = match args[0].as_str() {
            "close" => {
                if args.len() < 2 {
                    return Err(zbus::fdo::Error::Failed(
                        "Invalid command to close".to_string(),
                    ));
                }
                DaemonActions::CloseNotification(args[1].parse::<u32>().unwrap())
            }
            "history" => {
                if args.len() < 2 {
                    return Err(zbus::fdo::Error::Failed(
                        "Invalid command to history".to_string(),
                    ));
                }
                match args[1].as_str() {
                    "open" => DaemonActions::OpenHistory,
                    "close" => DaemonActions::CloseHistory,
                    "toggle" => DaemonActions::ToggleHistory,
                    _ => {
                        return Err(zbus::fdo::Error::Failed("Invalid command".to_string()));
                    }
                }
            }
            "action" => {
                if args.len() < 3 {
                    return Err(zbus::fdo::Error::Failed(
                        "Invalid command to action".to_string(),
                    ));
                }
                DaemonActions::ActionInvoked(args[1].parse::<u32>().unwrap(), args[2].to_string())
            }
            _ => {
                return Err(zbus::fdo::Error::Failed("Invalid command".to_string()));
            }
        };

        let message = serde_json::to_string(&message).map_err(|e| {
            eprintln!("Failed to serialize message: {}", e);
            zbus::fdo::Error::Failed("Failed to serialize message".to_string())
        })?;

        println!("Sending message {:?}", message);
        let message = message.as_bytes();

        stream.write_all(message).await.map_err(|e| {
            eprintln!("Failed to write to stream: {}", e);
            zbus::fdo::Error::Failed("Failed to write to stream".to_string())
        })?;
        println!("Message sent");
    } else {
        eprintln!("Failed to connect to the daemon.");
    }

    println!("Exiting");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = config::parse_config();
    let args = env::args().collect::<Vec<String>>();

    if args.len() < 2 {
        println!("end-rs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let arg = &args[1];
    if arg == "daemon" {
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
        run_daemon(conn).await?;
        loop {
            sleep(Duration::from_secs(1)).await;
        }
    } else {
        send_message(args[1..].to_vec()).await?;
        println!("Message sent");
    }

    Ok(())
}
