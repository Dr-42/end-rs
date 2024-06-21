use crate::config::Config;
use crate::daemon::Notification;
use std::collections::HashMap;

pub fn eww_open_window(cfg: &Config, window: &str) -> Result<(), std::io::Error> {
    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" open ");
    cmd.push_str(window);
    println!("{}", cmd);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()?;
    Ok(())
}

pub fn eww_close_window(cfg: &Config, window: &str) -> Result<(), std::io::Error> {
    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" close ");
    cmd.push_str(window);
    println!("{}", cmd);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()?;
    Ok(())
}

pub fn eww_update_value(cfg: &Config, var: &str, value: &str) {
    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" update ");
    cmd.push_str(var);
    cmd.push('=');
    cmd.push('\'');
    cmd.push_str(value);
    cmd.push('\'');
    println!("{}", cmd);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()
        .expect("Failed to execute command");
}

pub fn eww_create_notifications_value(cfg: &Config, notifs: &HashMap<u32, Notification>) -> String {
    let mut widgets = "(box :space-evenly false :orientation \"".to_string();
    widgets.push_str(&cfg.notification_orientation);
    widgets.push_str("\" ");

    for notif in notifs {
        let widget_string = format!(
            "(box (end-notification :notification \"{{\\\"actions\\\":[],\\\"application\\\":\\\"{}\\\",\\\"body\\\":\\\"{}\\\",\\\"icon\\\":\\\"{}\\\",\\\"id\\\":{},\\\"summary\\\":\\\"{}\\\"}}\"))",
            notif.1.app_name,
            notif.1.body,
            notif.1.icon,
            notif.0,
            notif.1.summary,
        );
        widgets.push_str(&widget_string);
    }
    widgets.push(')');
    widgets
}

pub fn eww_update_notifications(cfg: &Config, notifs: &HashMap<u32, Notification>) {
    let widgets = eww_create_notifications_value(cfg, notifs);
    eww_update_value(cfg, &cfg.eww_default_notification_var, &widgets);
    let _res = eww_open_window(cfg, &cfg.eww_window);
}

pub fn eww_close_notifications(cfg: &Config) {
    let _res = eww_close_window(cfg, &cfg.eww_window);
}