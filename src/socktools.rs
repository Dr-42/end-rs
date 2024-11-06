#![allow(clippy::too_many_arguments)]
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use zbus::conn::Builder;
use zbus::fdo::Result;
use zbus::Connection;

use crate::config::Config;
use crate::ewwface::{eww_create_reply_widget, eww_open_window, eww_update_value};
use crate::log;
use crate::notifdaemon::NotificationDaemon;

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

pub async fn run_daemon(cfg: Config) -> Result<()> {
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
    let cfg = Arc::new(cfg);

    // Initialize daemon-specific structures
    let connection = Connection::session().await?;
    let daemon = NotificationDaemon {
        notifications: Default::default(),
        notifications_history: Default::default(),
        config: Arc::clone(&cfg),
        next_id: 0,
        connection,
    };

    let conn = Builder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", daemon)?
        .build()
        .await?;

    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let iface_ref = conn
                .object_server()
                .interface::<_, NotificationDaemon>("/org/freedesktop/Notifications")
                .await
                .unwrap();

            let iface = iface_ref.get_mut().await;
            println!("Received: {}", message);
            let message: DaemonActions = serde_json::from_str(&message).unwrap();
            let dest: Option<&str> = None;

            match message {
                DaemonActions::CloseNotification(id) => {
                    log!("Closing notification {}", id);
                    iface.close_notification(id).await.unwrap();
                    log!("Notification {} closed", id);
                }
                DaemonActions::OpenHistory => {
                    log!("Opening notification history");
                    iface.open_history().await.unwrap();
                    log!("Notification history opened");
                }
                DaemonActions::CloseHistory => {
                    log!("Closing notification history");
                    iface.close_history().await.unwrap();
                    log!("Notification history closed");
                }
                DaemonActions::ToggleHistory => {
                    log!("Toggling notification history");
                    iface.toggle_history().await.unwrap();
                    log!("Notification history toggled");
                }
                DaemonActions::ActionInvoked(id, action) => {
                    if action == "inline-reply" {
                        log!("Opening inline reply for notification {}", id);
                        println!("Opening inline reply window");
                        let eww_widget_str = &eww_create_reply_widget(&cfg, id);
                        println!("{}", eww_widget_str);
                        eww_update_value(&cfg, &cfg.eww_reply_text, "");
                        eww_update_value(&cfg, &cfg.eww_reply_var, eww_widget_str);
                        let _ = eww_open_window(&cfg, &cfg.eww_reply_window);
                        iface.disable_timeout(id).await.unwrap();
                        log!("Inline reply for notification {} opened", id);
                    } else {
                        log!("Invoking action {} for notification {}", action, id);
                        conn.emit_signal(
                            dest,
                            "/org/freedesktop/Notifications",
                            "org.freedesktop.Notifications",
                            "ActionInvoked",
                            &(id, &action),
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
                        log!("Invoked action {} for notification {}", action, id);
                    }
                }
                DaemonActions::ReplySend(id, reply) => {
                    log!("Sending reply {} for notification {}", reply, id);
                    println!("Replying to notification {}", id);
                    conn.emit_signal(
                        dest,
                        "/org/freedesktop/Notifications",
                        "org.freedesktop.Notifications",
                        "NotificationReplied",
                        &(id, &reply),
                    )
                    .await
                    .unwrap();
                    iface.reply_close(id).await.unwrap();
                    iface.close_notification(id).await.unwrap();
                    log!("Sent reply {} for notification {}", reply, id);
                }
                DaemonActions::ReplyClose(id) => {
                    log!("Closing reply for notification {}", id);
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
                    iface.reply_close(id).await.unwrap();
                    log!("Closed reply for notification {}", id);
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
            "reply" => {
                if args.len() < 3 {
                    return Err(zbus::fdo::Error::Failed(
                        "Invalid command to reply".to_string(),
                    ));
                }
                match args[1].as_str() {
                    "send" => {
                        if args.len() < 4 {
                            return Err(zbus::fdo::Error::Failed(
                                "Invalid command to reply-send".to_string(),
                            ));
                        }
                        DaemonActions::ReplySend(args[2].parse::<u32>().unwrap(), args[3].clone())
                    }
                    "close" => DaemonActions::ReplyClose(args[2].parse::<u32>().unwrap()),
                    _ => {
                        return Err(zbus::fdo::Error::Failed("Invalid command".to_string()));
                    }
                }
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
