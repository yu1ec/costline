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
}

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
