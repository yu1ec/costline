use costline_core::notify::Notice;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// macOS 开发模式下，tauri-plugin-notification 会把通知身份伪装成
/// `com.apple.Terminal`。如果系统设置里关闭了 Terminal 的通知，测试通知就完全
/// 看不到。若本机安装过 Costline，则提前锁定到自己的 bundle id，让开发模式的
/// 通知也以 Costline 身份显示。
///
/// `set_application` 只会生效一次，所以必须赶在第一条通知之前调用。
#[cfg(target_os = "macos")]
pub fn pin_identity(app: &tauri::App) {
    if !tauri::is_dev() {
        return;
    }

    let identifier = app.config().identifier.as_str();
    let product = app.package_info().name.as_str();
    let mut bundles =
        vec![std::path::PathBuf::from("/Applications").join(format!("{product}.app"))];
    if let Some(home) = dirs::home_dir() {
        bundles.push(home.join("Applications").join(format!("{product}.app")));
    }
    let installed = bundles
        .iter()
        .any(|bundle| bundle.join("Contents/Info.plist").is_file());
    if !installed {
        // 没装正式版时保持插件默认行为（由 Terminal 代发）。
        return;
    }

    if let Err(error) = notify_rust::set_application(identifier) {
        eprintln!("锁定 macOS 通知身份失败（回退到 Terminal）：{error}");
    }
}

/// 发送由应用自身发出的系统通知（通知中心里显示为 Costline）。
pub fn send(app: &AppHandle, notice: &Notice, sound: Option<&str>) -> Result<(), String> {
    let mut builder = app
        .notification()
        .builder()
        .title(notice.title.clone())
        .body(notice.body.clone());
    if let Some(sound) = sound.map(str::trim).filter(|sound| !sound.is_empty()) {
        builder = builder.sound(sound);
    }
    builder.show().map_err(|error| error.to_string())
}
