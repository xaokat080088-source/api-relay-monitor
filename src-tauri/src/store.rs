use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub provider_type: String,
    pub base_url: String,
    pub cookie: String,
    pub api_token: String,
    pub new_api_user: String,
    pub refresh_interval: u64,
    pub low_balance_threshold: f64,
    pub enable_notification: bool,
    pub always_on_top: bool,
    pub auto_launch: bool,
    pub window_x: i32,
    pub window_y: i32,
    #[serde(default)]
    pub debug_mode: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            provider_type: "mock".into(),
            base_url: String::new(),
            cookie: String::new(),
            api_token: String::new(),
            new_api_user: String::new(),
            refresh_interval: 60,
            low_balance_threshold: 5.0,
            enable_notification: true,
            always_on_top: true,
            auto_launch: false,
            window_x: -1,
            window_y: -1,
            debug_mode: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BalanceRecord {
    pub balance: f64,
    pub last_cost: Option<f64>,
    pub today_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub request_count: Option<u64>,
    pub token_used: Option<u64>,
    pub timestamp: u64,
    pub source: String,
}

fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn settings_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join("settings.json")
}

fn history_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join("history.json")
}

pub fn read_settings(app: &AppHandle) -> AppSettings {
    let path = settings_path(app);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write_settings(app: &AppHandle, settings: &AppSettings) {
    let path = settings_path(app);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(settings) {
        let _ = fs::write(path, json);
    }
}

pub fn read_history(app: &AppHandle) -> Vec<BalanceRecord> {
    let path = history_path(app);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write_history(app: &AppHandle, history: &[BalanceRecord]) {
    let path = history_path(app);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(history) {
        let _ = fs::write(path, json);
    }
}
