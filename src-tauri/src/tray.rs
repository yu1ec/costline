use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, Rect, WebviewWindow,
};

use crate::commands;
use crate::state::AppState;

/// 托盘下拉面板的窗口 label，见 tauri.conf.json。
pub const PANEL_LABEL: &str = "tray-panel";

/// 面板因失焦隐藏的时间。macOS 上点击托盘图标的顺序是「面板失焦 -> 托盘 click」，
/// 如果不做保护，刚被失焦收起的面板会被同一次点击立刻重新弹出。
static LAST_HIDDEN_MS: AtomicU64 = AtomicU64::new(0);
const REOPEN_GUARD_MS: u64 = 260;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

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
            "show" => show_dashboard(app),
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
                rect,
                ..
            } = event
            {
                toggle_panel(tray.app_handle(), rect);
            }
        })
        .build(app)?;

    Ok(())
}

/// 左键点击托盘：已展开则收起，否则贴着托盘图标弹出。
fn toggle_panel(app: &AppHandle, rect: Rect) {
    let Some(window) = app.get_webview_window(PANEL_LABEL) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        hide_panel(app);
        return;
    }

    // 刚刚被这次点击的失焦事件收起，不要再弹出来。
    if now_ms().saturating_sub(LAST_HIDDEN_MS.load(Ordering::Relaxed)) < REOPEN_GUARD_MS {
        return;
    }

    place_panel(&window, rect);
    let _ = window.show();
    let _ = window.set_focus();
}

/// 收起下拉面板（失焦、Esc、打开完整面板时复用）。
pub fn hide_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(PANEL_LABEL) {
        let _ = window.hide();
    }
    LAST_HIDDEN_MS.store(now_ms(), Ordering::Relaxed);
}

/// 显示完整面板，同时收起下拉面板。
pub fn show_dashboard(app: &AppHandle) {
    hide_panel(app);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 让下拉面板居中贴在托盘图标下方，并限制在当前显示器的工作区内。
fn place_panel(window: &WebviewWindow, rect: Rect) {
    let scale = window.scale_factor().unwrap_or(1.0);
    let icon = rect.position.to_physical::<f64>(scale);
    let icon_size = rect.size.to_physical::<f64>(scale);
    let (width, height) = window
        .outer_size()
        .map(|size| (size.width as f64, size.height as f64))
        .unwrap_or((380.0 * scale, 440.0 * scale));

    let gap = 6.0 * scale;
    let margin = 8.0 * scale;
    let icon_center = icon.x + icon_size.width / 2.0;
    let mut x = icon_center - width / 2.0;
    let mut y = icon.y + icon_size.height + gap;

    if let Ok(Some(monitor)) = window.monitor_from_point(icon_center, icon.y + 1.0) {
        let area = monitor.work_area();
        let left = area.position.x as f64;
        let right = left + area.size.width as f64;
        let top = area.position.y as f64;
        let bottom = top + area.size.height as f64;

        x = x.min(right - width - margin).max(left + margin);
        if y + height > bottom {
            // 下方空间不足时向上弹出（macOS 菜单栏在顶部，正常不会走到这里）。
            y = (icon.y - height - gap).max(top + margin);
        }
    }

    let _ = window.set_position(PhysicalPosition::new(
        x.round() as i32,
        y.round() as i32,
    ));
}
