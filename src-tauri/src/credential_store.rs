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

use std::collections::HashMap;
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

// ── 极智账密存储（双备份）────────────────────────────────────
//
// 存储 1：Windows 凭据管理器（keyring crate，service = "api-monitor"，
//         account = "jizhi-<profile_id>"，DPAPI 持久化）。
// 存储 2：DPAPI 加密本地文件 jizhi_credentials.json（仅当前 Windows 用户可解密）。
//
// 为什么双备份：keyring 3 在 Windows 上必须开 windows-native feature 才真正写凭据管理器，
// 否则默认是进程内 mock 存储（不落盘，重启或新建 Entry 即读不到）——早期版本就栽在这里。
// 现在两处都真实落盘：读取时先凭据管理器、后加密文件，任一存活即可自动登录，
// 文件兜底命中后还会回写修复凭据管理器。

const KEYRING_SERVICE: &str = "api-monitor";

fn jizhi_entry(profile_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("jizhi-{}", profile_id))
        .map_err(|e| format!("keyring entry: {}", e))
}

// ── DPAPI 加密文件 ──────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
struct JizhiCredEntry {
    /// DPAPI 加密后的密码（base64）；空 = 未存
    #[serde(default)]
    password_b64: String,
    /// 账号（非敏感，明文；settings.json 才是主存储，这里是兜底镜像）
    #[serde(default)]
    username: String,
    #[serde(default)]
    saved_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct JizhiCredFile {
    #[serde(default)]
    profiles: HashMap<String, JizhiCredEntry>,
}

/// 加密文件全局写锁（读改写串行化）
static CRED_FILE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn cred_file_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("jizhi_credentials.json")
}

fn load_cred_file(app: &AppHandle) -> JizhiCredFile {
    fs::read_to_string(cred_file_path(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn store_cred_file(app: &AppHandle, file: &JizhiCredFile) {
    let path = cred_file_path(app);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(file) {
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, json).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

/// 锁内修改加密文件
fn with_cred_file<F: FnOnce(&mut JizhiCredFile)>(app: &AppHandle, f: F) {
    let _guard = CRED_FILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut file = load_cred_file(app);
    f(&mut file);
    store_cred_file(app, &file);
}

// ── DPAPI（CryptProtectData / CryptUnprotectData，CurrentUser 范围）──

#[allow(non_snake_case)]
mod dpapi {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB,
    };

    fn to_blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_ptr() as *mut u8,
        }
    }

    /// DPAPI 加密（CurrentUser：只有本机当前用户可解）。存 UTF-16LE 字节，与 keyring 语义一致。
    pub fn encrypt(plain: &str) -> Option<Vec<u8>> {
        let input: Vec<u8> = plain
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let in_blob = to_blob(&input);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = unsafe {
            CryptProtectData(
                &in_blob,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                &mut out,
            )
        };
        if ok == 0 || out.cbData == 0 {
            return None;
        }
        let buf = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe { LocalFree(out.pbData as *mut core::ffi::c_void) };
        Some(buf)
    }

    /// DPAPI 解密（blob 为 UTF-16LE 编码的密码）
    pub fn decrypt(blob: &[u8]) -> Option<String> {
        let in_blob = to_blob(blob);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = unsafe {
            CryptUnprotectData(
                &in_blob,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                &mut out,
            )
        };
        if ok == 0 || out.cbData == 0 {
            return None;
        }
        let buf = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe { LocalFree(out.pbData as *mut core::ffi::c_void) };
        let units: Vec<u16> = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        Some(String::from_utf16_lossy(&units))
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── 对外接口 ────────────────────────────────────────────────

/// 保存极智账密：keyring + DPAPI 加密文件双写 + 账号镜像。
/// 密码为空 = 清除该 profile 的全部极智凭据。
pub fn save_jizhi_credentials(app: &AppHandle, profile_id: &str, username: &str, password: &str) -> Result<(), String> {
    if password.is_empty() {
        // 清除：凭据管理器条目 + 加密文件条目（含账号镜像）
        delete_jizhi_password(profile_id);
        with_cred_file(app, |file| {
            file.profiles.remove(profile_id);
        });
        return Ok(());
    }

    // 1) keyring（失败不阻断，加密文件仍是有效备份）
    if let Err(e) = jizhi_entry(profile_id)?.set_password(password) {
        eprintln!("[credential] keyring 写入失败（已由加密文件兜底）: {}", e);
    }
    // 2) DPAPI 加密文件（密码密文 + 账号明文）
    let cipher = dpapi::encrypt(password).ok_or("DPAPI 加密失败")?;
    let b64 = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(cipher)
    };
    let uname = username.to_string();
    let ts = now_secs();
    with_cred_file(app, |file| {
        file.profiles.insert(
            profile_id.to_string(),
            JizhiCredEntry { password_b64: b64, username: uname, saved_at: ts },
        );
    });
    Ok(())
}

/// 读取极智密码：先凭据管理器，后 DPAPI 加密文件（命中文件时回写 keyring 自愈）
pub fn load_jizhi_password(app: &AppHandle, profile_id: &str) -> Option<String> {
    // 1) keyring
    if let Ok(entry) = jizhi_entry(profile_id) {
        if let Ok(p) = entry.get_password() {
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    // 2) DPAPI 文件兜底
    let file = load_cred_file(app);
    let entry = file.profiles.get(profile_id)?;
    if entry.password_b64.is_empty() {
        return None;
    }
    let cipher = {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(&entry.password_b64)
            .ok()?
    };
    let password = dpapi::decrypt(&cipher)?;
    if password.is_empty() {
        return None;
    }
    // 自愈：把密码回写凭据管理器
    if let Ok(e) = jizhi_entry(profile_id) {
        let _ = e.set_password(&password);
    }
    Some(password)
}

/// 读取极智账号镜像（未存过返回 None）
pub fn load_jizhi_username(app: &AppHandle, profile_id: &str) -> Option<String> {
    let file = load_cred_file(app);
    let u = file.profiles.get(profile_id)?.username.trim().to_string();
    if u.is_empty() { None } else { Some(u) }
}

/// 删除已存的极智密码（不存在时静默成功）
pub fn delete_jizhi_password(profile_id: &str) {
    if let Ok(entry) = jizhi_entry(profile_id) {
        let _ = entry.delete_credential();
    }
}

/// 兼容旧调用：仅保存密码（无账号信息）到双存储
pub fn save_jizhi_password(app: &AppHandle, profile_id: &str, password: &str) -> Result<(), String> {
    // 保留文件里已有的账号
    let uname = load_jizhi_username(app, profile_id).unwrap_or_default();
    save_jizhi_credentials(app, profile_id, &uname, password)
}
