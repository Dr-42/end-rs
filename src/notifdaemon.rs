#![allow(clippy::too_many_arguments)]
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use zbus::fdo::Result;
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zvariant::Value;

use crate::config::Config;
use crate::ewwface::{
    eww_close_history, eww_close_notifications, eww_close_window, eww_toggle_history,
    eww_update_and_open_history, eww_update_history, eww_update_notifications,
};
use crate::log;
use crate::utils::{find_icon, save_icon};

pub struct Notification {
    pub app_name: String,
    pub icon: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub urgency: String,
    pub actions: Vec<(String, String)>,
    pub timeout_cancelled: bool,
    pub timeout_future: Option<JoinHandle<()>>,
}

pub struct HistoryNotification {
    pub app_name: String,
    pub icon: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub urgency: String,
}

pub struct NotificationDaemon {
    pub config: Arc<Config>,
    pub notifications: Arc<Mutex<HashMap<u32, Notification>>>,
    pub notifications_history: Arc<RwLock<Vec<HistoryNotification>>>,
    pub connection: zbus::Connection,
    pub next_id: u32,
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationDaemon {
    pub async fn notify(
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
        log!("Notifying {} - {}", app_name, body);
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            self.next_id += 1;
            self.next_id
        };
        log!("ID: {}", id);
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
                    find_icon(app_icon, &self.config).or_else(|| Some(app_icon.to_string()))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| app_icon.to_string());
        log!("Icon: {}", icon);

        let app_icon = find_icon(app_name, &self.config).unwrap_or("".into());

        log!("AppIcon: {}", app_icon);
        let urgency = hints.get("urgency").and_then(|value| match value {
            Value::U8(urgency) => Some(*urgency),
            _ => None,
        });

        let mut expire_timeout = expire_timeout;
        if expire_timeout < 0 {
            match urgency {
                Some(0) => expire_timeout = self.config.timeout.low as i32 * 1000,
                Some(1) => expire_timeout = self.config.timeout.normal as i32 * 1000,
                Some(2) => expire_timeout = self.config.timeout.critical as i32 * 1000,
                _ => expire_timeout = self.config.timeout.normal as i32 * 1000,
            }
        }

        let urgency_str = match urgency {
            Some(0) => "low",
            Some(1) => "normal",
            Some(2) => "critical",
            _ => "normal",
        };
        log!("Expire timeout: {}", expire_timeout);

        // create an actions vector of type Vec<(String, String)> where even elements are keys and
        // odd elements are values
        let actions: Vec<(String, String)> = actions
            .chunks(2)
            .map(|chunk| {
                let key = chunk.first().unwrap_or(&"").to_string();
                let value = chunk.get(1).unwrap_or(&"").to_string();
                (key, value)
            })
            .collect();

        let is_transient = hints
            .get("transient")
            .and_then(|value| match value {
                Value::Bool(transient) => Some(*transient),
                _ => None,
            })
            .unwrap_or(false);

        if !is_transient {
            log!("Notification is not transient");
            let history_notification = HistoryNotification {
                app_name: app_name.to_string(),
                icon: icon.clone(),
                app_icon: app_icon.clone(),
                summary: summary.to_string(),
                body: body.to_string(),
                urgency: urgency_str.to_string(),
            };
            let mut notifications_history = self.notifications_history.write().await;
            notifications_history.push(history_notification);
            log!("Updated history");
            // Release the lock before updating the notifications
            if notifications_history.len() > self.config.max_notifications as usize {
                notifications_history.remove(0);
            }

            drop(notifications_history);
            if self.config.update_history {
                self.update_history().await?;
                log!("Updated history for update_history");
            }
            log!("Updated history");
        }

        let mut join_handle = None;
        if expire_timeout != 0 {
            // Spawn a task to handle timeout
            let notifications = Arc::clone(&self.notifications);
            let config_thread = Arc::clone(&self.config);
            join_handle = Some(tokio::spawn(async move {
                sleep(Duration::from_millis(expire_timeout as u64)).await;
                let notifications = notifications.try_lock();
                if let Ok(mut notifications) = notifications {
                    if let Some(notif) = notifications.remove(&id) {
                        if !notif.timeout_cancelled {
                            eww_update_notifications(&config_thread, &notifications);
                            if notifications.is_empty() {
                                eww_close_notifications(&config_thread);
                            }
                        }
                    }
                }
            }));
        }

        let notification = Notification {
            app_name: app_name.to_string(),
            icon: icon.clone(),
            app_icon,
            actions,
            summary: summary.to_string(),
            body: body.to_string(),
            urgency: urgency_str.to_string(),
            timeout_cancelled: false,
            timeout_future: join_handle,
        };

        let notifications = self.notifications.try_lock();
        if let Ok(mut notifications) = notifications {
            notifications.insert(id, notification);
            eww_update_notifications(&self.config, &notifications);
        }
        log!("Notification with ID {} created", id);
        Ok(id)
    }

    pub async fn close_notification(&self, id: u32) -> Result<()> {
        let notifications = self.notifications.try_lock();
        if let Ok(mut notifications) = notifications {
            if notifications.remove(&id).is_some() {
                println!("Notification with ID {} closed", id);
                eww_update_notifications(&self.config, &notifications);
                if notifications.is_empty() {
                    eww_close_notifications(&self.config);
                }
                let dest: Option<&str> = None;
                self.connection
                    .emit_signal(
                        dest,
                        "/org/freedesktop/Notifications",
                        "org.freedesktop.Notifications",
                        "NotificationClosed",
                        &(id, 3_u32),
                    )
                    .await
                    .unwrap();
            }
        }
        Ok(())
    }

    pub fn get_capabilities(&self) -> Vec<String> {
        vec!["body".to_string(), "actions".to_string()]
    }

    pub fn get_server_information(&self) -> Result<(String, String, String, String)> {
        Ok((
            "NotificationDaemon".to_string(),
            "1.0".to_string(),
            "end-rs".to_string(),
            "1.0".to_string(),
        ))
    }

    pub async fn update_history(&self) -> Result<()> {
        let history = self.notifications_history.read().await;
        eww_update_history(&self.config, &history);
        Ok(())
    }

    pub async fn open_history(&self) -> Result<()> {
        println!("Getting history");
        let history = self.notifications_history.read().await;
        eww_update_and_open_history(&self.config, &history);
        Ok(())
    }

    pub async fn close_history(&self) -> Result<()> {
        println!("Closing history");
        eww_close_history(&self.config);
        Ok(())
    }

    pub async fn toggle_history(&self) -> Result<()> {
        println!("Toggling history");
        let history = self.notifications_history.read().await;
        eww_toggle_history(&self.config, &history);
        Ok(())
    }

    pub async fn reply_close(&self, id: u32) -> Result<()> {
        println!("Closing reply window");
        let notifications = self.notifications.try_lock();
        if let Err(e) = notifications {
            eprintln!("Failed to lock notifications: {}", e);
            return Err(zbus::fdo::Error::Failed(
                "Failed to lock notifications".to_string(),
            ));
        }
        let mut notifications = notifications.unwrap();
        if let Some(notification) = notifications.get_mut(&id) {
            notification.actions.clear();
            eww_update_notifications(&self.config, &notifications);
        }
        eww_close_window(&self.config, &self.config.eww_reply_window).map_err(|e| {
            eprintln!("Failed to close reply window: {}", e);
            zbus::fdo::Error::Failed("Failed to close reply window".to_string())
        })?;
        Ok(())
    }

    #[zbus(signal)]
    pub async fn action_invoked(
        ctx: &SignalEmitter<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn notification_closed(
        ctx: &SignalEmitter<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn notification_replied(
        ctx: &SignalEmitter<'_>,
        id: u32,
        message: &str,
    ) -> zbus::Result<()>;
}

impl NotificationDaemon {
    pub async fn disable_timeout(&self, id: u32) -> Result<()> {
        let notifications = self.notifications.try_lock();
        if let Err(e) = notifications {
            eprintln!("Failed to lock notifications: {}", e);
            return Err(zbus::fdo::Error::Failed(
                "Failed to lock notifications".to_string(),
            ));
        }
        let mut notifications = notifications.unwrap();
        if let Some(notification) = notifications.get_mut(&id) {
            notification.timeout_cancelled = true;
        }
        Ok(())
    }
}
