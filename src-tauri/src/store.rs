use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

/// settings.json 全局读写锁：悬浮窗续期写 token、设置页保存、切站等并发操作串行化，
/// 避免"读-改-写"交错导致字段被旧数据覆盖。
static SETTINGS_LOCK: Mutex<()> = Mutex::new(());

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
    /// 极智自动登录账号（明文存 settings；密码存 Windows 凭据管理器）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jizhi_username: Option<String>,
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
            jizhi_username: None,
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
    let mut settings = match fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<AppSettings>(&s) {
            Ok(v) => v,
            Err(_) => {
                // 文件损坏（多为并发写入历史遗留）：备份损坏文件再降级默认值，
                // 避免静默用全新配置覆盖（profile_id 变了会让凭据管理器里的密码对不上号）
                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let _ = fs::rename(&path, path.with_extension(format!("json.corrupt-{}", ts)));
                AppSettings::default()
            }
        },
        Err(_) => AppSettings::default(),
    };

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
            jizhi_username: None,
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
        // 原子写：先写临时文件再 rename 覆盖，读者不会读到写了一半的 JSON
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, json).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

/// 锁内执行一次"读 settings → 修改 → 写回"，所有局部改字段的场景都必须走这里，
/// 防止与设置页全量保存、其他窗口的修改互相覆盖。
pub fn update_settings<F: FnOnce(&mut AppSettings)>(app: &AppHandle, f: F) {
    let _guard = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut s = read_settings(app);
    f(&mut s);
    write_settings(app, &s);
}

/// 锁内全量写入设置（设置页"保存设置"用）
pub fn write_settings_locked(app: &AppHandle, settings: &AppSettings) {
    let _guard = SETTINGS_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    write_settings(app, settings);
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
