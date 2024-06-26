#![allow(clippy::too_many_arguments)]
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use zbus::fdo::Result;
use zbus::interface;
use zbus::object_server::SignalContext;
use zvariant::Value;

use crate::config::Config;
use crate::ewwface::{
    eww_close_history, eww_close_notifications, eww_toggle_history, eww_update_history,
    eww_update_notifications,
};
use crate::utils::{find_icon, save_icon};

#[derive(Clone)]
pub struct Notification {
    pub app_name: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<(String, String)>,
}

pub struct NotificationDaemon {
    pub config: Arc<Mutex<Config>>,
    pub notifications: Arc<Mutex<HashMap<u32, Notification>>>,
    pub notifications_history: Arc<Mutex<Vec<Notification>>>,
    pub connection: Arc<Mutex<zbus::Connection>>,
    pub next_id: u32,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationDaemon {
    async fn notify(
        &mut self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<&str>,
        hints: HashMap<&str, zvariant::Value<'_>>,
        expire_timeout: i32,
    ) -> Result<u32> {
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.next_id += 1;
            self.next_id
        };
        let config_main = self.config.lock().await;
        let icon = hints
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
            .or_else(|| {
                if !app_name.is_empty() {
                    find_icon(app_icon, &config_main).or_else(|| Some(app_icon.to_string()))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| app_icon.to_string());

        let mut expire_timeout = expire_timeout;
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

        // create a actions vector of type Vec<(String, String)> where even elements are keys and
        // odd elements are values
        let actions: Vec<(String, String)> = actions
            .chunks(2)
            .map(|chunk| {
                let key = chunk.first().unwrap_or(&"").to_string();
                let value = chunk.get(1).unwrap_or(&"").to_string();
                (key, value)
            })
            .collect();

        let notification = Notification {
            app_name: app_name.to_string(),
            icon,
            actions,
            summary: summary.to_string(),
            body: body.to_string(),
        };

        let mut notifications_history = self.notifications_history.lock().await;
        notifications_history.push(notification.clone());
        // Release the lock before updating the notifications
        if notifications_history.len() > config_main.max_notifications as usize {
            notifications_history.remove(0);
        }
        drop(notifications_history);

        let mut notifications = self.notifications.lock().await;
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

        Ok(id)
    }

    async fn close_notification(&self, id: u32) -> Result<()> {
        let mut notifications = self.notifications.lock().await;
        if notifications.remove(&id).is_some() {
            println!("Notification with ID {} closed", id);
            let config = self.config.try_lock();
            if config.is_err() {
                println!("Failed to lock config");
                return Err(zbus::fdo::Error::Failed(
                    "Failed to lock config".to_string(),
                ));
            }
            let config = config.unwrap();
            eww_update_notifications(&config, &notifications);
            if notifications.is_empty() {
                eww_close_notifications(&config);
            }
            let dest: Option<&str> = None;
            let conn = self.connection.lock().await;
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
        Ok(())
    }

    fn get_capabilities(&self) -> Vec<String> {
        vec!["body".to_string(), "actions".to_string()]
    }

    fn get_server_information(&self) -> Result<(String, String, String, String)> {
        Ok((
            "NotificationDaemon".to_string(),
            "1.0".to_string(),
            "end-rs".to_string(),
            "1.0".to_string(),
        ))
    }

    async fn open_history(&self) -> Result<()> {
        println!("Getting history");
        let config = self.config.try_lock();
        if config.is_err() {
            println!("Failed to lock config");
            return Err(zbus::fdo::Error::Failed(
                "Failed to lock config".to_string(),
            ));
        }
        let config = config.unwrap();
        let history = self.notifications_history.lock().await;
        eww_update_history(&config, &history);
        Ok(())
    }

    async fn close_history(&self) -> Result<()> {
        println!("Closing history");
        let config = self.config.try_lock();
        if config.is_err() {
            println!("Failed to lock config");
            return Err(zbus::fdo::Error::Failed(
                "Failed to lock config".to_string(),
            ));
        }
        let config = config.unwrap();
        eww_close_history(&config);
        Ok(())
    }

    async fn toggle_history(&self) -> Result<()> {
        println!("Toggling history");
        let config = self.config.try_lock();
        if config.is_err() {
            println!("Failed to lock config");
            return Err(zbus::fdo::Error::Failed(
                "Failed to lock config".to_string(),
            ));
        }
        let config = config.unwrap();
        let history = self.notifications_history.lock().await;
        eww_toggle_history(&config, &history);
        Ok(())
    }
    #[zbus(signal)]
    async fn action_invoked(ctx: &SignalContext<'_>, id: u32, action_key: &str)
        -> zbus::Result<()>;

    #[zbus(signal)]
    async fn notification_closed(ctx: &SignalContext<'_>, id: u32, reason: u32)
        -> zbus::Result<()>;
}
