use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct TimeoutConfig {
    pub low: u32,
    pub normal: u32,
    pub critical: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub eww_binary_path: String,
    pub icon_dirs: Vec<String>,
    pub icon_theme: String,
    pub eww_notification_window: String,
    pub eww_notification_widget: String,
    pub eww_notification_var: String,
    pub eww_history_window: String,
    pub eww_history_widget: String,
    pub eww_history_var: String,
    pub eww_reply_window: String,
    pub eww_reply_widget: String,
    pub eww_reply_var: String,
    pub eww_reply_text: String,
    pub max_notifications: u32,
    pub notification_orientation: String,
    pub timeout: TimeoutConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            eww_binary_path: String::from("~/.local/bin/eww"),
            icon_dirs: vec![
                String::from("/usr/share/icons"),
                String::from("/usr/share/pixmaps"),
            ],
            icon_theme: String::from("Adwaita"),
            eww_notification_window: String::from("notification-frame"),
            eww_notification_widget: String::from("end-notification"),
            eww_notification_var: String::from("end-notifications"),
            eww_history_window: String::from("history-frame"),
            eww_history_widget: String::from("end-history"),
            eww_history_var: String::from("end-histories"),
            eww_reply_window: String::from("reply-frame"),
            eww_reply_widget: String::from("end-reply"),
            eww_reply_var: String::from("end-replies"),
            eww_reply_text: String::from("end-reply-text"),
            max_notifications: 10,
            notification_orientation: String::from("v"),
            timeout: TimeoutConfig {
                low: 5,
                normal: 10,
                critical: 0,
            },
        }
    }
}

pub fn parse_config() -> Config {
    let mut config = Config::default();
    let xdg_config_home = env::var("XDG_CONFIG_HOME")
        .unwrap_or_else(|_| format!("{}/.config", env::var("HOME").unwrap()));
    let config_path = format!("{}/end-rs/config.toml", xdg_config_home);
    if Path::new(&config_path).exists() {
        let config_str = fs::read_to_string(config_path).unwrap();
        config = toml::from_str(&config_str).unwrap();
    } else {
        let config_dir = Path::new(&config_path).parent().unwrap();
        fs::create_dir_all(config_dir).unwrap();
        let config_str = toml::to_string_pretty(&config).unwrap();
        fs::write(config_path, config_str).unwrap();
    }
    config
}
