use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 轮询间隔（秒）
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    /// 单次 HTTP 请求超时（秒）
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// 全局回本阈值：现价 >= 成本价 * (1 + threshold_percent / 100)
    #[serde(default)]
    pub threshold_percent: f64,
    #[serde(default)]
    pub notify: NotifyConfig,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotifyConfig {
    /// 两次通知之间的最小间隔（秒），避免反复穿越成本线时刷屏
    #[serde(default = "default_cooldown")]
    pub cooldown_secs: u64,
    /// 价格持续高于成本线时的重复提醒间隔（秒），0 表示只在穿越时提醒一次
    #[serde(default)]
    pub repeat_interval_secs: u64,
    #[serde(default)]
    pub channels: Vec<Channel>,
}

impl Default for NotifyConfig {
    fn default() -> Self {
        Self {
            cooldown_secs: default_cooldown(),
            repeat_interval_secs: 0,
            channels: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval_secs: default_interval(),
            timeout_secs: default_timeout(),
            threshold_percent: 0.0,
            notify: NotifyConfig::default(),
            assets: Vec::new(),
        }
    }
}

fn default_interval() -> u64 {
    60
}

fn default_timeout() -> u64 {
    10
}

fn default_cooldown() -> u64 {
    1800
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    Stock,
    Fund,
    Gold,
}

impl AssetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AssetKind::Stock => "stock",
            AssetKind::Fund => "fund",
            AssetKind::Gold => "gold",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AssetKind::Stock => "股票",
            AssetKind::Fund => "基金",
            AssetKind::Gold => "黄金",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub kind: AssetKind,
    /// 展示名称，留空时使用行情接口返回的名称
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 股票代码 / 黄金代码，例如 sh600000、000001、00700.HK、AU9999
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// 东方财富 secid，例如 1.600000、116.00700、118.AU9999
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secid: Option<String>,
    /// 场外基金代码，例如 161725
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// 成本价 / 持仓均价
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    /// 持有数量：股票为股数，基金为份额，黄金为克数
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<f64>,
    /// 本金；与 quantity 一起配置时可自动计算成本价
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub principal: Option<f64>,
    /// 该资产单独的回本阈值（%），覆盖全局配置
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threshold_percent: Option<f64>,
    /// 明确的触发价，优先级高于成本价与阈值
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_price: Option<f64>,
}

impl Asset {
    pub fn id(&self) -> String {
        self.secid
            .as_deref()
            .or(self.symbol.as_deref())
            .or(self.code.as_deref())
            .unwrap_or("?")
            .to_string()
    }

    /// 用于状态去重的稳定 key
    pub fn key(&self) -> String {
        format!("{}:{}", self.kind.as_str(), self.id().to_lowercase())
    }

    pub fn label(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.id())
    }

    /// 成本价：优先 cost，其次 principal / quantity
    pub fn cost_price(&self) -> Option<f64> {
        if let Some(cost) = self.cost {
            return Some(cost);
        }
        match (self.principal, self.quantity) {
            (Some(principal), Some(quantity)) if quantity > 0.0 => Some(principal / quantity),
            _ => None,
        }
    }

    pub fn target_price(&self, global_threshold: f64) -> Option<f64> {
        if let Some(target) = self.target_price {
            return Some(target);
        }
        let cost = self.cost_price()?;
        let threshold = self.threshold_percent.unwrap_or(global_threshold);
        Some(cost * (1.0 + threshold / 100.0))
    }

    pub fn validate(&self) -> Result<()> {
        let label = self.label();

        match self.kind {
            AssetKind::Stock => {
                if self.symbol.is_none() && self.secid.is_none() {
                    bail!("资产「{label}」缺少 symbol 或 secid");
                }
            }
            AssetKind::Fund => {
                if self.code.is_none() && self.symbol.is_none() && self.secid.is_none() {
                    bail!("资产「{label}」缺少 code（场外基金）或 secid（场内基金）");
                }
            }
            AssetKind::Gold => {
                if self.secid.is_none() && self.symbol.is_none() {
                    bail!("资产「{label}」缺少 symbol 或 secid");
                }
            }
        }

        if self.cost.is_none()
            && self.target_price.is_none()
            && (self.principal.is_none() || self.quantity.is_none())
        {
            bail!("资产「{label}」需要配置 cost，或者同时配置 principal 与 quantity");
        }

        if self.principal.is_some() && self.quantity.is_none() && self.cost.is_none() {
            bail!("资产「{label}」配置了 principal，但缺少 quantity，无法计算成本价");
        }

        for (field, value) in [
            ("cost", self.cost),
            ("quantity", self.quantity),
            ("principal", self.principal),
            ("target_price", self.target_price),
        ] {
            if let Some(v) = value {
                if !v.is_finite() || v <= 0.0 {
                    bail!("资产「{label}」的 {field} 必须大于 0");
                }
            }
        }

        if let Some(threshold) = self.threshold_percent {
            if !threshold.is_finite() || threshold < 0.0 {
                bail!("资产「{label}」的 threshold_percent 不能为负数（触发条件是现价高于成本）");
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Channel {
    /// Bark（iOS，免费）
    Bark {
        key: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        group: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sound: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<String>,
    },
    /// Server 酱（微信，免费）
    Serverchan {
        sendkey: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        version: Option<u8>,
    },
    /// PushPlus（微信，免费）
    Pushplus {
        token: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        topic: Option<String>,
    },
    /// Telegram Bot
    Telegram { bot_token: String, chat_id: String },
    /// 企业微信群机器人
    Wecom { webhook: String },
    /// 钉钉群机器人
    Dingtalk {
        webhook: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret: Option<String>,
    },
    /// 飞书自定义机器人
    Feishu {
        webhook: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        secret: Option<String>,
    },
    /// ntfy（开源，可自建）
    Ntfy {
        topic: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        server: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
    },
    /// Gotify（开源，可自建）
    Gotify { server: String, token: String },
    /// 通用 Webhook：POST JSON {title, content, timestamp, source}
    Webhook {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        headers: Option<BTreeMap<String, String>>,
    },
    /// 系统本地通知（macOS / Linux）
    Desktop {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sound: Option<String>,
    },
}

impl Channel {
    pub fn label(&self) -> &'static str {
        match self {
            Channel::Bark { .. } => "Bark",
            Channel::Serverchan { .. } => "Server酱",
            Channel::Pushplus { .. } => "PushPlus",
            Channel::Telegram { .. } => "Telegram",
            Channel::Wecom { .. } => "企业微信",
            Channel::Dingtalk { .. } => "钉钉",
            Channel::Feishu { .. } => "飞书",
            Channel::Ntfy { .. } => "ntfy",
            Channel::Gotify { .. } => "Gotify",
            Channel::Webhook { .. } => "Webhook",
            Channel::Desktop { .. } => "系统通知",
        }
    }

    pub fn validate(&self) -> Result<()> {
        let empty = |s: &str| s.trim().is_empty();
        match self {
            Channel::Bark { key, .. } if empty(key) => bail!("Bark 的 key 不能为空"),
            Channel::Serverchan { sendkey, .. } if empty(sendkey) => {
                bail!("Server酱的 sendkey 不能为空")
            }
            Channel::Pushplus { token, .. } if empty(token) => bail!("PushPlus 的 token 不能为空"),
            Channel::Telegram { bot_token, chat_id } if empty(bot_token) || empty(chat_id) => {
                bail!("Telegram 的 bot_token 和 chat_id 不能为空")
            }
            Channel::Wecom { webhook } if empty(webhook) => bail!("企业微信 webhook 不能为空"),
            Channel::Dingtalk { webhook, .. } if empty(webhook) => bail!("钉钉 webhook 不能为空"),
            Channel::Feishu { webhook, .. } if empty(webhook) => bail!("飞书 webhook 不能为空"),
            Channel::Ntfy { topic, .. } if empty(topic) => bail!("ntfy topic 不能为空"),
            Channel::Gotify { server, token } if empty(server) || empty(token) => {
                bail!("Gotify 的 server 和 token 不能为空")
            }
            Channel::Webhook { url, .. } if empty(url) => bail!("Webhook url 不能为空"),
            _ => {}
        }
        Ok(())
    }
}

impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.interval_secs < 5 {
            bail!("interval_secs 不能小于 5 秒（免费行情接口请保持合理频率）");
        }
        if self.timeout_secs == 0 {
            bail!("timeout_secs 必须大于 0");
        }
        if !self.threshold_percent.is_finite() || self.threshold_percent < 0.0 {
            bail!("threshold_percent 不能为负数（触发条件是现价高于成本）");
        }
        for channel in &self.notify.channels {
            channel.validate()?;
        }
        for asset in &self.assets {
            asset.validate()?;
        }
        Ok(())
    }
}

pub fn load(path: &Path) -> Result<Config> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("读取配置文件失败：{}", path.display()))?;
    let config: Config =
        toml::from_str(&text).with_context(|| format!("解析配置文件失败：{}", path.display()))?;
    config.validate()?;
    Ok(config)
}

/// 校验并写入配置文件（先写临时文件再原子替换）
pub fn save(path: &Path, config: &Config) -> Result<()> {
    config.validate()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("创建配置目录失败：{}", parent.display()))?;
    }
    let text = toml::to_string_pretty(config).context("序列化配置失败")?;
    let temp_path = path.with_extension("toml.tmp");
    fs::write(&temp_path, text)
        .with_context(|| format!("写入配置文件失败：{}", temp_path.display()))?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("更新配置文件失败：{}", path.display()))?;
    Ok(())
}

pub fn default_config_path() -> PathBuf {
    if let Some(path) = env::var_os("COSTLINE_CONFIG") {
        return PathBuf::from(path);
    }
    let local = PathBuf::from("costline.toml");
    if local.exists() {
        return local;
    }
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("costline")
        .join("config.toml")
}

pub fn default_state_path() -> PathBuf {
    if let Some(path) = env::var_os("COSTLINE_STATE") {
        return PathBuf::from(path);
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("costline")
        .join("state.json")
}

pub const SAMPLE_CONFIG: &str = r#"# ============================================================
# costline 配置文件
# ============================================================
# 价格单位：
#   股票 / ETF：每股报价（A股 CNY，港股 HKD，美股 USD）
#   场外基金：每份净值（CNY），盘中优先使用天天基金估值
#   黄金：元/克（118.AU9999 为上海黄金交易所 Au99.99）
#
# 触发条件：现价 >= 成本价 × (1 + threshold_percent / 100)
# 若配置了 target_price，则直接使用 target_price 作为触发价。

interval_secs = 60       # 轮询间隔，建议 >= 30 秒
timeout_secs = 10
threshold_percent = 0.0  # 0 表示价格超过成本价即提醒；2.0 表示盈利 2% 才提醒

[notify]
cooldown_secs = 1800       # 两次提醒的最小间隔（秒）
repeat_interval_secs = 0   # 持续高于成本线时的重复提醒间隔，0 = 不重复

# ============================================================
# 通知渠道：按需取消注释（可同时配置多个）
# ============================================================

# --- Bark（iOS，免费） ---
# [[notify.channels]]
# type = "bark"
# key = "你的BarkKey"
# server = "https://api.day.app"   # 自建 Bark 时修改
# group = "costline"
# sound = "default"
# level = "active"

# --- Server 酱（微信，免费） ---
# [[notify.channels]]
# type = "serverchan"
# sendkey = "sctp... 或 SCT..."
# # version = 2                    # 旧版 SendKey（SCT 开头）可显式指定
# # base_url = "https://..."       # 自建 / 反代时使用

# --- PushPlus（微信，免费） ---
# [[notify.channels]]
# type = "pushplus"
# token = "你的PushPlusToken"
# # topic = "群组编码"              # 可选，群组推送

# --- Telegram Bot ---
# [[notify.channels]]
# type = "telegram"
# bot_token = "123456:ABC..."
# chat_id = "123456789"

# --- 企业微信群机器人 ---
# [[notify.channels]]
# type = "wecom"
# webhook = "https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=..."

# --- 钉钉群机器人 ---
# [[notify.channels]]
# type = "dingtalk"
# webhook = "https://oapi.dingtalk.com/robot/send?access_token=..."
# # secret = "SEC..."               # 机器人开启“加签”时填写

# --- 飞书自定义机器人 ---
# [[notify.channels]]
# type = "feishu"
# webhook = "https://open.feishu.cn/open-apis/bot/v2/hook/..."
# # secret = "..."                  # 机器人开启签名校验时填写

# --- ntfy（开源，可自建） ---
# [[notify.channels]]
# type = "ntfy"
# topic = "costline-请换成随机主题"
# server = "https://ntfy.sh"
# # token = "tk_..."                # 自建服务需要鉴权时填写

# --- Gotify（开源，可自建） ---
# [[notify.channels]]
# type = "gotify"
# server = "https://gotify.example.com"
# token = "你的AppToken"

# --- 通用 Webhook ---
# [[notify.channels]]
# type = "webhook"
# url = "https://example.com/hook"
# # [notify.channels.headers]
# # Authorization = "Bearer xxx"

# --- 系统本地通知（桌面版默认开启；这里可指定提示音） ---
# 桌面版触发时会自动发送系统通知，无需配置；命令行版需要显式添加。
# [[notify.channels]]
# type = "desktop"
# sound = "Glass"

# ============================================================
# 持仓配置
# ============================================================

[[assets]]
kind = "stock"
name = "浦发银行"
symbol = "sh600000"      # 支持 600000 / 600000.SH / sh600000 / 1.600000
cost = 10.00             # 你的成本价
quantity = 1000          # 股数（可选，用于计算浮动盈亏）

[[assets]]
kind = "fund"
name = "招商中证白酒"
code = "161725"          # 场外基金代码
cost = 0.5877
quantity = 10000

[[assets]]
kind = "gold"
name = "黄金9999"
secid = "118.AU9999"     # 上海黄金交易所 Au99.99，单位：元/克
cost = 800.00
quantity = 100           # 克数

# 也可以用“本金 + 数量”代替 cost，成本价会自动计算：
# [[assets]]
# kind = "stock"
# name = "平安银行"
# symbol = "sz000001"
# principal = 10000       # 本金
# quantity = 1000         # 股数
# # 成本价 = 10000 / 1000 = 10.00

# 其他市场示例：
# 港股：symbol = "00700.HK" 或 secid = "116.00700"
# 美股：symbol = "AAPL"      或 secid = "105.AAPL"
# ETF：kind = "fund", code = "518880"（自动识别为场内基金）
# 国际金价：kind = "gold", symbol = "hf_XAU"（单位：USD/盎司）
# 沪金主连：kind = "gold", symbol = "nf_AU0"（单位：元/克）
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_config_parses() {
        let config: Config = toml::from_str(SAMPLE_CONFIG).expect("sample config should parse");
        config.validate().expect("sample config should validate");
        assert_eq!(config.assets.len(), 3);
        assert_eq!(config.assets[0].cost_price(), Some(10.0));
    }

    #[test]
    fn sample_config_roundtrips() {
        let config: Config = toml::from_str(SAMPLE_CONFIG).expect("sample config should parse");
        let text = toml::to_string_pretty(&config).expect("config should serialize");
        let parsed: Config = toml::from_str(&text).expect("serialized config should parse");
        assert_eq!(parsed.assets.len(), 3);
        assert_eq!(parsed.assets[2].target_price(0.0), Some(800.0));
    }

    #[test]
    fn principal_and_quantity_derive_cost() {
        let asset = Asset {
            kind: AssetKind::Stock,
            name: None,
            symbol: Some("sh600000".into()),
            secid: None,
            code: None,
            cost: None,
            quantity: Some(1000.0),
            principal: Some(12000.0),
            threshold_percent: None,
            target_price: None,
        };
        asset.validate().unwrap();
        assert_eq!(asset.cost_price(), Some(12.0));
        assert_eq!(asset.target_price(2.0), Some(12.24));
    }

    #[test]
    fn all_channel_types_parse() {
        let text = r#"
interval_secs = 30

[[notify.channels]]
type = "bark"
key = "bark-key"

[[notify.channels]]
type = "serverchan"
sendkey = "sctp-key"

[[notify.channels]]
type = "pushplus"
token = "pushplus-token"

[[notify.channels]]
type = "telegram"
bot_token = "123:abc"
chat_id = "456"

[[notify.channels]]
type = "wecom"
webhook = "https://example.com/wecom"

[[notify.channels]]
type = "dingtalk"
webhook = "https://example.com/dingtalk"
secret = "SEC123"

[[notify.channels]]
type = "feishu"
webhook = "https://example.com/feishu"
secret = "feishu-secret"

[[notify.channels]]
type = "ntfy"
topic = "costline-test"

[[notify.channels]]
type = "gotify"
server = "https://gotify.example.com"
token = "gotify-token"

[[notify.channels]]
type = "webhook"
url = "https://example.com/hook"

[notify.channels.headers]
Authorization = "Bearer token"

[[notify.channels]]
type = "desktop"

[[assets]]
kind = "gold"
symbol = "AU9999"
cost = 800.0
"#;
        let config: Config = toml::from_str(text).expect("all channel types should parse");
        config
            .validate()
            .expect("all channel types should validate");
        assert_eq!(config.notify.channels.len(), 11);
    }

    #[test]
    fn target_price_has_priority() {
        let asset = Asset {
            kind: AssetKind::Gold,
            name: None,
            symbol: Some("AU9999".into()),
            secid: None,
            code: None,
            cost: Some(800.0),
            quantity: None,
            principal: None,
            threshold_percent: None,
            target_price: Some(900.0),
        };
        assert_eq!(asset.target_price(10.0), Some(900.0));
    }
}
