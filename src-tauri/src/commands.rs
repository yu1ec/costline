use costline_core::{config, monitor, notify, Channel, ChannelResult, Config};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::desktop;
use crate::state::{AppState, Snapshot};
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

    Ok(results)
}

#[tauri::command]
pub fn reset_state(app: AppHandle) -> Result<Snapshot, String> {
    let state = app.state::<AppState>();
    if state.state_path.exists() {
        std::fs::remove_file(&state.state_path)
            .map_err(|error| format!("删除状态文件失败：{error}"))?;
    }
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
