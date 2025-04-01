use crate::config::{Config, NotificationWindow};
use crate::log;
use crate::notifdaemon::{HistoryNotification, Notification};
use serde_json::json;
use std::collections::HashMap;

// Macro replacement example:
macro_rules! eww_val {
    ({ $($key:literal : $value:expr),* $(,)? }) => {
        serde_json::to_string(&json!({
            $($key: $value),*
        })).unwrap()
    };
}

pub fn eww_is_window_open(cfg: &Config, window: &str) -> bool {
    log!("Checking if {} is open", window);
    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" active-windows");
    //println!("{}", cmd);
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .expect("Failed to execute command");
    let output = String::from_utf8_lossy(&output.stdout);
    output.contains(format!("{}: {}\n", window, window).as_str())
}

pub fn eww_open_window(cfg: &Config, window: &str) -> Result<(), std::io::Error> {
    log!("Opening {}", window);
    if eww_is_window_open(cfg, window) {
        log!("{} is already open", window);
        return Ok(());
    }

    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" open ");
    cmd.push_str(window);
    //println!("{}", cmd);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()?
        .wait()
        .expect("Couldn't open window");
    log!("{} opened", window);
    Ok(())
}

pub fn eww_close_window(cfg: &Config, window: &str) -> Result<(), std::io::Error> {
    log!("Closing {}", window);
    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" close ");
    cmd.push_str(window);
    //println!("{}", cmd);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()?
        .wait()
        .expect("Couldn't close window");
    log!("{} closed", window);
    Ok(())
}

pub fn eww_toggle_window(cfg: &Config, window: &str) -> Result<(), std::io::Error> {
    log!("Toggling {}", window);
    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" open --toggle ");
    cmd.push_str(window);
    //println!("{}", cmd);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()?;
    log!("{} toggled", window);
    Ok(())
}

pub fn eww_update_value(cfg: &Config, var: &str, value: &str) {
    log!("Updating {} with {}", var, value);
    let value = shlex::try_quote(value).unwrap().replace('\n', "<br>");
    let mut cmd = String::new();
    cmd.push_str(&cfg.eww_binary_path);
    cmd.push_str(" update ");
    cmd.push_str(var);
    cmd.push('=');
    cmd.push_str(&value);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()
        .expect("Failed to execute command")
        .wait()
        .expect("Failed to execute command");
    log!("{} updated", var);
}

pub fn quote_hexator(s: &str) -> String {
    s.replace('"', "&#34;").replace('\'', "&#39;")
}

pub fn eww_create_notifications_value(cfg: &Config, notifs: &HashMap<u32, Notification>) -> String {
    let mut widgets = format!(
        "(box :space-evenly false :orientation \"{}\" ",
        cfg.notification_orientation
    );

    for notif in notifs {
        let actions: Vec<_> = notif
            .1
            .actions
            .iter()
            .map(|(id, text)| json!({"id": quote_hexator(id), "text": quote_hexator(text)}))
            .collect();

        let widget_json = eww_val!({
            "actions": actions,
            "application": quote_hexator(&notif.1.app_name),
            "body": quote_hexator(&notif.1.body),
            "icon": quote_hexator(&notif.1.icon),
            "app_icon": quote_hexator(&notif.1.app_icon),
            "id": &notif.0,
            "summary": quote_hexator(&notif.1.summary),
            "urgency": quote_hexator(&notif.1.urgency),
        });

        let widget_string = format!(
            "(box ({} :notification '{}'))",
            cfg.eww_notification_widget, widget_json
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
        // NOTE: Keeping this as a comment for future reference in case eww_val! is not working
        // let widget_string = format!("({} :history \"{{\\\"app_name\\\":\\\"{}\\\",\\\"body\\\":\\\"{}\\\",\\\"icon\\\":\\\"{}\\\",\\\"app_icon\\\":\\\"{}\\\",\\\"summary\\\":\\\"{}\\\"}}\")", cfg.eww_history_widget, hist.app_name, hist.body, hist.icon, hist.app_icon, hist.summary);
        let widget_string = format!(
            "(box ({} :history `{}`))",
            cfg.eww_history_widget,
            eww_val!({
                "app_name": hist.app_name,
                "body": hist.body,
                "icon": hist.icon,
                "app_icon": hist.app_icon,
                "summary": hist.summary,
                "urgency": hist.urgency
            })
        );
        history_text.push_str(&widget_string);
    }
    history_text.push(')');
    history_text
}

pub fn eww_update_history(cfg: &Config, history: &[HistoryNotification]) {
    let widgets = eww_create_history_value(cfg, history);
    eww_update_value(cfg, &cfg.eww_history_var, &widgets);
}

pub fn eww_update_and_open_history(cfg: &Config, history: &[HistoryNotification]) {
    eww_update_history(cfg, history);
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
