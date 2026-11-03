mod commands;
mod desktop;
mod state;
mod tray;
mod watcher;

use std::fs;
use std::path::{Path, PathBuf};

use costline_core::{config, Config};
use tauri::Manager;

pub use state::{AppState, Snapshot};

fn resolve_paths(app: &tauri::App) -> (PathBuf, PathBuf) {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("costline"));
    let _ = fs::create_dir_all(&data_dir);
    (data_dir.join("config.toml"), data_dir.join("state.json"))
}

/// 首次启动时尝试沿用 CLI 版的配置与状态。
fn migrate_legacy(config_path: &Path, state_path: &Path) {
    if !config_path.exists() {
        if let Some(legacy) = dirs::config_dir().map(|dir| dir.join("costline").join("config.toml"))
        {
            if legacy.exists() {
                let _ = fs::copy(legacy, config_path);
            }
        }
    }
    if !state_path.exists() {
        if let Some(legacy) =
            dirs::data_local_dir().map(|dir| dir.join("costline").join("state.json"))
        {
            if legacy.exists() {
                let _ = fs::copy(legacy, state_path);
            }
        }
    }
}

fn load_or_init_config(config_path: &Path) -> (Config, Option<String>) {
    if !config_path.exists() {
        let config = Config::default();
        if let Err(error) = config::save(config_path, &config) {
            return (config, Some(format!("创建配置文件失败：{error:#}")));
        }
        return (config, None);
    }

    match config::load(config_path) {
        Ok(config) => (config, None),
        Err(error) => {
            let backup = config_path
                .with_extension(format!("toml.invalid-{}", chrono::Utc::now().timestamp()));
            let backup_note = if fs::rename(config_path, &backup).is_ok() {
                format!("，已备份为 {}", backup.display())
            } else {
                String::new()
            };
            let config = Config::default();
            let _ = config::save(config_path, &config);
            (
                config,
                Some(format!("配置文件解析失败{backup_note}：{error:#}")),
            )
        }
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let (config_path, state_path) = resolve_paths(app);
            migrate_legacy(&config_path, &state_path);
            let (config, load_error) = load_or_init_config(&config_path);

            app.manage(AppState::new(config_path, state_path, config));
            if let Some(error) = load_error {
                app.state::<AppState>().lock().last_error = Some(error);
            }

            tray::create(app)?;
            watcher::spawn(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::get_snapshot,
            commands::save_config,
            commands::check_now,
            commands::set_watching,
            commands::search_assets,
            commands::test_notify,
            commands::reset_state,
            commands::open_config_dir,
            commands::quit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Costline");
}
