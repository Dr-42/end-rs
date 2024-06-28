#![allow(clippy::too_many_arguments)]
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{mpsc, Mutex};
use zbus::fdo::Result;
use zbus::Connection;

#[derive(Serialize, Deserialize)]
enum DaemonActions {
    CloseNotification(u32),
    OpenHistory,
    CloseHistory,
    ToggleHistory,
    ActionInvoked(u32, String),
    ReplySend(u32, String),
    ReplyClose(u32),
}

async fn handle_connection(stream: UnixStream, tx: mpsc::Sender<String>) {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    while reader.read_line(&mut line).await.unwrap() > 0 {
        tx.send(line.clone()).await.unwrap();
        line.clear();
    }
}

pub async fn run_daemon(conn: Arc<Mutex<Connection>>) -> Result<()> {
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
                DaemonActions::ReplySend(id, reply) => {
                    println!("Replying to notification {}", id);
                    conn.emit_signal(
                        dest,
                        "/org/freedesktop/Notifications",
                        "org.freedesktop.Notifications",
                        "NotificationReplied",
                        &(id, reply),
                    )
                    .await
                    .unwrap();
                    conn.call_method(
                        Some("org.freedesktop.Notifications"),
                        "/org/freedesktop/Notifications",
                        Some("org.freedesktop.Notifications"),
                        "ReplyClose",
                        &(id),
                    )
                    .await
                    .unwrap();
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
                DaemonActions::ReplyClose(id) => {
                    println!("Closing reply for notification {}", id);
                    conn.call_method(
                        Some("org.freedesktop.Notifications"),
                        "/org/freedesktop/Notifications",
                        Some("org.freedesktop.Notifications"),
                        "ReplyClose",
                        &(id),
                    )
                    .await
                    .unwrap();
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

pub async fn send_message(args: Vec<String>) -> Result<()> {
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
            "reply-send" => {
                if args.len() < 3 {
                    return Err(zbus::fdo::Error::Failed(
                        "Invalid command to reply-send".to_string(),
                    ));
                }
                println!("Sending reply {} {}", args[1], args[2]);
                DaemonActions::ReplySend(args[1].parse::<u32>().unwrap(), args[2].to_string())
            }
            "reply-close" => {
                if args.len() < 2 {
                    return Err(zbus::fdo::Error::Failed(
                        "Invalid command to reply-close".to_string(),
                    ));
                }
                DaemonActions::ReplyClose(args[2].parse::<u32>().unwrap())
            }
            _ => {
                let err = format!("Invalid command {}", args[0]);
                return Err(zbus::fdo::Error::Failed(err));
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
