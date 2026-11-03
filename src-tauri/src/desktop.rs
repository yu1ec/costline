use costline_core::notify::Notice;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

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
