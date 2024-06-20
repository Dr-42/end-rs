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
    pub icon_theme_path: String,
    pub eww_window: String,
    pub eww_default_notification_key: String,
    pub max_notifications: u32,
    pub notification_orientation: String,
    pub timeout: TimeoutConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            eww_binary_path: String::from("eww"),
            icon_theme_path: String::from("/usr/share/icons/AdwaitaLegacy/48x48/"),
            eww_window: String::from("notifications-frame"),
            eww_default_notification_key: String::from("end-notification"),
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
