use futures_util::stream::TryStreamExt;
use rand::random;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use zbus::fdo::Result;
use zbus::{interface, Connection, ConnectionBuilder, MessageStream};
use zvariant::Value;

use crate::config::Config;
use crate::ewwface::{eww_close_notifications, eww_update_notifications};
use crate::utils::{find_icon, save_icon};

pub struct Notification {
    pub app_name: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    pub timeout: i32,
}

struct NotificationDaemon {
    config: Config,
    notifications: Arc<Mutex<HashMap<u32, Notification>>>,
    next_id: Arc<Mutex<u32>>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationDaemon {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<&str>,
        hints: HashMap<&str, zvariant::Value>,
        expire_timeout: i32,
    ) -> u32 {
        let mut notifications = self.notifications.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            *next_id = random::<u32>();
            *next_id
        };
        let icon = if !app_name.is_empty() {
            find_icon(app_icon).or_else(|| {
                hints
                    .get("image_data")
                    .and_then(|value| match value {
                        Value::Structure(icon_data) => save_icon(icon_data, id),
                        _ => None,
                    })
                    .or_else(|| {
                        hints.get("image-data").and_then(|value| match value {
                            Value::Structure(icon_data) => save_icon(icon_data, id),
                            _ => None,
                        })
                    })
                    .or_else(|| Some(app_icon.to_string()))
            })
        } else {
            None
        }
        .unwrap_or_else(|| app_icon.to_string());
        let notification = Notification {
            app_name: app_name.to_string(),
            icon,
            summary: summary.to_string(),
            body: body.to_string(),
            timeout: expire_timeout,
        };
        notifications.insert(id, notification);
        eww_update_notifications(&self.config, &notifications);
        id
    }

    fn close_notification(&self, id: u32) {
        let mut notifications = self.notifications.lock().unwrap();
        if notifications.remove(&id).is_some() {
            println!("Notification with ID {} closed", id);
            eww_update_notifications(&self.config, &notifications);
            if notifications.is_empty() {
                eww_close_notifications(&self.config);
            }
        }
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".to_string(), "actions".to_string()]
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        (
            "NotificationDaemon".to_string(),
            "1.0".to_string(),
            "end-rs".to_string(),
            "1.0".to_string(),
        )
    }
}

pub async fn launch_daemon(cfg: Config) -> Result<()> {
    let daemon = NotificationDaemon {
        notifications: Arc::new(Mutex::new(HashMap::new())),
        next_id: Arc::new(Mutex::new(1)),
        config: cfg,
    };

    let connection = ConnectionBuilder::session()?
        .serve_at("/org/freedesktop/Notifications", daemon)?
        .build()
        .await?;

    connection
        .request_name("org.freedesktop.Notifications")
        .await?;

    println!("Notification Daemon running...");
    loop {
        let mut stream = MessageStream::from(&connection);
        while let Some(msg) = stream.try_next().await? {
            println!("Got message: {}", msg);
        }
    }
}

pub async fn close_notification(id: u32) -> Result<()> {
    let connection = ConnectionBuilder::session()?.build().await?;

    connection.request_name("org.dr42.notifproxy").await?;

    // Send a message to the org.freedesktop.Notifications service
    connection
        .call_method(
            Some("org.freedesktop.Notifications"),
            "/org/freedesktop/Notifications",
            Some("org.freedesktop.Notifications"),
            "CloseNotification",
            &(&id),
        )
        .await?;

    Ok(())
}
