use futures_util::stream::TryStreamExt;
use rand::random;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use walkdir::WalkDir;
use zbus::fdo::Result;
use zbus::{interface, ConnectionBuilder, MessageStream};
use zvariant::{Array, Value};

fn find_icon(icon_name: &str) -> Option<String> {
    // Check whether the icon needs to be searched
    if icon_name.starts_with('/') {
        return Some(icon_name.to_string());
    } else if icon_name.starts_with('~') {
        return Some(
            icon_name.replace('~', format!("{}/", std::env::var("HOME").unwrap()).as_str()),
        );
    }
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

struct Notification {
    app_name: String,
    icon: String,
    summary: String,
    body: String,
    timeout: i32,
}

struct NotificationDaemon {
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
        actions: Vec<&str>,
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
        let mut icon = find_icon(app_icon).unwrap_or_default();
        if let Some(Value::Structure(icon_data)) = hints.get("image_data") {
            println!("{:?}", icon_data);
            let parent_dir = "/tmp/end-data";
            if !Path::new(&parent_dir).exists() {
                fs::create_dir_all(parent_dir).unwrap();
            }
            let icon_path = format!("{}/{}.png", parent_dir, id);
            let width: i32 = icon_data.fields()[0]
                .try_clone()
                .unwrap()
                .try_into()
                .unwrap();
            let height: i32 = icon_data.fields()[1]
                .try_clone()
                .unwrap()
                .try_into()
                .unwrap();
            let _rowstride: i32 = icon_data.fields()[2]
                .try_clone()
                .unwrap()
                .try_into()
                .unwrap();
            let _has_alpha: bool = icon_data.fields()[3]
                .try_clone()
                .unwrap()
                .try_into()
                .unwrap();
            let _bits_per_sample: i32 = icon_data.fields()[4]
                .try_clone()
                .unwrap()
                .try_into()
                .unwrap();
            let _channels: i32 = icon_data.fields()[5]
                .try_clone()
                .unwrap()
                .try_into()
                .unwrap();
            let data = icon_data.fields()[6].try_clone().unwrap();
            let mut vec_val: Vec<u8> = Vec::new();
            match data {
                Value::Array(data) => {
                    data.iter()
                        .map(|x| vec_val.push(x.try_clone().unwrap().try_into().unwrap()))
                        .count();
                }
                _ => {
                    println!("Invalid data type");
                }
            }

            let _res = image::save_buffer(
                &icon_path,
                &vec_val,
                width as u32,
                height as u32,
                image::ColorType::Rgba8,
            );

            println!("Icon saved at {}", icon_path);
            icon = icon_path;
        }
        let notification = Notification {
            app_name: app_name.to_string(),
            icon,
            summary: summary.to_string(),
            body: body.to_string(),
            timeout: expire_timeout,
        };
        notifications.insert(id, notification);
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

    let connection = ConnectionBuilder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at("/org/freedesktop/Notifications", daemon)?
        .build()
        .await?;

    println!("Notification Daemon running...");
    loop {
        let mut stream = MessageStream::from(&connection);
        while let Some(msg) = stream.try_next().await? {
            println!("Got message: {}", msg);
        }
    }
}
