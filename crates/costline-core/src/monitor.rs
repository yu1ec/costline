use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Local};
use reqwest::Client;
use serde::Serialize;

use crate::config::{Asset, AssetKind, Config};
use crate::market::{self, FundTradeStatus, SubscribeVerdict};
use crate::notify::{self, Notice};
use crate::position::Position;
use crate::state::State;

/// 单期定投在提醒阶段的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SipPeriodStatus {
    /// 申购状态正常，可以按计划买入
    Ready,
    /// 暂停申购 / 封闭 / 超过限额，本期不要买入
    Blocked,
    /// 无法确认申购状态，需要人工确认
    Unknown,
}

/// 面板展示用的基金申购状态。
#[derive(Debug, Clone, Serialize)]
pub struct SipTradeInfo {
    /// 原始状态文案，例如「开放申购」「限大额」「暂停申购」
    pub status: String,
    /// 按本期金额判断是否受阻
    pub blocked: bool,
    pub max_amount: Option<f64>,
    pub reason: Option<String>,
    pub checked_at: i64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SipInfo {
    pub enabled: bool,
    pub amount: f64,
    /// 例如「每月 1 日 09:30」
    pub schedule: String,
    /// 下一次定投时间，例如 2026-12-01 09:30
    pub next_at: Option<String>,
    /// 今天是否是定投日（且已到提醒时间）
    pub due_today: bool,
    /// 基金申购状态（非基金或不适用时为空）
    pub trade: Option<SipTradeInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    pub key: String,
    pub name: String,
    pub kind: &'static str,
    pub id: String,
    pub price: f64,
    pub cost: Option<f64>,
    /// 触发价；本金已收回或已清仓时为 None
    pub target: Option<f64>,
    /// 相对昨收的涨跌额
    pub change_amount: Option<f64>,
    pub change_percent: Option<f64>,
    pub pnl: Option<f64>,
    /// 摊薄后的剩余本金；未配置持仓数量时为 None
    pub principal: Option<f64>,
    pub quantity: Option<f64>,
    pub currency: &'static str,
    pub above: bool,
    pub triggered: bool,
    /// 本金已全部收回（剩余本金 <= 0，仍有持仓）
    pub recovered: bool,
    /// 已清仓
    pub closed: bool,
    pub note: String,
    /// 国内金价参照（黄金及黄金基金、黄金股票显示）
    pub gold_price: Option<market::GoldReference>,
    pub sip: Option<SipInfo>,
}

/// 本轮到期的定投提醒。
#[derive(Debug, Clone, Serialize)]
pub struct SipReminder {
    pub key: String,
    pub name: String,
    pub kind: &'static str,
    pub id: String,
    pub amount: f64,
    pub currency: String,
    pub price: Option<f64>,
    /// 按当前价估算的可买数量
    pub estimate_quantity: Option<f64>,
    pub schedule: String,
    pub next_at: Option<String>,
    /// 本期申购状态
    pub status: SipPeriodStatus,
    /// 阻断 / 未知状态的原因
    pub reason: Option<String>,
    /// 基金申购状态原文
    pub trade_status: Option<String>,
    /// 基金单笔申购上限
    pub max_amount: Option<f64>,
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
    /// 本轮到期的定投提醒
    pub sip_due: Vec<SipReminder>,
    /// 定投提醒的远程渠道发送结果
    pub sip_notify_results: Vec<ChannelResult>,
    /// 有定投到期但未发送通知时的原因
    pub sip_notify_skipped: Option<String>,
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

    pub fn sip_notify_success_count(&self) -> usize {
        self.sip_notify_results
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
    let local_now = Local::now();
    let today = local_now.format("%Y-%m-%d").to_string();
    let mut state = State::load(state_path);
    let mut alerts = Vec::new();
    // 同一轮内只取一次国内金价；失败也缓存，避免每个黄金资产都重试。
    let mut domestic_gold: Option<Option<market::GoldReference>> = None;

    for asset in &config.assets {
        match market::fetch_quote(client, asset).await {
            Ok(quote) => {
                let key = asset.key();
                let previous = state.assets.get(&key).cloned().unwrap_or_default();
                // 交割单导入的成交保存在状态文件里，与配置中的期初持仓一起计算。
                let trades = state.trades_for(&key).to_vec();
                let position = match Position::derive(asset, &trades) {
                    Ok(position) => position,
                    Err(error) => {
                        report.failures += 1;
                        report
                            .errors
                            .push(format!("{} 成交流水无效：{error:#}", asset.label()));
                        continue;
                    }
                };
                let cost = asset.cost_price_with(&trades);
                let target = asset.target_price_with(config.threshold_percent, &trades);
                if target.is_none() && !position.recovered && !position.closed {
                    report
                        .skipped
                        .push(format!("{} 缺少成本价或触发价，已跳过", asset.label()));
                    report.failures += 1;
                    continue;
                }

                let above = match target {
                    Some(target) => quote.price >= target,
                    None => position.recovered,
                };
                let cooldown_ok = previous.last_notified == 0
                    || now - previous.last_notified >= config.notify.cooldown_secs as i64;
                let repeat_due = config.notify.repeat_interval_secs > 0
                    && previous.last_notified > 0
                    && now - previous.last_notified >= config.notify.repeat_interval_secs as i64;
                let triggered = target.is_some()
                    && above
                    && (force_notify || !previous.above || repeat_due)
                    && (force_notify || cooldown_ok);

                let display_name = asset
                    .name
                    .clone()
                    .or_else(|| quote.name.clone())
                    .unwrap_or_else(|| asset.id());
                let gold_price = if market::is_gold_related(asset, quote.name.as_deref()) {
                    if domestic_gold.is_none() {
                        domestic_gold = Some(match market::fetch_domestic_gold(client).await {
                            Ok(gold) => Some(gold),
                            Err(error) => {
                                report.errors.push(format!("国内金价获取失败：{error:#}"));
                                None
                            }
                        });
                    }
                    domestic_gold.clone().flatten()
                } else {
                    None
                };
                let row = Row {
                    key: key.clone(),
                    name: display_name,
                    kind: asset.kind.label(),
                    id: asset.id(),
                    price: quote.price,
                    cost,
                    target,
                    change_amount: quote.change_amount(),
                    change_percent: quote.change_percent(),
                    pnl: position.market_pnl(quote.price),
                    principal: position.has_position.then_some(position.principal),
                    quantity: position.has_position.then_some(position.quantity),
                    currency: quote.currency,
                    above,
                    triggered,
                    recovered: position.recovered,
                    closed: position.closed,
                    note: quote.note.clone(),
                    gold_price,
                    sip: asset.sip.as_ref().map(|sip| SipInfo {
                        enabled: sip.enabled,
                        amount: sip.amount,
                        schedule: sip.schedule_label(),
                        next_at: sip
                            .next_occurrence(local_now)
                            .map(|next| next.format("%Y-%m-%d %H:%M").to_string()),
                        due_today: sip.is_due_at(local_now),
                        trade: None,
                    }),
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

    // 定投相关：先刷新申购状态，再补全面板展示并判断本期能否买入。
    prepare_trade_statuses(config, client, &mut state, now, local_now).await;
    for row in &mut report.rows {
        let Some(sip) = row.sip.as_mut().filter(|sip| sip.enabled) else {
            continue;
        };
        let trade = state
            .assets
            .get(&row.key)
            .and_then(|entry| entry.trade_status.as_ref())
            .map(|status| build_sip_trade_info(status, sip.amount));
        sip.trade = trade;
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

    let sip_due = collect_sip_due(config, &state, &report.rows, &today, local_now);
    if !sip_due.is_empty() {
        if !notify_enabled {
            report.sip_notify_skipped = Some("通知已暂停".to_string());
        } else if config.notify.channels.is_empty() {
            report.sip_notify_skipped = Some("未配置通知渠道".to_string());
        } else {
            let notice = build_sip_notice(&sip_due);
            let results = notify::send_all(client, &config.notify.channels, &notice).await;
            let mut any_success = false;
            for result in results {
                let ok = result.result.is_ok();
                any_success |= ok;
                report.sip_notify_results.push(ChannelResult {
                    label: result.label.to_string(),
                    ok,
                    error: result.result.err().map(|error| format!("{error:#}")),
                });
            }
            if any_success {
                let ready: Vec<String> = sip_due
                    .iter()
                    .filter(|item| item.status == SipPeriodStatus::Ready)
                    .map(|item| item.key.clone())
                    .collect();
                let warned: Vec<String> = sip_due
                    .iter()
                    .filter(|item| item.status != SipPeriodStatus::Ready)
                    .map(|item| item.key.clone())
                    .collect();
                state.mark_sip_notified(&ready, &today);
                state.mark_sip_blocked(&warned, &today);
            }
        }
        report.sip_due = sip_due;
    }

    if let Err(error) = state.save(state_path) {
        report.errors.push(format!("保存状态失败：{error:#}"));
    }

    report
}

fn collect_sip_due(
    config: &Config,
    state: &State,
    rows: &[Row],
    today: &str,
    now: DateTime<Local>,
) -> Vec<SipReminder> {
    config
        .assets
        .iter()
        .filter_map(|asset| {
            let sip = asset.sip.as_ref()?;
            if !sip.is_due_at(now) {
                return None;
            }
            let key = asset.key();
            let entry = state.assets.get(&key);
            if entry.and_then(|entry| entry.last_sip_notified.as_deref()) == Some(today) {
                return None;
            }

            let (status, reason, trade_status, max_amount) = evaluate_sip_status(
                asset,
                sip.amount,
                entry.and_then(|e| e.trade_status.as_ref()),
            );
            if status != SipPeriodStatus::Ready
                && entry.and_then(|entry| entry.last_sip_blocked.as_deref()) == Some(today)
            {
                return None;
            }

            let row = rows.iter().find(|row| row.key == key);
            let price = row.map(|row| row.price);
            let currency = row.map(|row| row.currency).unwrap_or("CNY").to_string();
            let estimate_quantity = price
                .filter(|price| *price > 0.0)
                .map(|price| sip.amount / price);
            Some(SipReminder {
                key,
                name: row
                    .map(|row| row.name.clone())
                    .unwrap_or_else(|| asset.label()),
                kind: asset.kind.label(),
                id: asset.id(),
                amount: sip.amount,
                currency,
                price,
                estimate_quantity,
                schedule: sip.schedule_label(),
                next_at: sip
                    .next_occurrence(now + ChronoDuration::minutes(1))
                    .map(|next| next.format("%Y-%m-%d %H:%M").to_string()),
                status,
                reason,
                trade_status,
                max_amount,
            })
        })
        .collect()
}

/// 场外基金代码；股票、黄金、场内基金返回 None（不适用申购状态接口）。
fn otc_fund_code(asset: &Asset) -> Option<&str> {
    if asset.kind != AssetKind::Fund {
        return None;
    }
    let code = asset.code.as_deref().or(asset.symbol.as_deref())?;
    if market::is_exchange_fund(code) {
        None
    } else {
        Some(code)
    }
}

/// 判断本期定投能否买入，返回 (状态, 原因, 原始申购状态, 单笔上限)。
fn evaluate_sip_status(
    asset: &Asset,
    amount: f64,
    trade: Option<&FundTradeStatus>,
) -> (SipPeriodStatus, Option<String>, Option<String>, Option<f64>) {
    if otc_fund_code(asset).is_none() {
        return (SipPeriodStatus::Ready, None, None, None);
    }
    let Some(trade) = trade else {
        return (
            SipPeriodStatus::Unknown,
            Some("尚未查询到申购状态".to_string()),
            None,
            None,
        );
    };

    let (verdict, reason) = trade.check_amount(amount);
    let status = match verdict {
        SubscribeVerdict::Open => SipPeriodStatus::Ready,
        SubscribeVerdict::Blocked => SipPeriodStatus::Blocked,
        SubscribeVerdict::Unknown => SipPeriodStatus::Unknown,
    };
    (
        status,
        reason,
        trade.subscribe_status.clone(),
        trade.max_amount,
    )
}

fn build_sip_trade_info(status: &FundTradeStatus, amount: f64) -> SipTradeInfo {
    let (verdict, reason) = status.check_amount(amount);
    SipTradeInfo {
        status: status
            .subscribe_status
            .clone()
            .unwrap_or_else(|| "未确认".to_string()),
        blocked: verdict == SubscribeVerdict::Blocked,
        max_amount: status.max_amount,
        reason: reason.or_else(|| status.error.clone()),
        checked_at: status.checked_at,
        error: status.error.clone(),
    }
}

/// 刷新定投基金的申购状态；同一个 TTL 内复用缓存，失败时保留上一次结果并记录错误。
async fn prepare_trade_statuses(
    config: &Config,
    client: &Client,
    state: &mut State,
    now: i64,
    local_now: DateTime<Local>,
) {
    for asset in &config.assets {
        let Some(sip) = asset.sip.as_ref().filter(|sip| sip.enabled) else {
            continue;
        };
        let Some(code) = otc_fund_code(asset) else {
            continue;
        };

        let key = asset.key();
        let cached = state
            .assets
            .get(&key)
            .and_then(|entry| entry.trade_status.clone());
        // 定投日状态可能临时变化，缩短缓存；其他时间一天查询几次即可。
        let ttl = if sip.is_sip_day(local_now) {
            15 * 60
        } else {
            6 * 3600
        };
        if cached
            .as_ref()
            .is_some_and(|cache| now >= cache.checked_at && now - cache.checked_at < ttl)
        {
            continue;
        }

        match market::fetch_fund_trade_status(client, code).await {
            Ok(status) => {
                state.assets.entry(key).or_default().trade_status = Some(status);
            }
            Err(error) => {
                let mut fallback = cached.unwrap_or_default();
                fallback.error = Some(format!("{error:#}"));
                fallback.checked_at = now;
                state.assets.entry(key).or_default().trade_status = Some(fallback);
            }
        }
    }
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
        let target_text = row.target.map(fmt_price).unwrap_or_else(|| "-".to_string());
        lines.push(format!(
            "现价 {} ｜ 成本 {} ｜ 触发价 {}",
            fmt_price(row.price),
            cost_text,
            target_text
        ));
        if let Some(change) = row.change_percent {
            lines.push(format!("日涨跌 {}", fmt_percent(Some(change))));
        }
        if let Some(gold) = &row.gold_price {
            lines.push(format!(
                "国内金价 {} 元/克{}",
                gold_price_text(gold),
                gold.change_percent
                    .map(|change| format!("（{}）", fmt_percent(Some(change))))
                    .unwrap_or_default()
            ));
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

pub fn build_sip_notice(reminders: &[SipReminder]) -> Notice {
    let blocked = reminders
        .iter()
        .filter(|item| item.status == SipPeriodStatus::Blocked)
        .count();
    let unknown = reminders
        .iter()
        .filter(|item| item.status == SipPeriodStatus::Unknown)
        .count();
    let title = if blocked == reminders.len() {
        format!("costline 定投阻断提醒（{} 项）", reminders.len())
    } else if reminders.len() == 1 {
        format!("costline 定投提醒：{}", reminders[0].name)
    } else if blocked + unknown > 0 {
        format!(
            "costline 定投提醒（{} 项，{} 项需确认）",
            reminders.len(),
            blocked + unknown
        )
    } else {
        format!("costline 定投提醒（{} 项）", reminders.len())
    };

    let mut lines = Vec::new();
    lines.push(format!(
        "时间：{}",
        Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push("定投日到了，请按计划处理：".to_string());
    lines.push(String::new());

    for item in reminders {
        let icon = match item.status {
            SipPeriodStatus::Ready => "📥",
            SipPeriodStatus::Blocked => "⛔",
            SipPeriodStatus::Unknown => "⚠️",
        };
        lines.push(format!("{icon} {}（{}）", item.name, item.id));
        lines.push(format!(
            "本期金额 {}{:.2}（{}）",
            currency_symbol(&item.currency),
            item.amount,
            item.currency
        ));
        lines.push(format!("定投计划 {}", item.schedule));

        match item.status {
            SipPeriodStatus::Blocked => {
                if let Some(reason) = &item.reason {
                    lines.push(format!("阻断原因：{reason}"));
                }
                lines.push("本期资金未计入持仓，请勿买入".to_string());
            }
            SipPeriodStatus::Unknown => {
                if let Some(reason) = &item.reason {
                    lines.push(format!("状态未知：{reason}"));
                }
                lines.push("请先确认基金能否申购，再决定是否买入".to_string());
            }
            SipPeriodStatus::Ready => {
                if let Some(status) = &item.trade_status {
                    let limit = item
                        .max_amount
                        .map(|max| {
                            format!("，单笔上限 {}{:.2}", currency_symbol(&item.currency), max)
                        })
                        .unwrap_or_default();
                    lines.push(format!("申购状态 {status}{limit}"));
                }
                if let Some(price) = item.price {
                    let estimate = item
                        .estimate_quantity
                        .map(|quantity| format!("，约 {quantity:.2} 份/股"))
                        .unwrap_or_default();
                    lines.push(format!("当前价 {}{estimate}", fmt_price(price)));
                }
            }
        }

        if let Some(next) = &item.next_at {
            lines.push(format!("下次定投 {next}"));
        }
        lines.push(String::new());
    }

    lines.push("—— costline（仅提醒，不会自动下单）".to_string());
    Notice {
        title,
        body: lines.join("\n"),
    }
}

/// 国内金价的展示文案：有买卖价时显示「买入 x · 卖出 y」，否则退回最新价。
pub fn gold_price_text(gold: &market::GoldReference) -> String {
    match (gold.buy_price, gold.sell_price) {
        (Some(buy), Some(sell)) => {
            format!("买入 {} · 卖出 {}", fmt_price(buy), fmt_price(sell))
        }
        _ => fmt_price(gold.price),
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
            target: Some(10.0),
            change_amount: Some(0.12),
            change_percent: Some(1.2),
            pnl: Some(500.0),
            principal: Some(10000.0),
            quantity: Some(1000.0),
            currency: "CNY",
            above: true,
            triggered: true,
            recovered: false,
            closed: false,
            note: "东方财富实时".to_string(),
            gold_price: None,
            sip: None,
        };
        let notice = build_notice(&[row]);
        assert!(notice.title.contains("浦发银行"));
        assert!(notice.body.contains("现价 10.50"));
    }

    #[test]
    fn builds_sip_notice() {
        let reminder = SipReminder {
            key: "fund:161725".to_string(),
            name: "招商中证白酒".to_string(),
            kind: "基金",
            id: "161725".to_string(),
            amount: 500.0,
            currency: "CNY".to_string(),
            price: Some(0.6),
            estimate_quantity: Some(833.33),
            schedule: "每月 1 日 09:30".to_string(),
            next_at: Some("2026-12-01 09:30".to_string()),
            status: SipPeriodStatus::Ready,
            reason: None,
            trade_status: Some("限大额".to_string()),
            max_amount: Some(500000.0),
        };
        let notice = build_sip_notice(&[reminder]);
        assert!(notice.title.contains("招商中证白酒"));
        assert!(notice.body.contains("¥500.00"));
        assert!(notice.body.contains("每月 1 日 09:30"));
        assert!(notice.body.contains("限大额"));
    }

    #[test]
    fn builds_blocked_sip_notice() {
        let reminder = SipReminder {
            key: "fund:501018".to_string(),
            name: "南方原油A".to_string(),
            kind: "基金",
            id: "501018".to_string(),
            amount: 500.0,
            currency: "CNY".to_string(),
            price: None,
            estimate_quantity: None,
            schedule: "每月 1 日 09:30".to_string(),
            next_at: None,
            status: SipPeriodStatus::Blocked,
            reason: Some("暂停申购".to_string()),
            trade_status: Some("暂停申购".to_string()),
            max_amount: None,
        };
        let notice = build_sip_notice(&[reminder]);
        assert!(notice.title.contains("阻断"));
        assert!(notice.body.contains("阻断原因：暂停申购"));
        assert!(notice.body.contains("本期资金未计入持仓"));
    }
}
