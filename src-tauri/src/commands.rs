use tauri::{AppHandle, Manager, Emitter, command};
use crate::store::{AppSettings, BalanceRecord, read_settings, read_history, write_history};
use crate::credential_store::{SessionData, read_session, write_session, clear_session, mask_cookie};

#[command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    read_settings(&app)
}

#[command]
pub fn save_settings(app: AppHandle, settings: AppSettings) -> AppSettings {
    crate::store::write_settings_locked(&app, &settings);
    settings
}

#[command]
pub fn get_history(app: AppHandle, profile_id: String) -> Vec<BalanceRecord> {
    read_history(&app, &profile_id)
}

#[command]
pub fn append_history(app: AppHandle, profile_id: String, record: BalanceRecord) -> Vec<BalanceRecord> {
    let mut history = read_history(&app, &profile_id);
    history.push(record);
    if history.len() > 200 {
        let drain = history.len() - 200;
        history.drain(0..drain);
    }
    write_history(&app, &profile_id, &history);
    history
}

#[command]
pub fn clear_history(app: AppHandle, profile_id: String) -> Vec<BalanceRecord> {
    write_history(&app, &profile_id, &[]);
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
        // 拖动高频触发，锁内只改坐标两个字段，避免覆盖其他并发写入
        crate::store::update_settings(&app, |s| {
            s.window_x = x;
            s.window_y = y;
        });
    }
}

#[command]
pub async fn move_window_smooth(app: AppHandle, target_x: i32, target_y: i32, duration_ms: u64) {
    if let Some(win) = app.get_webview_window("floating") {
        if let Ok(current_pos) = win.outer_position() {
            let (start_x, start_y) = (current_pos.x, current_pos.y);
            let steps = (duration_ms / 16).max(1); // 60fps
            let dx = (target_x - start_x) as f64 / steps as f64;
            let dy = (target_y - start_y) as f64 / steps as f64;

            for i in 1..=steps {
                let x = start_x + (dx * i as f64) as i32;
                let y = start_y + (dy * i as f64) as i32;
                let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
                tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
            }

            // 确保最终位置精确
            let _ = win.set_position(tauri::PhysicalPosition::new(target_x, target_y));
        }
    }
}

#[command]
pub fn get_window_position(app: AppHandle) -> Result<(i32, i32), String> {
    if let Some(win) = app.get_webview_window("floating") {
        win.outer_position()
            .map(|p| (p.x, p.y))
            .map_err(|e| e.to_string())
    } else {
        Err("floating window not found".to_string())
    }
}

#[command]
pub fn get_primary_monitor_size(app: AppHandle) -> Result<(u32, u32), String> {
    app.primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no primary monitor".to_string())
        .map(|m| {
            let size = m.size();
            (size.width, size.height)
        })
}

#[command]
pub fn get_cursor_position(app: AppHandle) -> Result<(i32, i32), String> {
    app.cursor_position()
        .map(|p| (p.x as i32, p.y as i32))
        .map_err(|e| e.to_string())
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
    // 自动续期：账密重新登录成功后携带新 token（前端回写 profile）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renewed_token: Option<String>,
}

const QUOTA_TO_USD: f64 = 500_000.0;

fn quota_usd(q: f64) -> f64 {
    (q / QUOTA_TO_USD * 1_000_000.0).round() / 1_000_000.0
}

fn fmt_timestamp(ts: u64) -> String {
    use chrono::{TimeZone, Utc};
    // ts 可能是秒或毫秒
    let secs = if ts > 1_000_000_000_000 {
        (ts / 1000) as i64
    } else {
        ts as i64
    };

    // 使用 chrono 转换为东八区时间
    if let Some(dt) = Utc.timestamp_opt(secs, 0).single() {
        // 转换为东八区 (+08:00)
        let local = dt.with_timezone(&chrono::FixedOffset::east_opt(8 * 3600).unwrap());
        local.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        "--".to_string()
    }
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
            renewed_token: None,
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
                renewed_token: None,
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
            renewed_token: None,
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
            renewed_token: None,
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
                renewed_token: None,
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
            renewed_token: None,
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
        renewed_token: None,
    })
}

// ── 极智 API（jizhiapi.site）────────────────────────────────
// 认证：Authorization: Bearer <JWT>
// 用户信息：GET /api/v1/auth/me
// 使用记录：GET /api/v1/usage?start_date=&end_date=&page=1&page_size=100

fn parse_jizhi_log_item(item: &serde_json::Value) -> XiaomaLogItem {
    // 费用：极智界面显示美元金额，字段可能是 cost/amount/quota
    let cost = pick_f64(item, &["cost", "amount", "real_cost", "actual_cost", "money"])
        .or_else(|| pick_f64(item, &["quota", "used_quota"]).map(quota_usd))
        .unwrap_or(0.0);

    let input_tokens = pick_i64(item, &[
        "prompt_tokens", "promptTokens", "input_tokens", "inputTokens", "prompt",
    ]).unwrap_or(0);
    let output_tokens = pick_i64(item, &[
        "completion_tokens", "completionTokens", "output_tokens", "outputTokens", "completion",
    ]).unwrap_or(0);

    let model = pick_str(item, &["model_name", "modelName", "model"])
        .unwrap_or("--").to_string();
    let token_name = pick_str(item, &[
        "token_name", "tokenName", "key_name", "api_key", "name", "channel", "group",
    ]).unwrap_or("--").to_string();

    // 时间：优先尝试解析 ISO 字符串，再尝试数字时间戳
    let (time, ts) = if let Some(time_str) = pick_str(item, &["created_at", "time", "created_time", "date"]) {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
            // ISO 字符串，直接格式化（已包含时区信息）
            let formatted = dt.format("%Y-%m-%d %H:%M:%S").to_string();
            let timestamp = dt.timestamp() as u64;
            (formatted, timestamp)
        } else {
            // 非 ISO 字符串，尝试解析为时间戳
            if let Some(ts_num) = pick_i64(item, &["created_at", "createdAt", "created", "timestamp", "time"]) {
                let ts = ts_num as u64;
                (fmt_timestamp(ts), ts)
            } else {
                (time_str.to_string(), 0)
            }
        }
    } else if let Some(ts_num) = pick_i64(item, &["created_at", "createdAt", "created", "timestamp", "time"]) {
        let ts = ts_num as u64;
        (fmt_timestamp(ts), ts)
    } else {
        ("--".to_string(), 0)
    };

    XiaomaLogItem { time, timestamp: ts, token_name, model, input_tokens, output_tokens, cost }
}

#[command]
pub async fn jizhi_fetch(
    base_url: String,
    bearer_token: String,
    cookie: Option<String>,
    debug_mode: bool,
) -> Result<XiaomaSnapshot, String> {
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let bearer = bearer_token.trim().to_string();
    let cookie = cookie.unwrap_or_default().trim().to_string();
    let base = base_url.trim_end_matches('/').to_string();

    let snap_err = |status: &str, msg: Option<String>, url: Option<String>, http: Option<u16>| XiaomaSnapshot {
        wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
        recent_logs: vec![],
        status: status.to_string(),
        log_error: msg.clone(),
        timestamp: now_ts,
        debug_url: url,
        debug_http_status: http,
        debug_resp_keys: None,
        debug_message: msg,
        renewed_token: None,
    };

    if bearer.is_empty() {
        return Ok(snap_err("cookie_missing", Some("未填写 Bearer Token".into()), None, None));
    }

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| format!("network_error: {}", e))?;

    // ── 1. 用户信息 /api/v1/auth/me ──
    let me_url = format!("{}/api/v1/auth/me?timezone=Etc%2FGMT-8", base);
    let mut req = client.get(&me_url)
        .header("Accept", "application/json, text/plain, */*")
        .header("Authorization", format!("Bearer {}", bearer))
        .header("Referer", format!("{}/dashboard", base))
        .header("Origin", &base);
    if !cookie.is_empty() {
        req = req.header("Cookie", &cookie);
    }

    let me_resp = match req.send().await {
        Err(e) => return Ok(snap_err("network_error", Some(format!("请求失败: {}", e)), Some(me_url), None)),
        Ok(r) => r,
    };
    let me_status = me_resp.status().as_u16();
    if debug_mode {
        eprintln!("[jizhi_fetch] auth/me status: {}", me_status);
    }
    if me_status == 401 || me_status == 403 {
        let body = me_resp.text().await.unwrap_or_default();
        let msg: String = body.chars().take(120).collect();
        return Ok(snap_err("auth_error", Some(if msg.is_empty() { "Token 无效或已过期".into() } else { msg }), Some(me_url), Some(me_status)));
    }
    if me_status < 200 || me_status >= 300 {
        let body = me_resp.text().await.unwrap_or_default();
        let preview: String = body.chars().take(120).collect();
        return Ok(snap_err("network_error", Some(format!("HTTP {}: {}", me_status, preview)), Some(me_url), Some(me_status)));
    }

    let me_json: serde_json::Value = match me_resp.json().await {
        Err(e) => return Ok(snap_err("parse_error", Some(format!("JSON 解析失败: {}", e)), Some(me_url), Some(me_status))),
        Ok(v) => v,
    };

    let top_keys = me_json.as_object()
        .map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    if debug_mode {
        eprintln!("[jizhi_fetch] auth/me keys: {}", top_keys);
    }

    // 数据可能在 data / user 字段里，或顶层
    let d = me_json.get("data")
        .or_else(|| me_json.get("user"))
        .unwrap_or(&me_json);
    if debug_mode {
        if let Some(o) = d.as_object() {
            eprintln!("[jizhi_fetch] me data keys: {:?}", o.keys().collect::<Vec<_>>());
        }
    }

    // 余额：极智界面显示美元，字段名尝试多种
    let balance = pick_f64(d, &["balance", "remain", "remaining", "credit", "money"])
        .or_else(|| pick_f64(d, &["quota", "remain_quota"]).map(quota_usd))
        .unwrap_or(0.0);
    let total_cost = pick_f64(d, &["total_cost", "used", "used_money", "total_used", "spent"])
        .or_else(|| pick_f64(d, &["used_quota"]).map(quota_usd))
        .unwrap_or(0.0);
    let request_count = pick_i64(d, &["request_count", "total_requests", "requests", "count"])
        .unwrap_or(0);

    let wallet = XiaomaWallet { balance, total_cost, request_count };

    // ── 2. 使用记录 /api/v1/usage ──
    let start_ymd = fmt_date(now_ts.saturating_sub(86400 * 7));
    let end_ymd = fmt_date(now_ts);
    let usage_url = format!(
        "{}/api/v1/usage?start_date={}&end_date={}&page=1&page_size=100&timezone=Etc%2FGMT-8",
        base, start_ymd, end_ymd
    );
    let mut ureq = client.get(&usage_url)
        .header("Accept", "application/json, text/plain, */*")
        .header("Authorization", format!("Bearer {}", bearer))
        .header("Referer", format!("{}/dashboard", base))
        .header("Origin", &base);
    if !cookie.is_empty() {
        ureq = ureq.header("Cookie", &cookie);
    }

    let (recent_logs, log_error) = match ureq.send().await {
        Err(e) => (vec![], Some(format!("网络失败: {}", e))),
        Ok(resp) => {
            let us = resp.status().as_u16();
            if debug_mode {
                eprintln!("[jizhi_fetch] usage status: {}", us);
            }
            if us < 200 || us >= 300 {
                (vec![], Some(format!("使用记录接口 HTTP {}", us)))
            } else {
                match resp.json::<serde_json::Value>().await {
                    Err(e) => (vec![], Some(format!("使用记录解析失败: {}", e))),
                    Ok(uj) => {
                        if debug_mode {
                            if let Some(o) = uj.as_object() {
                                eprintln!("[jizhi_fetch] usage keys: {:?}", o.keys().collect::<Vec<_>>());
                            }
                        }
                        let raw = extract_logs(&uj);
                        if debug_mode && !raw.is_empty() {
                            if let Some(o) = raw[0].as_object() {
                                eprintln!("[jizhi_fetch] usage[0] keys: {:?}", o.keys().collect::<Vec<_>>());
                            }
                        }
                        let logs: Vec<XiaomaLogItem> = raw.iter().map(parse_jizhi_log_item).collect();
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
        debug_url: Some(me_url),
        debug_http_status: Some(me_status),
        debug_resp_keys: Some(top_keys),
        debug_message: None,
        renewed_token: None,
    })
}

// ── 极智新版 API（jizhiapi.site 2026-08 更新后）──────────────
// 认证：Authorization: Bearer <JWT>
// 用户信息：GET /api/v1/auth/me → data.balance
// 使用记录：GET /api/v1/usage → data.items[]，actual_cost 为实际费用

fn parse_jizhi_new_log_item(item: &serde_json::Value) -> XiaomaLogItem {
    let cost = pick_f64(item, &["actual_cost", "total_cost", "cost"]).unwrap_or(0.0);
    let input_tokens = pick_i64(item, &["input_tokens", "prompt_tokens"]).unwrap_or(0);
    let output_tokens = pick_i64(item, &["output_tokens", "completion_tokens"]).unwrap_or(0);
    let model = pick_str(item, &["model"]).unwrap_or("--").to_string();
    let token_name = pick_str(item, &["api_key.name", "group.name", "group"]).unwrap_or("--").to_string();

    // created_at 是 ISO 字符串 "2026-08-20T20:12:46.035872+08:00"
    let time_str = pick_str(item, &["created_at"]).unwrap_or("--");
    let (time, ts) = if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
        // 直接格式化为本地时间字符串，不要再调用 fmt_timestamp（它会重复加时区）
        let formatted = dt.format("%Y-%m-%d %H:%M:%S").to_string();
        let timestamp = dt.timestamp() as u64;
        (formatted, timestamp)
    } else {
        (time_str.to_string(), 0)
    };

    XiaomaLogItem { time, timestamp: ts, token_name, model, input_tokens, output_tokens, cost }
}

// ── 极智自动登录 / Token 自动续期 ────────────────────────────

use std::sync::atomic::{AtomicI64, Ordering};

/// 续期失败后的冷却截止时间戳（秒）。5 分钟内不再自动撞登录接口。
static JIZHI_RENEW_COOLDOWN_UNTIL: AtomicI64 = AtomicI64::new(0);

fn jizhi_renew_allowed() -> bool {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    now >= JIZHI_RENEW_COOLDOWN_UNTIL.load(Ordering::Relaxed)
}

fn jizhi_renew_mark_failure() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    JIZHI_RENEW_COOLDOWN_UNTIL.store(now + 300, Ordering::Relaxed);
}

/// 从 settings 读账号 + 凭据存储（keyring→DPAPI 文件兜底）读密码；任一缺失返回 None。
/// settings 里账号丢失时（曾被并发覆盖等），尝试从加密文件镜像恢复并回写 settings。
fn load_jizhi_credentials(app: &AppHandle, profile_id: &str) -> Option<(String, String)> {
    let settings = read_settings(app);
    let profile = settings.profiles.iter().find(|p| p.id == profile_id)?;
    let mut username = profile
        .jizhi_username
        .clone()
        .unwrap_or_default()
        .trim()
        .to_string();

    // 兜底自愈：settings 丢账号但加密文件镜像还在 → 恢复账号回写 settings
    if username.is_empty() {
        if let Some(mirrored) = crate::credential_store::load_jizhi_username(app, profile_id) {
            username = mirrored;
            let pid = profile_id.to_string();
            let uname = username.clone();
            crate::store::update_settings(app, move |s| {
                if let Some(p) = s.profiles.iter_mut().find(|p| p.id == pid) {
                    p.jizhi_username = Some(uname);
                }
            });
        }
    }

    if username.is_empty() {
        return None;
    }
    let password = crate::credential_store::load_jizhi_password(app, profile_id)?;
    if password.is_empty() {
        return None;
    }
    Some((username, password))
}

/// 把续期得到的新 token 写回 profile（settings.json，锁内读改写防并发覆盖）
fn persist_profile_token(app: &AppHandle, profile_id: &str, token: &str) {
    crate::store::update_settings(app, |s| {
        if let Some(p) = s.profiles.iter_mut().find(|p| p.id == profile_id) {
            p.api_token = token.to_string();
        }
    });
}

fn persist_profile_credentials(app: &AppHandle, profile_id: &str, username: &str, token: &str) {
    crate::store::update_settings(app, |s| {
        if let Some(p) = s.profiles.iter_mut().find(|p| p.id == profile_id) {
            p.jizhi_username = Some(username.to_string());
            p.api_token = token.to_string();
        }
    });
}

fn mask_token_preview(token: &str) -> String {
    if token.len() <= 12 {
        "***".to_string()
    } else {
        format!("{}…{}", &token[..6], &token[token.len() - 4..])
    }
}

/// 从登录响应提取 token：顶层 / data / data.data 里的 token / access_token / jwt
fn extract_login_token(json: &serde_json::Value) -> Option<String> {
    let data = json.get("data").unwrap_or(json);
    let data2 = data.get("data").unwrap_or(data);
    for c in [json, data, data2] {
        if let Some(t) = pick_str(c, &["token", "access_token", "jwt"]) {
            if !t.is_empty() {
                return Some(t.to_string());
            }
        }
    }
    None
}

/// 自适应登录极智：多组 接口路径 × 请求体格式 顺序尝试，命中即返回 JWT。
/// 未抓包确认过登录接口，按极智新版 API 风格穷举常见组合。
async fn jizhi_auto_login(base: &str, username: &str, password: &str, debug_mode: bool) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("client: {}", e))?;

    let paths = ["/api/v1/auth/login", "/api/user/login"];
    let bodies = [
        serde_json::json!({ "username": username, "password": password }),
        serde_json::json!({ "email": username, "password": password }),
    ];

    let mut last_status = 0u16;
    let mut last_info = String::from("未尝试任何请求");

    for path in paths {
        let url = format!("{}{}", base, path);
        for body in &bodies {
            let resp = match client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .json(body)
                .send()
                .await
            {
                Ok(r) => r,
                // 网络失败与路径/字段组合无关，直接终止
                Err(e) => return Err(format!("网络请求失败: {}", e)),
            };
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            let preview: String = text.chars().take(150).collect();

            if debug_mode {
                eprintln!("[jizhi_login] POST {} -> {} {}", url, status, preview);
            }

            if (200..300).contains(&status) {
                match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(json) => {
                        if let Some(t) = extract_login_token(&json) {
                            return Ok(t);
                        }
                        let keys = json
                            .as_object()
                            .map(|o| o.keys().cloned().collect::<Vec<_>>().join(", "))
                            .unwrap_or_else(|| "(非对象)".into());
                        last_status = status;
                        last_info = format!("响应 2xx 但未找到 token 字段（顶层字段: {}）", keys);
                    }
                    Err(_) => {
                        last_status = status;
                        last_info = format!("响应非 JSON: {}", preview);
                    }
                }
            } else {
                last_status = status;
                last_info = preview;
            }
        }
    }

    Err(format!("HTTP {} {}", last_status, last_info))
}

/// 用 keyring 账密自动登录。
/// Ok(Some(token)) = 登录成功；Ok(None) = 未配置账密（静默跳过）；Err(msg) = 配置了但登录失败/冷却中。
async fn try_jizhi_renew(app: &AppHandle, profile_id: &str, base: &str, debug_mode: bool) -> Result<Option<String>, String> {
    let Some((username, password)) = load_jizhi_credentials(app, profile_id) else {
        return Ok(None);
    };
    if !jizhi_renew_allowed() {
        return Err("自动登录冷却中（上次登录失败，5 分钟后自动重试），也可检查账号密码后手动测试连接".to_string());
    }
    match jizhi_auto_login(base, &username, &password, debug_mode).await {
        Ok(token) => Ok(Some(token)),
        Err(e) => {
            jizhi_renew_mark_failure();
            Err(format!("自动登录失败（请检查账号密码）：{}", e))
        }
    }
}

#[command]
pub async fn jizhi_new_fetch(
    app: AppHandle,
    profile_id: String,
    base_url: String,
    bearer_token: String,
    debug_mode: bool,
) -> Result<XiaomaSnapshot, String> {
    let base = base_url.trim_end_matches('/').to_string();

    // ── 有 token：直接请求；401 时尝试自动续期后重试 ──
    let bearer = bearer_token.trim().to_string();
    if !bearer.is_empty() {
        let snap = jizhi_new_fetch_inner(&base, &bearer, debug_mode).await?;
        if snap.status == "auth_error" {
            match try_jizhi_renew(&app, &profile_id, &base, debug_mode).await {
                Ok(Some(token)) => {
                    let mut snap2 = jizhi_new_fetch_inner(&base, &token, debug_mode).await?;
                    persist_profile_token(&app, &profile_id, &token);
                    snap2.renewed_token = Some(token);
                    return Ok(snap2);
                }
                Ok(None) => return Ok(snap),
                Err(msg) => {
                    let mut s = snap;
                    s.log_error = Some(msg);
                    return Ok(s);
                }
            }
        }
        return Ok(snap);
    }

    // ── 无 token：有账密则自动登录拉取 ──
    match try_jizhi_renew(&app, &profile_id, &base, debug_mode).await {
        Ok(Some(token)) => {
            let mut snap = jizhi_new_fetch_inner(&base, &token, debug_mode).await?;
            persist_profile_token(&app, &profile_id, &token);
            snap.renewed_token = Some(token);
            return Ok(snap);
        }
        Ok(None) => {}
        Err(msg) => {
            let now_ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            return Ok(XiaomaSnapshot {
                wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
                recent_logs: vec![],
                status: "auth_error".to_string(),
                log_error: Some(msg),
                timestamp: now_ts,
                debug_url: None,
                debug_http_status: None,
                debug_resp_keys: None,
                debug_message: None,
                renewed_token: None,
            });
        }
    }

    // ── token 和账密都没有 ──
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    Ok(XiaomaSnapshot {
        wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
        recent_logs: vec![],
        status: "cookie_missing".to_string(),
        log_error: Some("未填写 Token，也未配置自动登录账号；请到设置页填写".to_string()),
        timestamp: now_ts,
        debug_url: None,
        debug_http_status: None,
        debug_resp_keys: None,
        debug_message: None,
        renewed_token: None,
    })
}

async fn jizhi_new_fetch_inner(base: &str, bearer: &str, debug_mode: bool) -> Result<XiaomaSnapshot, String> {
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let base = base.trim_end_matches('/');

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("client: {}", e))?;

    // 1. 用户信息
    let me_url = format!("{}/api/v1/auth/me", base);
    let me_req = client.get(&me_url)
        .header("Authorization", format!("Bearer {}", bearer))
        .header("Accept", "application/json");

    let me_resp = me_req.send().await.map_err(|e| format!("network_error: {}", e))?;
    let me_status = me_resp.status().as_u16();

    if debug_mode {
        eprintln!("[jizhi_new_fetch] /api/v1/auth/me status: {}", me_status);
    }

    if me_status == 401 || me_status == 403 {
        return Ok(XiaomaSnapshot {
            wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
            recent_logs: vec![],
            status: "auth_error".to_string(),
            log_error: Some(format!("HTTP {}", me_status)),
            timestamp: now_ts,
            debug_url: Some(me_url),
            debug_http_status: Some(me_status),
            debug_resp_keys: None,
            debug_message: Some(format!("HTTP {}", me_status)),
            renewed_token: None,
        });
    }

    let me_json: serde_json::Value = me_resp.json().await.map_err(|e| format!("parse_error: {}", e))?;
    let data = me_json.get("data").unwrap_or(&me_json);
    let balance = pick_f64(data, &["balance"]).unwrap_or(0.0);
    // 尝试从用户信息获取总消耗，如果没有则设为 0（等使用记录接口获取）
    let user_total_cost = pick_f64(data, &["total_cost", "used", "used_money", "total_used"])
        .or_else(|| pick_f64(data, &["used_quota"]).map(quota_usd));

    // 2. 使用记录
    let usage_url = format!("{}/api/v1/usage?page=1&page_size=20", base);
    let usage_req = client.get(&usage_url)
        .header("Authorization", format!("Bearer {}", bearer))
        .header("Accept", "application/json");

    let (recent_logs, log_error, computed_total_cost, request_count) = match usage_req.send().await {
        Err(e) => (vec![], Some(format!("网络失败: {}", e)), 0.0, 0),
        Ok(resp) => {
            let us = resp.status().as_u16();
            if us < 200 || us >= 300 {
                (vec![], Some(format!("使用记录 HTTP {}", us)), 0.0, 0)
            } else {
                match resp.json::<serde_json::Value>().await {
                    Err(e) => (vec![], Some(format!("解析失败: {}", e)), 0.0, 0),
                    Ok(uj) => {
                        // 尝试从响应顶层获取 total 或 pagination 信息
                        let total_from_resp = pick_f64(&uj, &["total_cost", "total_amount"]);
                        let total_count = pick_i64(&uj, &["total", "total_count"]).unwrap_or(0);

                        let raw = extract_logs(&uj);
                        let logs: Vec<XiaomaLogItem> = raw.iter().map(parse_jizhi_new_log_item).collect();
                        // 从使用记录累加（仅当前页的，可能不完整）
                        let computed_cost: f64 = logs.iter().map(|log| log.cost).sum();
                        let request_count = if total_count > 0 { total_count } else { logs.len() as i64 };
                        (logs, None, total_from_resp.unwrap_or(computed_cost), request_count)
                    }
                }
            }
        }
    };

    // 优先使用用户信息接口的总消耗，如果没有则使用计算值
    let total_cost = user_total_cost.unwrap_or(computed_total_cost);

    let wallet = XiaomaWallet {
        balance,
        total_cost,
        request_count,
    };

    Ok(XiaomaSnapshot {
        wallet,
        recent_logs,
        status: "ok".to_string(),
        log_error,
        timestamp: now_ts,
        debug_url: Some(me_url),
        debug_http_status: Some(me_status),
        debug_resp_keys: None,
        debug_message: None,
        renewed_token: None,
    })
}

// ── 极智账密管理 / 手动登录测试 ──────────────────────────────

/// 保存极智密码到双存储（凭据管理器 + DPAPI 加密文件，密码不进 settings.json）。
/// 空串 = 清除该配置的全部极智凭据。
#[command]
pub fn jizhi_save_password(app: AppHandle, profile_id: String, password: String) -> Result<(), String> {
    if password.is_empty() {
        crate::credential_store::save_jizhi_credentials(&app, &profile_id, "", "")
    } else {
        crate::credential_store::save_jizhi_password(&app, &profile_id, &password)
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JizhiLoginResult {
    /// 掩码预览（不含完整 token）
    pub token_masked: String,
    pub balance: f64,
}

/// 手动登录测试：自适应登录 → 验证 → 密码入凭据管理器、账号+新 token 落盘 settings。
async fn jizhi_fetch_balance(base: &str, bearer: &str) -> Option<f64> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let resp = client
        .get(format!("{}/api/v1/auth/me", base))
        .header("Authorization", format!("Bearer {}", bearer))
        .header("Accept", "application/json")
        .send()
        .await
        .ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    let data = json.get("data").unwrap_or(&json);
    pick_f64(data, &["balance"])
}

#[command]
pub async fn jizhi_login(
    app: AppHandle,
    profile_id: String,
    base_url: String,
    username: String,
    password: String,
    debug_mode: bool,
) -> Result<JizhiLoginResult, String> {
    let base = base_url.trim_end_matches('/').to_string();
    let username = username.trim().to_string();
    if username.is_empty() || password.is_empty() {
        return Err("账号或密码为空".to_string());
    }

    let token = jizhi_auto_login(&base, &username, &password, debug_mode).await?;
    let balance = jizhi_fetch_balance(&base, &token).await.unwrap_or(0.0);

    // 密码双存储（keyring + DPAPI 加密文件）；账号 + 新 token 写回 settings
    crate::credential_store::save_jizhi_credentials(&app, &profile_id, &username, &password)?;
    persist_profile_credentials(&app, &profile_id, &username, &token);

    // 登录成功说明之前若有冷却记录已失效，清除冷却
    JIZHI_RENEW_COOLDOWN_UNTIL.store(0, Ordering::Relaxed);

    Ok(JizhiLoginResult {
        token_masked: mask_token_preview(&token),
        balance,
    })
}

/// 前端收到 renewed_token 后回写 profile（也可由 Rust 侧直接 persist，此 command 兜底同步用）
#[command]
pub fn update_profile_token(app: AppHandle, profile_id: String, token: String) {
    persist_profile_token(&app, &profile_id, &token);
}

/// 悬浮窗切换当前监控的中转站：锁内只改 activeProfileId，
/// 不让悬浮窗用内存里的旧 profiles 全量覆盖设置页刚保存的配置（曾导致账号丢失）
#[command]
pub fn set_active_profile(app: AppHandle, profile_id: String) {
    crate::store::update_settings(&app, move |s| {
        s.active_profile_id = profile_id;
    });
}

// ── X网站 API（x-llm.net）─────────────────────────────────────
// 认证：Authorization: Bearer <JWT>
// 用户信息：返回 data.quota（剩余额度，单位 1/500000 美元）、data.used_quota
// 使用记录：GET /api/log/self → data.items[]，quota 为费用（同单位），created_at 为 Unix 秒

fn parse_xllm_log_item(item: &serde_json::Value) -> XiaomaLogItem {
    let raw_quota = pick_f64(item, &["quota"]).unwrap_or(0.0);
    let cost = quota_usd(raw_quota);
    let input_tokens = pick_i64(item, &["prompt_tokens", "input_tokens"]).unwrap_or(0);
    let output_tokens = pick_i64(item, &["completion_tokens", "output_tokens"]).unwrap_or(0);
    let model = pick_str(item, &["model_name", "model"]).unwrap_or("--").to_string();
    let token_name = pick_str(item, &["token_name", "username"]).unwrap_or("--").to_string();

    // 时间：优先尝试解析 ISO 字符串，再尝试数字时间戳
    let (time, ts) = if let Some(time_str) = pick_str(item, &["created_at", "time"]) {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(time_str) {
            // ISO 字符串，直接格式化（已包含时区信息）
            let formatted = dt.format("%Y-%m-%d %H:%M:%S").to_string();
            let timestamp = dt.timestamp() as u64;
            (formatted, timestamp)
        } else if let Some(ts_num) = pick_i64(item, &["created_at", "timestamp"]) {
            let ts = ts_num as u64;
            (fmt_timestamp(ts), ts)
        } else {
            (time_str.to_string(), 0)
        }
    } else if let Some(ts_num) = pick_i64(item, &["created_at", "timestamp"]) {
        let ts = ts_num as u64;
        (fmt_timestamp(ts), ts)
    } else {
        ("--".to_string(), 0)
    };

    XiaomaLogItem { time, timestamp: ts, token_name, model, input_tokens, output_tokens, cost }
}

#[command]
pub async fn xllm_fetch(
    base_url: String,
    bearer_token: String,
    debug_mode: bool,
) -> Result<XiaomaSnapshot, String> {
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let bearer = bearer_token.trim();
    let base = base_url.trim_end_matches('/');

    if bearer.is_empty() {
        return Ok(XiaomaSnapshot {
            wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
            recent_logs: vec![],
            status: "cookie_missing".to_string(),
            log_error: Some("未填写 Bearer Token".to_string()),
            timestamp: now_ts,
            debug_url: None,
            debug_http_status: None,
            debug_resp_keys: None,
            debug_message: Some("未填写 Bearer Token".to_string()),
            renewed_token: None,
        });
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("client: {}", e))?;

    // 1. 用户信息（X 网站接口路径待确认，先用通用的 /api/user）
    let user_url = format!("{}/api/user", base);
    let user_req = client.get(&user_url)
        .header("Authorization", format!("Bearer {}", bearer))
        .header("Accept", "application/json");

    let user_resp = user_req.send().await.map_err(|e| format!("network_error: {}", e))?;
    let user_status = user_resp.status().as_u16();

    if debug_mode {
        eprintln!("[xllm_fetch] /api/user status: {}", user_status);
    }

    if user_status == 401 || user_status == 403 {
        return Ok(XiaomaSnapshot {
            wallet: XiaomaWallet { balance: 0.0, total_cost: 0.0, request_count: 0 },
            recent_logs: vec![],
            status: "auth_error".to_string(),
            log_error: Some(format!("HTTP {}", user_status)),
            timestamp: now_ts,
            debug_url: Some(user_url),
            debug_http_status: Some(user_status),
            debug_resp_keys: None,
            debug_message: Some(format!("HTTP {}", user_status)),
            renewed_token: None,
        });
    }

    let user_json: serde_json::Value = user_resp.json().await.map_err(|e| format!("parse_error: {}", e))?;
    let data = user_json.get("data").unwrap_or(&user_json);
    let remain = pick_f64(data, &["quota"]).unwrap_or(0.0);
    let used = pick_f64(data, &["used_quota"]).unwrap_or(0.0);
    let req_count = pick_i64(data, &["request_count"]).unwrap_or(0);

    let wallet = XiaomaWallet {
        balance: quota_usd(remain),
        total_cost: quota_usd(used),
        request_count: req_count,
    };

    // 2. 使用记录
    let end_ts = now_ts;
    let start_ts = end_ts.saturating_sub(86400 * 7);
    let log_url = format!("{}/api/log/self?p=1&page_size=20&start_timestamp={}&end_timestamp={}", base, start_ts, end_ts);
    let log_req = client.get(&log_url)
        .header("Authorization", format!("Bearer {}", bearer))
        .header("Accept", "application/json");

    let (recent_logs, log_error) = match log_req.send().await {
        Err(e) => (vec![], Some(format!("网络失败: {}", e))),
        Ok(resp) => {
            let ls = resp.status().as_u16();
            if ls < 200 || ls >= 300 {
                (vec![], Some(format!("日志 HTTP {}", ls)))
            } else {
                match resp.json::<serde_json::Value>().await {
                    Err(e) => (vec![], Some(format!("解析失败: {}", e))),
                    Ok(lj) => {
                        let raw = extract_logs(&lj);
                        let logs: Vec<XiaomaLogItem> = raw.iter().map(parse_xllm_log_item).collect();
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
        debug_resp_keys: None,
        debug_message: None,
        renewed_token: None,
    })
}

// 时间戳(秒) → YYYY-MM-DD（东八区）
fn fmt_date(ts: u64) -> String {
    let secs_local = ts + 8 * 3600;
    let days = secs_local / 86400;
    let (y, mo, d) = days_from_epoch(days);
    format!("{:04}-{:02}-{:02}", y, mo, d)
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

    let result = if enabled {
        al.enable()
    } else {
        al.disable()
    };

    result.map_err(|e| {
        let err_str = e.to_string();
        // Windows error 5 = Access Denied
        if err_str.contains("os error 5") || err_str.contains("Access is denied") {
            format!(
                "开机启动设置失败：权限不足。\n\n可能的解决方案：\n\
                1. 尝试以管理员身份运行本程序\n\
                2. 检查防病毒软件是否拦截了注册表修改\n\
                3. 暂时关闭此选项，手动添加开机启动项\n\n\
                技术细节: {}",
                err_str
            )
        } else {
            format!("开机启动设置失败：{}", err_str)
        }
    })
}
