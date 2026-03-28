use serde::{Deserialize, Serialize};
use std::{env, fs, path::Path};

#[derive(Debug, Serialize, Deserialize)]
pub struct AppState {
    pub dnd_enabled: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self { dnd_enabled: false }
    }
}

fn data_path() -> String {
    let data_home = env::var("XDG_DATA_HOME")
        .unwrap_or_else(|_| format!("{}/.local/share", env::var("HOME").unwrap()));
    format!("{}/end-rs/state.json", data_home)
}

pub fn load_state() -> AppState {
    let path = data_path();
    let path_ref = Path::new(&path);

    if path_ref.exists() {
        let content = fs::read_to_string(path_ref).unwrap();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        let state = AppState::default();
        let dir = path_ref.parent().unwrap();
        fs::create_dir_all(dir).unwrap();
        save_state(&state);
        state
    }
}

pub fn save_state(state: &AppState) {
    let path = data_path();
    let path_ref = Path::new(&path);

    if let Some(dir) = path_ref.parent() {
        fs::create_dir_all(dir).unwrap();
    }

    let content = serde_json::to_string_pretty(state).unwrap();
    fs::write(path_ref, content).unwrap();
}