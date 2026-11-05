use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::Trade;
use crate::market::FundTradeStatus;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub assets: BTreeMap<String, AssetState>,
    /// 从交割单导入的成交流水，按资产 key 分组。
    ///
    /// 与配置文件分开保存，自动导入时无需改写用户的 costline.toml。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub trades: BTreeMap<String, Vec<Trade>>,
    /// 已导入文件的指纹，避免每轮重复解析没有变化的交割单。
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub imports: BTreeMap<String, ImportedFile>,
    /// 已经推送过的通知记录，最新的排在最后。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<NoticeRecord>,
}

/// 通知类型，用于前端分组与筛选。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoticeKind {
    /// 回本 / 价格穿越成本线提醒
    #[default]
    Price,
    /// 定投到期提醒（含阻断 / 状态未知）
    Sip,
    /// 设置页手动发送的测试消息
    Test,
}

/// 单个通知渠道的发送结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoticeChannel {
    pub label: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 一条已经推送出去的消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoticeRecord {
    /// 记录 ID，前端用于定位与高亮。
    pub id: String,
    /// 发送时间（Unix 秒）
    pub at: i64,
    pub kind: NoticeKind,
    pub title: String,
    pub body: String,
    /// 各渠道发送结果；至少有一个渠道成功才会记录。
    #[serde(default)]
    pub channels: Vec<NoticeChannel>,
}

impl NoticeRecord {
    pub fn new(
        kind: NoticeKind,
        title: impl Into<String>,
        body: impl Into<String>,
        channels: Vec<NoticeChannel>,
    ) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: format!("{}", now.timestamp_micros()),
            at: now.timestamp(),
            kind,
            title: title.into(),
            body: body.into(),
            channels,
        }
    }
}

/// 最多保留的通知条数，超出后丢弃最早的记录。
pub const MAX_NOTICE_HISTORY: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ImportedFile {
    #[serde(default)]
    pub len: u64,
    /// 文件修改时间（Unix 秒）
    #[serde(default)]
    pub modified: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetState {
    /// 上一次检查时是否已经高于触发价
    #[serde(default)]
    pub above: bool,
    /// 上一次成功通知的 Unix 时间戳（秒）
    #[serde(default)]
    pub last_notified: i64,
    /// 上一次检查到的价格
    #[serde(default)]
    pub last_price: Option<f64>,
    /// 最近一次发送定投提醒的日期（YYYY-MM-DD），用于同一天只提醒一次
    #[serde(default)]
    pub last_sip_notified: Option<String>,
    /// 最近一次发送“定投受阻 / 状态未知”提醒的日期（YYYY-MM-DD）。
    /// 与 last_sip_notified 分开记录：状态恢复开放后同一天仍会正常提醒。
    #[serde(default)]
    pub last_sip_blocked: Option<String>,
    /// 最近一次查询到的基金申购状态
    #[serde(default)]
    pub trade_status: Option<FundTradeStatus>,
}

impl State {
    /// 记录定投提醒已发送的日期，避免同一天重复提醒。
    pub fn mark_sip_notified(&mut self, keys: &[String], date: &str) {
        for key in keys {
            self.assets
                .entry(key.clone())
                .or_default()
                .last_sip_notified = Some(date.to_string());
        }
    }

    /// 记录“本期定投受阻”提醒已发送的日期。
    pub fn mark_sip_blocked(&mut self, keys: &[String], date: &str) {
        for key in keys {
            self.assets.entry(key.clone()).or_default().last_sip_blocked = Some(date.to_string());
        }
    }

    pub fn trades_for(&self, key: &str) -> &[Trade] {
        self.trades.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn add_trade(&mut self, key: &str, trade: Trade) {
        self.trades.entry(key.to_string()).or_default().push(trade);
    }

    pub fn has_trade(&self, key: &str, trade_key: &str) -> bool {
        self.trades_for(key)
            .iter()
            .any(|trade| trade.key() == trade_key)
    }

    /// 追加一条已推送的消息，并按上限裁剪最早的记录。
    pub fn record_notice(&mut self, record: NoticeRecord) {
        self.notices.push(record);
        if self.notices.len() > MAX_NOTICE_HISTORY {
            self.notices
                .drain(..self.notices.len() - MAX_NOTICE_HISTORY);
        }
    }

    pub fn clear_notices(&mut self) {
        self.notices.clear();
    }

    /// 按时间倒序返回消息历史，方便前端直接渲染。
    pub fn notices_desc(&self) -> Vec<NoticeRecord> {
        let mut notices = self.notices.clone();
        notices.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| b.id.cmp(&a.id)));
        notices
    }

    pub fn load(path: &Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else {
            return Self::default();
        };
        match serde_json::from_str(&text) {
            Ok(state) => state,
            Err(error) => {
                eprintln!(
                    "[costline] 状态文件 {} 解析失败，将重新开始：{error}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("创建状态目录失败：{}", parent.display()))?;
        }
        let temp_path = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self).context("序列化状态失败")?;
        fs::write(&temp_path, text)
            .with_context(|| format!("写入状态文件失败：{}", temp_path.display()))?;
        fs::rename(&temp_path, path)
            .with_context(|| format!("更新状态文件失败：{}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(at: i64) -> NoticeRecord {
        NoticeRecord {
            id: at.to_string(),
            at,
            kind: NoticeKind::Sip,
            title: "定投提醒".to_string(),
            body: "body".to_string(),
            channels: vec![NoticeChannel {
                label: "系统通知".to_string(),
                ok: true,
                error: None,
            }],
        }
    }

    #[test]
    fn keeps_only_latest_notices() {
        let mut state = State::default();
        for at in 0..(MAX_NOTICE_HISTORY as i64 + 5) {
            state.record_notice(record(at));
        }
        assert_eq!(state.notices.len(), MAX_NOTICE_HISTORY);
        assert_eq!(state.notices.first().map(|notice| notice.at), Some(5));
        assert_eq!(
            state.notices.last().map(|notice| notice.at),
            Some(MAX_NOTICE_HISTORY as i64 + 4)
        );
    }

    #[test]
    fn returns_notices_newest_first() {
        let mut state = State::default();
        state.record_notice(record(10));
        state.record_notice(record(20));
        let notices = state.notices_desc();
        assert_eq!(notices.len(), 2);
        assert_eq!(notices[0].at, 20);
        assert_eq!(notices[1].at, 10);
    }

    #[test]
    fn notice_serde_uses_lowercase_kind() {
        let notice = record(1);
        let text = serde_json::to_string(&notice).unwrap();
        assert!(text.contains("\"kind\":\"sip\""));
        let parsed: NoticeRecord = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.kind, NoticeKind::Sip);
        assert_eq!(parsed.channels[0].label, "系统通知");
    }
}
