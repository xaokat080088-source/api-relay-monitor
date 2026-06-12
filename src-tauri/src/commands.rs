use tauri::{AppHandle, Manager, Emitter, command};
use crate::store::{AppSettings, BalanceRecord, read_settings, write_settings, read_history, write_history};
use crate::credential_store::{SessionData, read_session, write_session, clear_session, mask_cookie};

#[command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    read_settings(&app)
}

#[command]
pub fn save_settings(app: AppHandle, settings: AppSettings) -> AppSettings {
    write_settings(&app, &settings);
    settings
}

#[command]
pub fn get_history(app: AppHandle) -> Vec<BalanceRecord> {
    read_history(&app)
}

#[command]
pub fn append_history(app: AppHandle, record: BalanceRecord) -> Vec<BalanceRecord> {
    let mut history = read_history(&app);
    history.push(record);
    if history.len() > 200 {
        let drain = history.len() - 200;
        history.drain(0..drain);
    }
    write_history(&app, &history);
    history
}

#[command]
pub fn clear_history(app: AppHandle) -> Vec<BalanceRecord> {
    write_history(&app, &[]);
    vec![]
}

#[command]
pub fn open_url(url: String) {
    let _ = open::that(url);
}

#[command]
pub fn show_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("floating") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[command]
pub fn hide_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("floating") {
        let _ = win.hide();
    }
}

#[command]
pub fn set_always_on_top(app: AppHandle, on_top: bool) {
    if let Some(win) = app.get_webview_window("floating") {
        let _ = win.set_always_on_top(on_top);
    }
}

#[command]
pub fn move_window(app: AppHandle, x: i32, y: i32) {
    if let Some(win) = app.get_webview_window("floating") {
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
        let mut s = read_settings(&app);
        s.window_x = x;
        s.window_y = y;
        write_settings(&app, &s);
    }
}

#[command]
pub fn close_settings_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.close();
    }
}

#[command]
pub fn broadcast_settings_changed(app: AppHandle) {
    let _ = app.emit("cmd:settings-changed", ());
}

#[command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    // 若窗口已存在：直接聚焦，不重建（避免 label 冲突 / 竞态白屏）
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }

    // dev 模式下 WebviewUrl::App 解析为 http://localhost:<port>/<path>
    // production 下解析为 tauri://localhost/<path>
    // "index.html?page=settings" 在两种模式下都正确
    tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("index.html?page=settings".into()),
    )
    .title("API Monitor 设置")
    .inner_size(520.0, 700.0)
    .min_inner_size(400.0, 500.0)
    .resizable(true)
    .decorations(true)
    .transparent(false)
    .always_on_top(false)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[command]
pub fn notify_low_balance(app: AppHandle, balance: f64) {
    let s = read_settings(&app);
    if !s.enable_notification {
        return;
    }
    use tauri_plugin_notification::NotificationExt;
    let _ = app
        .notification()
        .builder()
        .title("API Monitor — 余额不足")
        .body(format!(
            "当前余额 ${:.2}，已低于阈值 ${}",
            balance, s.low_balance_threshold
        ))
        .show();
}

// ── Cookie 管理（手动粘贴方案）────────────────────────────────

/// 保存手动粘贴的 cookie，并通知前端刷新状态。
#[command]
pub fn save_session_cookie(app: AppHandle, cookie: String, source: String) -> Result<(), String> {
    if cookie.trim().is_empty() {
        return Err("cookie 为空".to_string());
    }
    let data = SessionData {
        session_cookie: cookie,
        captured_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        source,
    };
    write_session(&app, &data);
    let _ = app.emit("session:captured", ());
    Ok(())
}

#[command]
pub fn notify_session_captured(app: AppHandle) {
    let _ = app.emit("session:captured", ());
}

/// 返回当前 session 状态（不返回完整 cookie）
#[command]
pub fn get_session_status(app: AppHandle) -> SessionStatus {
    let data = read_session(&app);
    if data.session_cookie.is_empty() {
        SessionStatus {
            has_session: false,
            masked_cookie: String::new(),
            captured_at: 0,
            source: String::new(),
        }
    } else {
        SessionStatus {
            has_session: true,
            masked_cookie: mask_cookie(&data.session_cookie),
            captured_at: data.captured_at,
            source: data.source,
        }
    }
}

/// 返回完整 session cookie 给 provider 使用（仅 Rust 侧内部，前端不展示原始值）
#[command]
pub fn get_session_cookie(app: AppHandle) -> String {
    read_session(&app).session_cookie
}

/// 清除 session
#[command]
pub fn clear_session_cookie(app: AppHandle) {
    clear_session(&app);
}

// ── 辅助类型 ──────────────────────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub has_session: bool,
    pub masked_cookie: String,
    pub captured_at: u64,
    pub source: String,
}

// ── xiaoma_fetch：Rust 后端代理请求，绕过渲染进程 CORS ─────────

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct XiaomaLogItem {
    pub time: String,
    pub timestamp: u64,
    pub token_name: String,
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct XiaomaWallet {
    pub balance: f64,
    pub total_cost: f64,
    pub request_count: i64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct XiaomaSnapshot {
    pub wallet: XiaomaWallet,
    pub recent_logs: Vec<XiaomaLogItem>,
    pub status: String,      // "ok" | "cookie_missing" | "auth_error" | "network_error" | "parse_error"
    pub log_error: Option<String>,
    pub timestamp: u64,
    // 调试摘要字段（不含完整 Cookie/响应体）
    pub debug_url: Option<String>,
    pub debug_http_status: Option<u16>,
    pub debug_resp_keys: Option<String>,   // 顶层字段名，逗号分隔
    pub debug_message: Option<String>,     // 接口返回的 message/error 字段
}

const QUOTA_TO_USD: f64 = 500_000.0;

fn quota_usd(q: f64) -> f64 {
    (q / QUOTA_TO_USD * 1_000_000.0).round() / 1_000_000.0
}

fn fmt_timestamp(ts: u64) -> String {
    // ts 可能是秒或毫秒
    let ms = if ts > 1_000_000_000_000 { ts } else { ts * 1000 };
    let secs = ms / 1000;
    // 加东八区偏移（+8h = +28800s）
    let secs_local = secs + 8 * 3600;
    let s = secs_local % 60;
    let m = (secs_local / 60) % 60;
    let h = (secs_local / 3600) % 24;
    let days_since_epoch = secs_local / 86400;
    let (y, mo, d) = days_from_epoch(days_since_epoch);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, mo, d, h, m, s)
}

fn days_from_epoch(mut z: u64) -> (u32, u32, u32) {
    z += 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y as u32, mo as u32, d as u32)
}

fn pick_f64(v: &serde_json::Value, keys: &[&str]) -> Option<f64> {
    for k in keys {
        if let Some(val) = v.get(k) {
            if let Some(n) = val.as_f64() {
                return Some(n);
            }
        }
    }
    None
}

fn pick_i64(v: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    for k in keys {
        if let Some(val) = v.get(k) {
            if let Some(n) = val.as_i64() {
                return Some(n);
            }
        }
    }
    None
}

fn pick_str<'a>(v: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    for k in keys {
        if let Some(val) = v.get(k) {
            if let Some(s) = val.as_str() {
                return Some(s);
            }
        }
    }
    None
}

fn parse_log_item(item: &serde_json::Value) -> XiaomaLogItem {
    let raw_quota = pick_f64(item, &["quota", "used_quota"]).unwrap_or(0.0);
    let cost = pick_f64(item, &["cost", "amount"]).unwrap_or_else(|| quota_usd(raw_quota));

    let input_tokens = pick_i64(item, &[
        "prompt_tokens", "promptTokens", "input_tokens", "inputTokens", "prompt",
    ]).unwrap_or(0);

    let output_tokens = pick_i64(item, &[
        "completion_tokens", "completionTokens", "output_tokens", "outputTokens", "completion",
    ]).unwrap_or(0);

    let model = pick_str(item, &["model_name", "modelName", "model"])
        .unwrap_or("--").to_string();

    let token_name = pick_str(item, &[
        "token_name", "tokenName", "token", "key_name", "name", "username", "channel",
    ]).unwrap_or("--").to_string();

    let ts = pick_i64(item, &["created_at", "createdAt", "created"])
        .unwrap_or(0) as u64;
    let time = if ts > 0 { fmt_timestamp(ts) } else { "--".to_string() };

    XiaomaLogItem { time, timestamp: ts, token_name, model, input_tokens, output_tokens, cost }
}

fn extract_logs(data: &serde_json::Value) -> Vec<serde_json::Value> {
    if let Some(arr) = data.as_array() {
        return arr.clone();
    }
    if let Some(d) = data.get("data") {
        if let Some(arr) = d.as_array() {
            return arr.clone();
        }
        if let Some(items) = d.get("items").and_then(|v| v.as_array()) {
            return items.clone();
        }
        if let Some(logs) = d.get("logs").and_then(|v| v.as_array()) {
            return logs.clone();
        }
    }
    vec![]
}

#[command]
pub async fn xiaoma_fetch(
    base_url: String,
    cookie: String,
    api_token: Option<String>,
    new_api_user: Option<String>,
    debug_mode: bool,
) -> Result<XiaomaSnapshot, String> {
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let cookie = cookie.trim().to_string();
    let api_token = api_token.as_deref().unwrap_or("").trim().to_string();
    let new_api_user = new_api_user.as_deref().unwrap_or("").trim().to_string();

    if cookie.is_empty() && api_token.is_empty() {
        return Ok(XiaomaSnapshot {
            wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
            recent_logs: vec![],
            status: "cookie_missing".to_string(),
            log_error: None,
            timestamp: now_ts,
            debug_url: None,
            debug_http_status: None,
            debug_resp_keys: None,
            debug_message: None,
        });
    }

    let base = base_url.trim_end_matches('/').to_string();

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
        .danger_accept_invalid_certs(false)
        .build()
        .map_err(|e| format!("network_error: {}", e))?;

    // ── 1. 用户信息 ─────────────────────────────────────────────

    let user_url = format!("{}/api/user/self", base);

    let mut req = client.get(&user_url)
        .header("Accept", "application/json, text/plain, */*")
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("Referer", format!("{}/console/topup", base))
        .header("Origin", &base);
    if !cookie.is_empty() {
        req = req.header("Cookie", &cookie);
    }
    if !api_token.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", api_token));
    }
    if !new_api_user.is_empty() {
        req = req.header("New-Api-User", &new_api_user);
    }

    let user_resp = match req.send().await {
        Err(e) => {
            return Ok(XiaomaSnapshot {
                wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
                recent_logs: vec![],
                status: "network_error".to_string(),
                log_error: Some(format!("请求失败: {}", e)),
                timestamp: now_ts,
                debug_url: Some(user_url),
                debug_http_status: None,
                debug_resp_keys: None,
                debug_message: None,
            });
        }
        Ok(r) => r,
    };

    let user_status = user_resp.status().as_u16();
    if debug_mode {
        eprintln!("[xiaoma_fetch] user/self status: {}", user_status);
    }

    if user_status == 401 || user_status == 403 {
        let body_text = user_resp.text().await.unwrap_or_default();
        let parsed = serde_json::from_str::<serde_json::Value>(&body_text).ok();
        let api_msg = parsed.as_ref()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()))
            .or_else(|| if !body_text.is_empty() { Some(body_text.chars().take(120).collect()) } else { None });

        // 根据 message 细化 status
        let status = if let Some(ref msg) = api_msg {
            if msg.contains("未提供 New-Api-User") || msg.contains("New-Api-User") {
                "new_api_user_missing"
            } else if msg.contains("额度不足") || msg.contains("用户额度不足") || msg.contains("余额不足") {
                "balance_insufficient"
            } else {
                "auth_error"
            }
        } else {
            "auth_error"
        };

        return Ok(XiaomaSnapshot {
            wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
            recent_logs: vec![],
            status: status.to_string(),
            log_error: api_msg.clone(),
            timestamp: now_ts,
            debug_url: Some(user_url),
            debug_http_status: Some(user_status),
            debug_resp_keys: None,
            debug_message: api_msg,
        });
    }
    if user_status < 200 || user_status >= 300 {
        let body_text = user_resp.text().await.unwrap_or_default();
        let preview: String = body_text.chars().take(120).collect();
        return Ok(XiaomaSnapshot {
            wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
            recent_logs: vec![],
            status: "network_error".to_string(),
            log_error: Some(format!("HTTP {}: {}", user_status, preview)),
            timestamp: now_ts,
            debug_url: Some(user_url),
            debug_http_status: Some(user_status),
            debug_resp_keys: None,
            debug_message: None,
        });
    }

    let user_json: serde_json::Value = match user_resp.json().await {
        Err(e) => {
            return Ok(XiaomaSnapshot {
                wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
                recent_logs: vec![],
                status: "parse_error".to_string(),
                log_error: Some(format!("JSON 解析失败: {}", e)),
                timestamp: now_ts,
                debug_url: Some(user_url),
                debug_http_status: Some(user_status),
                debug_resp_keys: None,
                debug_message: None,
            });
        }
        Ok(v) => v,
    };

    // 收集顶层字段名（用于调试）
    let top_keys = user_json.as_object()
        .map(|obj| obj.keys().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_default();

    if debug_mode {
        eprintln!("[xiaoma_fetch] user/self keys: {}", top_keys);
    }

    // success == false
    if user_json.get("success") == Some(&serde_json::Value::Bool(false)) {
        let msg = user_json.get("message").and_then(|v| v.as_str()).unwrap_or("unknown");
        let status = if msg.contains("未登录") || msg.contains("无效") || msg.contains("token") {
            "auth_error"
        } else {
            "parse_error"
        };
        return Ok(XiaomaSnapshot {
            wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
            recent_logs: vec![],
            status: status.to_string(),
            log_error: Some(msg.to_string()),
            timestamp: now_ts,
            debug_url: Some(user_url),
            debug_http_status: Some(user_status),
            debug_resp_keys: Some(top_keys),
            debug_message: Some(msg.to_string()),
        });
    }

    let d = user_json.get("data").unwrap_or(&user_json);
    let remain = pick_f64(d, &["quota", "remain_quota"]).unwrap_or(0.0);
    let used = pick_f64(d, &["used_quota"]).unwrap_or(0.0);
    let req_count = pick_i64(d, &["request_count"]).unwrap_or(0);

    let wallet = XiaomaWallet {
        balance: quota_usd(remain),
        total_cost: quota_usd(used),
        request_count: req_count,
    };

    // ── 2. 使用日志 ─────────────────────────────────────────────

    let end_ts = now_ts;
    let start_ts = end_ts.saturating_sub(86400 * 7);
    let log_url = format!(
        "{}/api/log/self?start_timestamp={}&end_timestamp={}&p=1&page_size=20",
        base, start_ts, end_ts
    );

    let mut log_req = client.get(&log_url)
        .header("Accept", "application/json, text/plain, */*")
        .header("Referer", format!("{}/console/topup", base))
        .header("Origin", &base);
    if !cookie.is_empty() {
        log_req = log_req.header("Cookie", &cookie);
    }
    if !api_token.is_empty() {
        log_req = log_req.header("Authorization", format!("Bearer {}", api_token));
    }
    if !new_api_user.is_empty() {
        log_req = log_req.header("New-Api-User", &new_api_user);
    }

    let (recent_logs, log_error) = match log_req.send().await {
        Err(e) => (vec![], Some(format!("网络失败: {}", e))),
        Ok(resp) => {
            let log_status = resp.status().as_u16();
            if debug_mode {
                eprintln!("[xiaoma_fetch] log/self status: {}", log_status);
            }
            if log_status == 401 || log_status == 403 {
                (vec![], Some(format!("日志接口 {}", log_status)))
            } else if log_status < 200 || log_status >= 300 {
                (vec![], Some(format!("日志接口 HTTP {}", log_status)))
            } else {
                match resp.json::<serde_json::Value>().await {
                    Err(e) => (vec![], Some(format!("日志解析失败: {}", e))),
                    Ok(log_json) => {
                        if debug_mode {
                            if let Some(obj) = log_json.as_object() {
                                eprintln!("[xiaoma_fetch] log/self keys: {:?}", obj.keys().collect::<Vec<_>>());
                            }
                        }
                        let raw_items = extract_logs(&log_json);
                        if debug_mode && !raw_items.is_empty() {
                            if let Some(obj) = raw_items[0].as_object() {
                                eprintln!("[xiaoma_fetch] log[0] keys: {:?}", obj.keys().collect::<Vec<_>>());
                            }
                        }
                        let logs: Vec<XiaomaLogItem> = raw_items.iter()
                            .map(parse_log_item)
                            .collect();
                        (logs, None)
                    }
                }
            }
        }
    };

    Ok(XiaomaSnapshot {
        wallet,
        recent_logs,
        status: "ok".to_string(),
        log_error,
        timestamp: now_ts,
        debug_url: Some(user_url),
        debug_http_status: Some(user_status),
        debug_resp_keys: Some(top_keys),
        debug_message: None,
    })
}

// ── 开机自启动 ────────────────────────────────────────────────

#[command]
pub fn get_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

#[command]
pub fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if enabled {
        al.enable().map_err(|e| e.to_string())
    } else {
        al.disable().map_err(|e| e.to_string())
    }
}
