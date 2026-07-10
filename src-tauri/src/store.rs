use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

// 单个中转站配置
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StationProfile {
    pub id: String,
    pub name: String,
    pub provider_type: String,
    pub base_url: String,
    pub cookie: String,
    pub api_token: String,
    pub new_api_user: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub profiles: Vec<StationProfile>,
    #[serde(default)]
    pub active_profile_id: String,

    pub refresh_interval: u64,
    pub low_balance_threshold: f64,
    pub enable_notification: bool,
    pub always_on_top: bool,
    pub auto_launch: bool,
    pub window_x: i32,
    pub window_y: i32,
    #[serde(default)]
    pub debug_mode: bool,

    // 兼容性：旧格式 settings.json 的顶层字段，仅用于迁移，不再序列化输出
    #[serde(default, skip_serializing)]
    pub provider_type: Option<String>,
    #[serde(default, skip_serializing)]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing)]
    pub cookie: Option<String>,
    #[serde(default, skip_serializing)]
    pub api_token: Option<String>,
    #[serde(default, skip_serializing)]
    pub new_api_user: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        let mock = StationProfile {
            id: uuid::Uuid::new_v4().to_string(),
            name: "Mock（示例数据）".into(),
            provider_type: "mock".into(),
            base_url: String::new(),
            cookie: String::new(),
            api_token: String::new(),
            new_api_user: String::new(),
        };
        let active = mock.id.clone();
        Self {
            profiles: vec![mock],
            active_profile_id: active,
            refresh_interval: 60,
            low_balance_threshold: 5.0,
            enable_notification: true,
            always_on_top: true,
            auto_launch: false,
            window_x: -1,
            window_y: -1,
            debug_mode: false,
            provider_type: None,
            base_url: None,
            cookie: None,
            api_token: None,
            new_api_user: None,
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
    #[serde(default)]
    pub profile_id: String,
}

fn data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn settings_path(app: &AppHandle) -> PathBuf {
    data_dir(app).join("settings.json")
}

fn history_path(app: &AppHandle, profile_id: &str) -> PathBuf {
    data_dir(app).join(format!("history_{}.json", profile_id))
}

pub fn read_settings(app: &AppHandle) -> AppSettings {
    let path = settings_path(app);
    let mut settings = fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<AppSettings>(&s).ok())
        .unwrap_or_default();

    // 迁移旧格式：profiles 为空且存在旧顶层字段
    if settings.profiles.is_empty() && settings.provider_type.is_some() {
        let legacy = StationProfile {
            id: uuid::Uuid::new_v4().to_string(),
            name: "默认中转站".into(),
            provider_type: settings.provider_type.take().unwrap_or_else(|| "mock".into()),
            base_url: settings.base_url.take().unwrap_or_default(),
            cookie: settings.cookie.take().unwrap_or_default(),
            api_token: settings.api_token.take().unwrap_or_default(),
            new_api_user: settings.new_api_user.take().unwrap_or_default(),
        };
        let legacy_id = legacy.id.clone();

        // 迁移旧 history.json → history_<id>.json
        let old_hist = data_dir(app).join("history.json");
        if old_hist.exists() {
            if let Ok(txt) = fs::read_to_string(&old_hist) {
                if let Ok(mut recs) = serde_json::from_str::<Vec<BalanceRecord>>(&txt) {
                    for r in &mut recs {
                        r.profile_id = legacy_id.clone();
                    }
                    write_history(app, &legacy_id, &recs);
                }
            }
            let _ = fs::rename(&old_hist, data_dir(app).join("history.json.bak"));
        }

        settings.profiles.push(legacy);
        settings.active_profile_id = legacy_id;
        write_settings(app, &settings);
    }

    // profiles 为空兜底
    if settings.profiles.is_empty() {
        settings = AppSettings::default();
        write_settings(app, &settings);
    }

    // active_profile_id 校验
    if !settings.profiles.iter().any(|p| p.id == settings.active_profile_id) {
        if let Some(first) = settings.profiles.first() {
            settings.active_profile_id = first.id.clone();
        }
    }

    settings
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

pub fn read_history(app: &AppHandle, profile_id: &str) -> Vec<BalanceRecord> {
    let path = history_path(app, profile_id);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write_history(app: &AppHandle, profile_id: &str, history: &[BalanceRecord]) {
    let path = history_path(app, profile_id);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(history) {
        let _ = fs::write(path, json);
    }
}
