use icon_loader::IconLoader;
use std::{fs, io::Write, path::Path};
use zvariant::{Structure, Value};

use crate::config::Config;
use crate::log;

use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

pub fn find_icon(icon_name: &str, config: &Config) -> Option<String> {
    // Check whether the icon needs to be searched
    log!("Icon name: {}", icon_name);
    let mut loader = IconLoader::new();
    log!("Created IconLoader");
    loader.set_search_paths(&config.icon_dirs);
    loader.set_theme_name_provider(config.icon_theme.clone());
    let loader = match loader.update_theme_name() {
        Ok(_) => loader,
        Err(_) => return None,
    };
    let loader = Arc::new(loader);
    log!("Updated IconLoader");

    if icon_name.is_empty() {
        log!("Empty icon name");
        None
    } else if icon_name.starts_with('/') {
        log!("Found icon: {:?}", icon_name);
        Some(icon_name.to_string())
    } else if icon_name.starts_with('~') {
        log!("Found icon: {:?}", icon_name);
        Some(icon_name.replace('~', format!("{}/", std::env::var("HOME").unwrap()).as_str()))
    } else {
        // I believe that IconLoader sometimes locks up if there is no icon for it to look up for.
        // Hence we shove it into a thread and wait it out. In case it locks up, the daemon still
        // keeps running

        log!("Searching for icon: {}", icon_name);

        // Use a channel to communicate between the threads
        let (tx, rx) = mpsc::channel();

        // Spawn a new thread to execute the potentially blocking code
        let icon_name = icon_name.to_string(); // Clone `icon_name` to move it into the thread
        let loader_clone = loader.clone(); // Clone `loader` to move it into the thread
        thread::spawn(move || {
            let icon = loader_clone.load_icon(&icon_name);
            // Send the result back to the main thread
            let _ = tx.send(icon);
        });

        // Set a timeout for the operation
        match rx.recv_timeout(Duration::from_secs(2)) {
            // adjust duration as needed
            Ok(Some(icon)) => {
                log!("Loaded icon");
                let icon_path = icon.file_for_size(config.icon_size.try_into().unwrap()).path().to_str().unwrap().to_string();
                log!("Found icon: {:?}", icon_path);
                Some(icon_path)
            }
            Ok(None) => {
                log!("No icon found");
                None
            }
            Err(_) => {
                // If the thread doesn't respond in time (i.e., it locked up), log an error and return None
                log!("Icon loading timed out");
                None
            }
        }
    }
}

pub fn save_icon(icon_data: &Structure, id: u32) -> Option<String> {
    let parent_dir = "/tmp/end-data";
    if !Path::new(&parent_dir).exists() {
        fs::create_dir_all(parent_dir).unwrap();
    }
    let icon_path = format!("{}/{}.png", parent_dir, id);
    let width: Result<i32, zvariant::Error> = icon_data.fields()[0].try_clone().unwrap().try_into();
    let height: Result<i32, zvariant::Error> =
        icon_data.fields()[1].try_clone().unwrap().try_into();
    let _rowstride: Result<i32, zvariant::Error> =
        icon_data.fields()[2].try_clone().unwrap().try_into();
    let _has_alpha: Result<bool, zvariant::Error> =
        icon_data.fields()[3].try_clone().unwrap().try_into();
    let _bits_per_sample: Result<i32, zvariant::Error> =
        icon_data.fields()[4].try_clone().unwrap().try_into();
    let _channels: Result<i32, zvariant::Error> =
        icon_data.fields()[5].try_clone().unwrap().try_into();
    let data = icon_data.fields()[6].try_clone();
    let mut vec_val: Vec<u8> = Vec::new();
    if width.is_err() || height.is_err() || data.is_err() {
        return None;
    }
    let width = width.unwrap();
    let height = height.unwrap();
    let data = data.unwrap();
    match data {
        Value::Array(data) => {
            data.iter()
                .map(|x| vec_val.push(x.try_clone().unwrap().try_into().unwrap()))
                .count();
        }
        _ => {
            return None;
        }
    }
    let res = image::save_buffer(
        &icon_path,
        &vec_val,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    );
    if res.is_err() {
        return None;
    }
    Some(icon_path)
}

pub fn log(args: std::fmt::Arguments) {
    let message = format!(
        "[{}] {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
        args
    );
    let logfile_path = "/tmp/end.log";
    let mut file = fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(logfile_path)
        .unwrap();
    file.write_all(message.as_bytes()).unwrap();
    file.write_all(b"\n").unwrap();
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        $crate::utils::log(format_args!($($arg)*))
    };
}
