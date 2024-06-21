use futures_util::stream::TryStreamExt;
use rand::random;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use zbus::fdo::Result;
use zbus::{interface, ConnectionBuilder, MessageStream};
use zvariant::Value;

use crate::config::Config;
use crate::ewwface::{eww_close_notifications, eww_update_notifications};
use crate::utils::{find_icon, save_icon};

pub struct Notification {
    pub app_name: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
}

struct NotificationDaemon {
    config: Arc<Mutex<Config>>,
    notifications: Arc<Mutex<HashMap<u32, Notification>>>,
    next_id: Arc<Mutex<u32>>,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationDaemon {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        _actions: Vec<&str>,
        hints: HashMap<&str, zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> u32 {
        let mut notifications = self.notifications.lock().await;
        let mut next_id = self.next_id.lock().await;
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

        let mut expire_timeout = expire_timeout;
        let config_main = self.config.lock().await;
        if expire_timeout < 0 {
            let urgency = hints.get("urgency").and_then(|value| match value {
                Value::U8(urgency) => Some(*urgency),
                _ => None,
            });
            match urgency {
                Some(0) => expire_timeout = config_main.timeout.low as i32 * 1000,
                Some(1) => expire_timeout = config_main.timeout.normal as i32 * 1000,
                Some(2) => expire_timeout = config_main.timeout.critical as i32 * 1000,
                _ => expire_timeout = config_main.timeout.normal as i32 * 1000,
            }
        }

        let notification = Notification {
            app_name: app_name.to_string(),
            icon,
            summary: summary.to_string(),
            body: body.to_string(),
        };

        notifications.insert(id, notification);
        eww_update_notifications(&config_main, &notifications);

        if expire_timeout != 0 {
            // Spawn a task to handle timeout
            let notifications = Arc::clone(&self.notifications);
            let config_thread = Arc::clone(&self.config);
            tokio::spawn(async move {
                sleep(Duration::from_millis(expire_timeout as u64)).await;
                let mut notifications = notifications.lock().await;
                if notifications.remove(&id).is_some() {
                    let config = config_thread.lock().await;
                    eww_update_notifications(&config, &notifications);
                    if notifications.is_empty() {
                        eww_close_notifications(&config);
                    }
                }
            });
        }

        id
    }

    async fn close_notification(&self, id: u32) {
        let mut notifications = self.notifications.lock().await;
        if notifications.remove(&id).is_some() {
            println!("Notification with ID {} closed", id);
            let config = self.config.try_lock();
            if config.is_err() {
                println!("Failed to lock config");
                return;
            }
            let config = config.unwrap();
            eww_update_notifications(&config, &notifications);
            if notifications.is_empty() {
                eww_close_notifications(&config);
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
        config: Arc::new(Mutex::new(cfg)),
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
