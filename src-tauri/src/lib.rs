use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};

mod commands;
mod credential_store;
mod store;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            #[cfg(debug_assertions)]
            eprintln!("[App] setup start");

            let settings = store::read_settings(app.handle());

            let (x, y) = if settings.window_x >= 0 && settings.window_y >= 0 {
                (settings.window_x, settings.window_y)
            } else {
                if let Some(monitor) = app.primary_monitor()? {
                    let size = monitor.size();
                    ((size.width as i32) - 260, 20)
                } else {
                    (1670, 20)
                }
            };

            let float_url = WebviewUrl::App("index.html?page=floating".into());
            #[cfg(debug_assertions)]
            eprintln!("[App] create floating window at ({}, {})", x, y);

            let win = WebviewWindowBuilder::new(app, "floating", float_url)
                .title("API Monitor")
                .inner_size(240.0, 232.0)
                .position(x as f64, y as f64)
                .decorations(false)
                .transparent(true)
                .always_on_top(settings.always_on_top)
                .skip_taskbar(true)
                .resizable(false)
                .shadow(false)
                .visible(true)
                .build()?;

            #[cfg(debug_assertions)]
            eprintln!("[App] floating window created");

            build_tray(app)?;
            #[cfg(debug_assertions)]
            eprintln!("[App] create tray");

            let _ = win;
            #[cfg(debug_assertions)]
            eprintln!("[App] setup done");
            Ok(())
        })
        .on_window_event(|win, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if win.label() == "floating" {
                    api.prevent_close();
                    let _ = win.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_history,
            commands::append_history,
            commands::clear_history,
            commands::open_url,
            commands::show_window,
            commands::hide_window,
            commands::set_always_on_top,
            commands::move_window,
            commands::move_window_smooth,
            commands::get_window_position,
            commands::get_primary_monitor_size,
            commands::get_cursor_position,
            commands::notify_low_balance,
            commands::open_settings_window,
            commands::close_settings_window,
            commands::broadcast_settings_changed,
            commands::xiaoma_fetch,
            commands::save_session_cookie,
            commands::notify_session_captured,
            commands::get_session_status,
            commands::get_session_cookie,
            commands::clear_session_cookie,
            commands::get_autostart_enabled,
            commands::set_autostart_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn build_tray(app: &mut App) -> tauri::Result<()> {
    let handle = app.handle().clone();
    let handle2 = handle.clone();
    let handle3 = handle.clone();
    let handle4 = handle.clone();
    let handle5 = handle.clone();

    let show   = MenuItem::with_id(app, "show",    "显示悬浮窗", true, None::<&str>)?;
    let refresh= MenuItem::with_id(app, "refresh", "刷新余额",   true, None::<&str>)?;
    let sep1   = tauri::menu::PredefinedMenuItem::separator(app)?;
    let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let open_web = MenuItem::with_id(app, "open_web", "打开中转站", true, None::<&str>)?;
    let sep2   = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit   = MenuItem::with_id(app, "quit",   "退出",      true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &refresh, &sep1, &settings_item, &open_web, &sep2, &quit])?;

    TrayIconBuilder::new()
        .icon(Image::from_bytes(include_bytes!("../icons/32x32.png"))?)
        .menu(&menu)
        .tooltip("API Monitor")
        .on_menu_event(move |_tray, event| {
            match event.id().as_ref() {
                "show" => {
                    if let Some(w) = handle.get_webview_window("floating") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "refresh" => {
                    if let Some(w) = handle2.get_webview_window("floating") {
                        let _ = w.emit("cmd:refresh", ());
                    }
                }
                "settings" => {
                    let h = handle3.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = commands::open_settings_window(h).await;
                    });
                }
                "open_web" => {
                    let s = store::read_settings(&handle3);
                    let url = s.base_url.trim().to_string();
                    if !url.is_empty() {
                        let _ = open::that(url);
                    }
                }
                "quit" => {
                    handle4.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if let Some(w) = handle5.get_webview_window("floating") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
