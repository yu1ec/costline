use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Local;
use reqwest::Client;
use serde::Serialize;

use crate::config::Config;
use crate::market;
use crate::notify::{self, Notice};
use crate::state::State;

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub key: String,
    pub name: String,
    pub kind: &'static str,
    pub id: String,
    pub price: f64,
    pub cost: Option<f64>,
    pub target: f64,
    pub change_percent: Option<f64>,
    pub pnl: Option<f64>,
    pub currency: &'static str,
    pub above: bool,
    pub triggered: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelResult {
    pub label: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CheckReport {
    pub rows: Vec<Row>,
    pub triggered: Vec<Row>,
    pub total: usize,
    pub failures: usize,
    /// 缺少成本价 / 触发价而跳过的资产
    pub skipped: Vec<String>,
    /// 行情获取或状态保存错误
    pub errors: Vec<String>,
    pub notify_results: Vec<ChannelResult>,
    /// 有触发但未发送通知时的原因
    pub notify_skipped: Option<String>,
}

impl CheckReport {
    pub fn fatal_error(&self) -> Option<String> {
        if self.total > 0 && self.failures == self.total {
            return Some(if self.errors.is_empty() {
                "所有资产行情获取失败".to_string()
            } else {
                self.errors.join("；")
            });
        }
        if !self.notify_results.is_empty() && self.notify_results.iter().all(|result| !result.ok) {
            return Some("所有通知渠道均发送失败".to_string());
        }
        None
    }

    pub fn notify_success_count(&self) -> usize {
        self.notify_results
            .iter()
            .filter(|result| result.ok)
            .count()
    }
}

pub fn build_client(config: &Config) -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs.max(1)))
        .connect_timeout(Duration::from_secs(5))
        .user_agent(format!("costline/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("创建 HTTP 客户端失败")
}

/// 检查一轮行情，按需发送通知，并持久化边沿触发状态。
pub async fn check(
    config: &Config,
    client: &Client,
    state_path: &Path,
    notify_enabled: bool,
    force_notify: bool,
) -> CheckReport {
    let mut report = CheckReport {
        total: config.assets.len(),
        ..CheckReport::default()
    };
    if config.assets.is_empty() {
        return report;
    }

    let now = chrono::Utc::now().timestamp();
    let mut state = State::load(state_path);
    let mut alerts = Vec::new();

    for asset in &config.assets {
        match market::fetch_quote(client, asset).await {
            Ok(quote) => {
                let key = asset.key();
                let previous = state.assets.get(&key).cloned().unwrap_or_default();
                let cost = asset.cost_price();
                let Some(target) = asset.target_price(config.threshold_percent) else {
                    report
                        .skipped
                        .push(format!("{} 缺少成本价或触发价，已跳过", asset.label()));
                    report.failures += 1;
                    continue;
                };

                let above = quote.price >= target;
                let cooldown_ok = previous.last_notified == 0
                    || now - previous.last_notified >= config.notify.cooldown_secs as i64;
                let repeat_due = config.notify.repeat_interval_secs > 0
                    && previous.last_notified > 0
                    && now - previous.last_notified >= config.notify.repeat_interval_secs as i64;
                let triggered = above
                    && (force_notify || !previous.above || repeat_due)
                    && (force_notify || cooldown_ok);

                let display_name = asset
                    .name
                    .clone()
                    .or_else(|| quote.name.clone())
                    .unwrap_or_else(|| asset.id());
                let row = Row {
                    key: key.clone(),
                    name: display_name,
                    kind: asset.kind.label(),
                    id: asset.id(),
                    price: quote.price,
                    cost,
                    target,
                    change_percent: quote.change_percent(),
                    pnl: match (cost, asset.quantity) {
                        (Some(cost), Some(quantity)) => Some((quote.price - cost) * quantity),
                        _ => None,
                    },
                    currency: quote.currency,
                    above,
                    triggered,
                    note: quote.note.clone(),
                };

                if triggered {
                    alerts.push(row.clone());
                }
                report.rows.push(row);

                let entry = state.assets.entry(key).or_default();
                entry.last_price = Some(quote.price);
                // 触发的资产等通知结果确定后再更新 above，通知被跳过/失败时保留旧值
                if !triggered {
                    entry.above = above;
                }
            }
            Err(error) => {
                report.failures += 1;
                report
                    .errors
                    .push(format!("{} 行情获取失败：{error:#}", asset.label()));
            }
        }
    }

    report.triggered = alerts.clone();

    if !alerts.is_empty() {
        if !notify_enabled {
            report.notify_skipped = Some("通知已暂停".to_string());
        } else if config.notify.channels.is_empty() {
            report.notify_skipped = Some("未配置通知渠道".to_string());
        } else {
            let notice = build_notice(&alerts);
            let results = notify::send_all(client, &config.notify.channels, &notice).await;
            let mut any_success = false;
            for result in results {
                match result.result {
                    Ok(()) => {
                        any_success = true;
                        report.notify_results.push(ChannelResult {
                            label: result.label.to_string(),
                            ok: true,
                            error: None,
                        });
                    }
                    Err(error) => {
                        report.notify_results.push(ChannelResult {
                            label: result.label.to_string(),
                            ok: false,
                            error: Some(format!("{error:#}")),
                        });
                    }
                }
            }

            if any_success {
                for alert in &alerts {
                    if let Some(entry) = state.assets.get_mut(&alert.key) {
                        entry.above = true;
                        entry.last_notified = now;
                    }
                }
            } else {
                for alert in &alerts {
                    if let Some(entry) = state.assets.get_mut(&alert.key) {
                        // 回滚状态，下一轮继续尝试通知
                        entry.above = false;
                    }
                }
            }
        }
    }

    if let Err(error) = state.save(state_path) {
        report.errors.push(format!("保存状态失败：{error:#}"));
    }

    report
}

pub fn test_notice() -> Notice {
    Notice {
        title: "costline 通知测试".to_string(),
        body: format!(
            "如果你收到这条消息，说明通知渠道配置成功。\n\n时间：{}\n版本：{}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            env!("CARGO_PKG_VERSION")
        ),
    }
}

pub fn build_notice(alerts: &[Row]) -> Notice {
    let title = if alerts.len() == 1 {
        format!("costline 回本提醒：{}", alerts[0].name)
    } else {
        format!("costline 回本提醒（{} 项）", alerts.len())
    };

    let mut lines = Vec::new();
    lines.push(format!(
        "时间：{}",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(String::new());

    for row in alerts {
        lines.push(format!("📈 {}（{}）", row.name, row.id));
        let cost_text = row.cost.map(fmt_price).unwrap_or_else(|| "-".to_string());
        lines.push(format!(
            "现价 {} ｜ 成本 {} ｜ 触发价 {}",
            fmt_price(row.price),
            cost_text,
            fmt_price(row.target)
        ));
        if let Some(change) = row.change_percent {
            lines.push(format!("日涨跌 {}", fmt_percent(Some(change))));
        }
        if let Some(pnl) = row.pnl {
            lines.push(format!(
                "浮动盈亏 {}（{}）",
                fmt_signed_money(pnl, row.currency),
                row.currency
            ));
        }
        if !row.note.is_empty() {
            lines.push(format!("数据：{}", row.note));
        }
        lines.push(String::new());
    }

    lines.push("—— costline".to_string());
    Notice {
        title,
        body: lines.join("\n"),
    }
}

pub fn fmt_price(value: f64) -> String {
    let decimals = if value.abs() < 1.0 {
        4
    } else if value.abs() < 10.0 {
        3
    } else {
        2
    };
    format!("{value:.decimals$}")
}

pub fn fmt_percent(value: Option<f64>) -> String {
    value
        .map(|value| format!("{value:+.2}%"))
        .unwrap_or_else(|| "-".to_string())
}

pub fn fmt_signed_money(value: f64, currency: &str) -> String {
    let sign = if value >= 0.0 { "+" } else { "-" };
    format!("{sign}{}{:.2}", currency_symbol(currency), value.abs())
}

pub fn currency_symbol(currency: &str) -> &'static str {
    match currency {
        "CNY" => "¥",
        "HKD" => "HK$",
        "USD" => "$",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_numbers() {
        assert_eq!(fmt_price(0.5877), "0.5877");
        assert_eq!(fmt_price(7.989), "7.989");
        assert_eq!(fmt_price(840.0), "840.00");
        assert_eq!(fmt_percent(Some(1.234)), "+1.23%");
        assert_eq!(fmt_percent(Some(-0.5)), "-0.50%");
        assert_eq!(fmt_signed_money(160.0, "CNY"), "+¥160.00");
        assert_eq!(fmt_signed_money(-160.0, "USD"), "-$160.00");
    }

    #[test]
    fn builds_notice_for_alerts() {
        let row = Row {
            key: "stock:1.600000".to_string(),
            name: "浦发银行".to_string(),
            kind: "股票",
            id: "1.600000".to_string(),
            price: 10.5,
            cost: Some(10.0),
            target: 10.0,
            change_percent: Some(1.2),
            pnl: Some(500.0),
            currency: "CNY",
            above: true,
            triggered: true,
            note: "东方财富实时".to_string(),
        };
        let notice = build_notice(&[row]);
        assert!(notice.title.contains("浦发银行"));
        assert!(notice.body.contains("现价 10.50"));
    }
}
