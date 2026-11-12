use costline_core::{
    config, market, monitor, notify, Channel, ChannelResult, Config, NoticeKind, NoticeRecord,
    SearchItem,
};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::desktop;
use crate::state::{AppState, Snapshot};
use crate::tray;
use crate::watcher;

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Config {
    state.lock().config.clone()
}

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> Snapshot {
    state.snapshot()
}

#[tauri::command]
pub async fn save_config(app: AppHandle, config: Config) -> Result<Snapshot, String> {
    let snapshot = {
        let state = app.state::<AppState>();
        config::save(&state.config_path, &config).map_err(|error| format!("{error:#}"))?;
        {
            let mut runtime = state.lock();
            runtime.config = config;
            runtime.last_error = None;
        }
        state.snapshot()
    };
    let _ = app.emit("snapshot", &snapshot);

    // 保存后立刻在后台拉一轮行情，让面板马上反映新配置。
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        watcher::run_check(&handle, true, false).await;
    });

    Ok(snapshot)
}

#[tauri::command]
pub async fn check_now(app: AppHandle) -> Snapshot {
    watcher::run_check(&app, true, false).await
}

#[tauri::command]
pub async fn set_watching(app: AppHandle, watching: bool) -> Snapshot {
    let snapshot = {
        let state = app.state::<AppState>();
        state.lock().watching = watching;
        state.snapshot()
    };
    let _ = app.emit("snapshot", &snapshot);
    if watching {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            watcher::run_check(&handle, true, false).await;
        });
    }
    snapshot
}

#[tauri::command]
pub fn set_realtime(app: AppHandle, realtime: bool) -> Result<Snapshot, String> {
    let snapshot = {
        let state = app.state::<AppState>();
        {
            let mut runtime = state.lock();
            let mut config = runtime.config.clone();
            config.realtime = realtime;
            config::save(&state.config_path, &config).map_err(|error| format!("{error:#}"))?;
            runtime.config = config;
            runtime.last_error = None;
        }
        state.snapshot()
    };
    let _ = app.emit("snapshot", &snapshot);

    // 开启后立刻刷新一轮，避免等待下一个周期。
    if realtime {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            watcher::run_price_refresh(&handle).await;
        });
    }

    Ok(snapshot)
}

#[tauri::command]
pub async fn search_assets(
    state: State<'_, AppState>,
    keyword: String,
) -> Result<Vec<SearchItem>, String> {
    let config = state.lock().config.clone();
    let client = monitor::build_client(&config).map_err(|error| format!("{error:#}"))?;
    market::search_assets(&client, &keyword)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub async fn test_notify(app: AppHandle, config: Config) -> Result<Vec<ChannelResult>, String> {
    for channel in &config.notify.channels {
        channel.validate().map_err(|error| format!("{error:#}"))?;
    }

    let client = monitor::build_client(&config).map_err(|error| format!("{error:#}"))?;
    let notice = monitor::test_notice();

    let mut results = Vec::new();

    // 系统通知始终测试，桌面版触发时会自动发送。
    let sound = config
        .notify
        .channels
        .iter()
        .find_map(|channel| match channel {
            Channel::Desktop { sound } => Some(sound.clone()),
            _ => None,
        })
        .flatten();
    match desktop::send(&app, &notice, sound.as_deref()) {
        Ok(()) => results.push(ChannelResult {
            label: "系统通知".to_string(),
            ok: true,
            error: None,
        }),
        Err(error) => results.push(ChannelResult {
            label: "系统通知".to_string(),
            ok: false,
            error: Some(error),
        }),
    }

    let remote: Vec<Channel> = config
        .notify
        .channels
        .iter()
        .filter(|channel| !matches!(channel, Channel::Desktop { .. }))
        .cloned()
        .collect();
    let remote_results = notify::send_all(&client, &remote, &notice).await;
    results.extend(remote_results.into_iter().map(|result| ChannelResult {
        label: result.label.to_string(),
        ok: result.result.is_ok(),
        error: result.result.err().map(|error| format!("{error:#}")),
    }));

    // 测试消息也进入历史，方便确认哪个渠道真正收到过推送。
    let channels = results
        .iter()
        .map(|result| costline_core::NoticeChannel {
            label: result.label.clone(),
            ok: result.ok,
            error: result.error.clone(),
        })
        .collect::<Vec<_>>();
    if channels.iter().any(|channel| channel.ok) {
        let lock = {
            let state = app.state::<AppState>();
            state.check_lock.clone()
        };
        let _guard = lock.lock().await;
        watcher::record_notice(&app, NoticeKind::Test, &notice, channels);
    }

    Ok(results)
}

#[tauri::command]
pub fn get_notice_history(state: State<'_, AppState>) -> Vec<NoticeRecord> {
    costline_core::State::load(&state.state_path).notices_desc()
}

#[tauri::command]
pub async fn clear_notice_history(app: AppHandle) -> Result<(), String> {
    let (state_path, lock) = {
        let state = app.state::<AppState>();
        (state.state_path.clone(), state.check_lock.clone())
    };
    let _guard = lock.lock().await;
    let mut saved = costline_core::State::load(&state_path);
    saved.clear_notices();
    saved
        .save(&state_path)
        .map_err(|error| format!("清空消息历史失败：{error:#}"))?;
    let _ = app.emit("notice-cleared", ());
    Ok(())
}

#[tauri::command]
pub fn reset_state(app: AppHandle) -> Result<Snapshot, String> {
    let state = app.state::<AppState>();
    // 只清空提醒状态；交割单导入的成交流水和文件缓存必须保留。
    let mut saved = costline_core::State::load(&state.state_path);
    saved.assets.clear();
    saved
        .save(&state.state_path)
        .map_err(|error| format!("重置状态失败：{error:#}"))?;
    let snapshot = state.snapshot();
    let _ = app.emit("snapshot", &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub fn open_config_dir(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let dir = state
        .config_path
        .parent()
        .map(|path| path.to_path_buf())
        .unwrap_or_else(|| state.config_path.clone());
    std::process::Command::new("open")
        .arg(dir)
        .spawn()
        .map_err(|error| format!("打开目录失败：{error}"))?;
    Ok(())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// 下拉面板里的「打开完整面板」，可指定要切换到的 tab。
#[tauri::command]
pub fn open_dashboard(app: AppHandle, tab: Option<String>) {
    if let Some(tab) = tab {
        let _ = app.emit("open-tab", tab);
    }
    tray::show_dashboard(&app);
}

/// 下拉面板失焦或按 Esc 时收起。
#[tauri::command]
pub fn hide_tray_panel(app: AppHandle) {
    tray::hide_panel(&app);
}

/// 下拉面板内容高度变化后，让窗口贴合内容（逻辑像素）。
#[tauri::command]
pub fn resize_tray_panel(app: AppHandle, height: f64) {
    let Some(window) = app.get_webview_window(tray::PANEL_LABEL) else {
        return;
    };
    let height = height.clamp(180.0, 680.0);
    let _ = window.set_size(tauri::LogicalSize::new(380.0, height));
}
