use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use zbus::fdo::Result;
use zbus::{interface, ConnectionBuilder};

fn find_icon(icon_name: &str) -> Option<String> {
    let icon_dir = "/usr/share/icons/AdwaitaLegacy/48x48/";
    // Recursively search for the icon in the icon directories
    for entry in WalkDir::new(icon_dir) {
        if entry.is_err() {
            continue;
        }
        let entry = entry.unwrap();
        if entry.file_name().to_string_lossy().contains(icon_name) {
            return Some(entry.path().to_string_lossy().to_string());
        }
    }
    None
}

struct NotificationDaemon {
    notifications: Arc<Mutex<HashMap<u32, String>>>,
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
        actions: Vec<&str>,
        hints: HashMap<&str, zvariant::Value>,
        expire_timeout: i32,
    ) -> u32 {
        let mut notifications = self.notifications.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
        let id = if replaces_id != 0 {
            replaces_id
        } else {
            *next_id += 1;
            *next_id
        };
        notifications.insert(id, summary.to_string());
        println!("Notification from: {}", app_name);
        println!("Notification received: {} - {}", summary, body);
        println!("Icon: {}", app_icon);
        println!("Icon path: {:?}", find_icon(app_icon));
        println!("Hints: {:?}", hints);
        println!("Actions: {:?}", actions);
        println!("Expire timeout: {}", expire_timeout);
        id
    }

    fn close_notification(&self, id: u32) {
        let mut notifications = self.notifications.lock().unwrap();
        if notifications.remove(&id).is_some() {
            println!("Notification with ID {} closed", id);
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

#[tokio::main]
async fn main() -> Result<()> {
    let daemon = NotificationDaemon {
        notifications: Arc::new(Mutex::new(HashMap::new())),
        next_id: Arc::new(Mutex::new(1)),
    };

    let _connection = ConnectionBuilder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", daemon)?
        .build()
        .await?;

    println!("Notification Daemon running...");
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
