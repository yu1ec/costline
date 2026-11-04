use std::path::Path;
use std::time::{Duration, Instant};

use costline_core::monitor::{self, Row};
use costline_core::{delivery, Channel, State};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::desktop;
use crate::state::{AppState, Snapshot};

/// 推送给前端的窗口内提醒。
#[derive(Debug, Clone, Serialize)]
pub struct AlertPayload {
    pub title: String,
    pub body: String,
    pub rows: Vec<Row>,
}

/// 推送给前端的定投提醒。
#[derive(Debug, Clone, Serialize)]
pub struct SipPayload {
    pub title: String,
    pub body: String,
    pub reminders: Vec<monitor::SipReminder>,
}

/// 实时价格刷新间隔（秒）。
const REALTIME_INTERVAL_SECS: u64 = 10;

/// 后台盯盘循环：每秒醒来一次，到了间隔就检查一轮。
pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut next_due = Instant::now() + Duration::from_secs(3);
        let mut next_realtime = next_due;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;

            let (watching, realtime, interval) = {
                let state = app.state::<AppState>();
                let runtime = state.lock();
                (
                    runtime.watching,
                    runtime.config.realtime,
                    runtime.config.interval_secs,
                )
            };
            let interval = Duration::from_secs(interval.max(5));

            let now = Instant::now();
            if watching && now >= next_due {
                run_check(&app, true, false).await;
                // 完整检查已经刷新过行情，实时刷新顺延到下一轮。
                next_due = Instant::now() + interval;
                next_realtime = Instant::now() + Duration::from_secs(REALTIME_INTERVAL_SECS);
            } else if realtime && now >= next_realtime {
                run_price_refresh(&app).await;
                next_realtime = Instant::now() + Duration::from_secs(REALTIME_INTERVAL_SECS);
            } else if !watching {
                next_due = Instant::now() + interval;
            }
        }
    });
}

/// 只刷新行情：不导入交割单，也不发送任何通知。
///
/// 用于面板上的「实时价格」开关；即使暂停盯盘，也能继续看到最新价格。
pub async fn run_price_refresh(app: &AppHandle) -> Snapshot {
    let (config, state_path, lock) = {
        let state = app.state::<AppState>();
        let runtime = state.lock();
        (
            runtime.config.clone(),
            state.state_path.clone(),
            state.check_lock.clone(),
        )
    };

    let _guard = lock.lock().await;

    let snapshot = match monitor::build_client(&config) {
        Ok(client) => {
            let report = monitor::check(&config, &client, &state_path, false, false).await;
            let mut messages = report.skipped.clone();
            messages.extend(report.errors.clone());
            if let Some(error) = report.fatal_error() {
                if !messages.iter().any(|message| message == &error) {
                    messages.push(error);
                }
            }

            let state = app.state::<AppState>();
            {
                let mut runtime = state.lock();
                runtime.rows = report.rows;
                runtime.last_check = Some(chrono::Utc::now().timestamp());
                runtime.last_error = if messages.is_empty() {
                    None
                } else {
                    Some(messages.join("\n"))
                };
            }
            state.snapshot()
        }
        Err(error) => {
            let state = app.state::<AppState>();
            {
                let mut runtime = state.lock();
                runtime.last_error = Some(format!("{error:#}"));
                runtime.last_check = Some(chrono::Utc::now().timestamp());
            }
            state.snapshot()
        }
    };

    let _ = app.emit("snapshot", &snapshot);
    snapshot
}

/// 执行一轮检查，更新内存快照并推送给前端。
pub async fn run_check(app: &AppHandle, notify_enabled: bool, force_notify: bool) -> Snapshot {
    let (config, config_path, state_path, lock) = {
        let state = app.state::<AppState>();
        let runtime = state.lock();
        (
            runtime.config.clone(),
            state.config_path.clone(),
            state.state_path.clone(),
            state.check_lock.clone(),
        )
    };

    let _guard = lock.lock().await;

    // 每轮检查前先扫描交割单，把新的成交写入状态文件。
    let mut import_messages = Vec::new();
    let config = match delivery::sync(&config_path, &state_path, None, false) {
        Ok(outcome) => {
            for warning in &outcome.report.warnings {
                import_messages.push(format!("交割单：{warning}"));
            }
            for error in &outcome.report.errors {
                import_messages.push(format!("交割单：{error}"));
            }
            {
                let state = app.state::<AppState>();
                state.lock().config = outcome.config.clone();
            }
            outcome.config
        }
        Err(error) => {
            import_messages.push(format!("交割单同步失败：{error:#}"));
            config
        }
    };

    // 桌面版始终发送应用自身的系统通知；显式配置的 desktop 渠道仅用于指定提示音，
    // 从远程渠道列表里移除，避免重复发送。
    let mut config = config;
    let desktop_sound = config
        .notify
        .channels
        .iter()
        .find_map(|channel| match channel {
            Channel::Desktop { sound } => Some(sound.clone()),
            _ => None,
        })
        .flatten();
    config
        .notify
        .channels
        .retain(|channel| !matches!(channel, Channel::Desktop { .. }));

    let mut alert: Option<AlertPayload> = None;
    let mut sip_alert: Option<SipPayload> = None;

    let snapshot = match monitor::build_client(&config) {
        Ok(client) => {
            let report =
                monitor::check(&config, &client, &state_path, notify_enabled, force_notify).await;

            if notify_enabled && !report.triggered.is_empty() {
                let notice = monitor::build_notice(&report.triggered);
                match desktop::send(app, &notice, desktop_sound.as_deref()) {
                    Ok(()) => {
                        // 没有其他渠道成功时，由系统通知来更新去重状态，避免反复触发。
                        if report.notify_success_count() == 0 {
                            mark_notified(&state_path, &report.triggered);
                        }
                    }
                    Err(error) => eprintln!("[costline] 系统通知发送失败：{error}"),
                }
                alert = Some(AlertPayload {
                    title: notice.title,
                    body: notice.body,
                    rows: report.triggered.clone(),
                });
            }

            if notify_enabled && !report.sip_due.is_empty() {
                let notice = monitor::build_sip_notice(&report.sip_due);
                match desktop::send(app, &notice, desktop_sound.as_deref()) {
                    Ok(()) => {
                        // 远程渠道都没有成功时，由系统通知来记录今天的定投提醒状态。
                        if report.sip_notify_success_count() == 0 {
                            mark_sip_notified(&state_path, &report.sip_due);
                        }
                    }
                    Err(error) => eprintln!("[costline] 定投系统通知发送失败：{error}"),
                }
                sip_alert = Some(SipPayload {
                    title: notice.title,
                    body: notice.body,
                    reminders: report.sip_due.clone(),
                });
            }

            let mut messages = import_messages.clone();
            messages.extend(report.skipped.clone());
            messages.extend(report.errors.clone());
            if let Some(error) = report.fatal_error() {
                if !messages.iter().any(|message| message == &error) {
                    messages.push(error);
                }
            }

            let state = app.state::<AppState>();
            {
                let mut runtime = state.lock();
                runtime.rows = report.rows;
                runtime.last_check = Some(chrono::Utc::now().timestamp());
                runtime.last_error = if messages.is_empty() {
                    None
                } else {
                    Some(messages.join("\n"))
                };
            }
            state.snapshot()
        }
        Err(error) => {
            let state = app.state::<AppState>();
            {
                let mut runtime = state.lock();
                runtime.last_error = Some(format!("{error:#}"));
                runtime.last_check = Some(chrono::Utc::now().timestamp());
            }
            state.snapshot()
        }
    };

    let _ = app.emit("snapshot", &snapshot);
    if let Some(alert) = alert {
        let _ = app.emit("alert", alert);
    }
    if let Some(sip) = sip_alert {
        let _ = app.emit("sip", sip);
    }
    snapshot
}

fn mark_sip_notified(state_path: &Path, reminders: &[monitor::SipReminder]) {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let ready: Vec<String> = reminders
        .iter()
        .filter(|item| item.status == monitor::SipPeriodStatus::Ready)
        .map(|item| item.key.clone())
        .collect();
    let warned: Vec<String> = reminders
        .iter()
        .filter(|item| item.status != monitor::SipPeriodStatus::Ready)
        .map(|item| item.key.clone())
        .collect();
    let mut state = State::load(state_path);
    state.mark_sip_notified(&ready, &today);
    state.mark_sip_blocked(&warned, &today);
    if let Err(error) = state.save(state_path) {
        eprintln!("[costline] 保存定投提醒状态失败：{error:#}");
    }
}

fn mark_notified(state_path: &Path, rows: &[Row]) {
    let now = chrono::Utc::now().timestamp();
    let mut state = State::load(state_path);
    for row in rows {
        let entry = state.assets.entry(row.key.clone()).or_default();
        entry.above = true;
        entry.last_notified = now;
    }
    if let Err(error) = state.save(state_path) {
        eprintln!("[costline] 保存提醒状态失败：{error:#}");
    }
}
