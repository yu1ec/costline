use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

use crate::commands;
use crate::state::AppState;

pub fn create(app: &mut tauri::App) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "打开面板", true, None::<&str>)?;
    let check = MenuItem::with_id(app, "check", "立即检查", true, None::<&str>)?;
    let watch = MenuItem::with_id(app, "watch", "暂停 / 恢复盯盘", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Costline", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &check, &watch, &separator, &quit])?;

    let icon = tauri::include_image!("icons/tray.png");

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Costline 盯盘")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_window(app),
            "check" => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    commands::check_now(handle).await;
                });
            }
            "watch" => {
                let watching = {
                    let state = app.state::<AppState>();
                    let mut runtime = state.lock();
                    runtime.watching = !runtime.watching;
                    runtime.watching
                };
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    commands::set_watching(handle, watching).await;
                });
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
