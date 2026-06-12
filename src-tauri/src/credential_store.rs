// CredentialStore — session cookie 的本地存储封装
//
// 当前实现：明文写入 app data dir 下的 session.json（与 settings.json 同目录）。
//
// TODO: 升级为 Windows Credential Manager（keyring crate）。
//       接口已封装，替换时只需改 read_session / write_session / clear_session
//       三个函数内部实现，调用方不需要改动。
//
// 安全说明：
//   - Cookie 不写入 settings.json，避免跟随设置导出/同步
//   - UI 和日志中只显示脱敏后的前4位+后4位
//   - 后续迁移 keyring 后可彻底脱离文件系统

use std::fs;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SessionData {
    /// 从登录窗口捕获的 cookie 字符串（非 HttpOnly 部分）
    pub session_cookie: String,
    /// 登录时间戳（秒）
    pub captured_at: u64,
    /// 捕获来源："webview" | "manual"
    pub source: String,
}

fn session_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("session.json")
}

pub fn read_session(app: &AppHandle) -> SessionData {
    let path = session_path(app);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn write_session(app: &AppHandle, data: &SessionData) {
    let path = session_path(app);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = fs::write(path, json);
    }
}

pub fn clear_session(app: &AppHandle) {
    let path = session_path(app);
    let _ = fs::remove_file(path);
}

/// 脱敏展示：只返回前4位 + *** + 后4位
pub fn mask_cookie(cookie: &str) -> String {
    if cookie.is_empty() {
        return "(empty)".to_string();
    }
    let chars: Vec<char> = cookie.chars().collect();
    if chars.len() <= 8 {
        return "***".to_string();
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{}***{}", head, tail)
}
