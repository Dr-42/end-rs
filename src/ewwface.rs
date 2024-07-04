use crate::config::{Config, NotificationWindow};
use crate::notifdaemon::{HistoryNotification, Notification};
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

pub fn eww_toggle_window(cfg: &Config, window: &str) -> Result<(), std::io::Error> {
    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" open --toggle ");
    cmd.push_str(window);
    println!("{}", cmd);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()?;
    Ok(())
}

pub fn eww_update_value(cfg: &Config, var: &str, value: &str) {
    let value = shlex::try_quote(value).unwrap().replace('\n', "<br>");
    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" update ");
    cmd.push_str(var);
    cmd.push('=');
    //cmd.push('\'');
    cmd.push_str(&value);
    //cmd.push('\'');
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
        let mut action_string = "[".to_string();

        for action in notif.1.actions.iter() {
            let action_str = format!(
                "{{\\\"id\\\":\\\"{}\\\",\\\"text\\\":\\\"{}\\\"}},",
                action.0, action.1
            );
            action_string.push_str(&action_str);
        }
        if !notif.1.actions.is_empty() {
            action_string.pop();
        }
        action_string.push(']');

        let widget_string = format!(
            "(box ({} :notification \"{{\\\"actions\\\":{},\\\"application\\\":\\\"{}\\\",\\\"body\\\":\\\"{}\\\",\\\"icon\\\":\\\"{}\\\",\\\"id\\\":{},\\\"summary\\\":\\\"{}\\\"}}\"))",
            cfg.eww_notification_widget,
            action_string,
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

pub fn eww_create_reply_widget(cfg: &Config, id: u32) -> String {
    format!("(box ({} :id {}))", cfg.eww_reply_widget, id)
}

pub fn eww_update_notifications(cfg: &Config, notifs: &HashMap<u32, Notification>) {
    let widgets = eww_create_notifications_value(cfg, notifs);
    eww_update_value(cfg, &cfg.eww_notification_var, &widgets);
    match &cfg.eww_notification_window {
        NotificationWindow::Single(window) => {
            let _res = eww_open_window(cfg, window);
        }
        NotificationWindow::Multiple(windows) => {
            windows.iter().for_each(|window| {
                let _res = eww_open_window(cfg, window);
            });
        }
    }
}

pub fn eww_close_notifications(cfg: &Config) {
    match &cfg.eww_notification_window {
        NotificationWindow::Single(window) => {
            let _res = eww_close_window(cfg, window);
        }
        NotificationWindow::Multiple(windows) => {
            windows.iter().for_each(|window| {
                let _res = eww_close_window(cfg, window);
            });
        }
    }
}

pub fn eww_create_history_value(cfg: &Config, history: &[HistoryNotification]) -> String {
    let mut history_text = "(box :space-evenly false :orientation \"".to_string();
    history_text.push_str(&cfg.notification_orientation);
    history_text.push_str("\" ");

    let history = history.iter().rev();

    for hist in history {
        let widget_string = format!("({} :history \"{{\\\"app_name\\\":\\\"{}\\\",\\\"body\\\":\\\"{}\\\",\\\"icon\\\":\\\"{}\\\",\\\"summary\\\":\\\"{}\\\"}}\")", cfg.eww_history_widget, hist.app_name, hist.body, hist.icon, hist.summary);
        history_text.push_str(&widget_string);
    }
    history_text.push(')');
    history_text
}

pub fn eww_update_history(cfg: &Config, history: &[HistoryNotification]) {
    let widgets = eww_create_history_value(cfg, history);
    eww_update_value(cfg, &cfg.eww_history_var, &widgets);
    let _res = eww_open_window(cfg, &cfg.eww_history_window);
}

pub fn eww_close_history(cfg: &Config) {
    let _res = eww_close_window(cfg, &cfg.eww_history_window);
}

pub fn eww_toggle_history(cfg: &Config, history: &[HistoryNotification]) {
    let widgets = eww_create_history_value(cfg, history);
    eww_update_value(cfg, &cfg.eww_history_var, &widgets);
    let _res = eww_toggle_window(cfg, &cfg.eww_history_window);
}
