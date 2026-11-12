use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::{Asset, AssetKind};

/// 东方财富搜索接口返回的一条候选资产。
#[derive(Debug, Clone, Serialize)]
pub struct SearchItem {
    /// stock / fund / gold
    pub kind: &'static str,
    pub name: String,
    /// 原始代码，例如 600000 / 00700 / 161725 / AU9999
    pub code: String,
    /// 适合直接填进 Asset.symbol 的代码
    pub symbol: Option<String>,
    /// 东方财富 secid，例如 1.600000 / 116.00700 / 118.AU9999
    pub secid: Option<String>,
    /// 市场说明，例如 沪A / 深A / 港股 / 美股 / 基金
    pub market: Option<String>,
}

const SEARCH_TOKEN: &str = "D43BF722C8E33BDC906FB84D85E326E8";

/// 通过名称或代码搜索支持的资产（股票 / 基金 / 黄金）。
pub async fn search_assets(client: &Client, keyword: &str) -> Result<Vec<SearchItem>> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Ok(Vec::new());
    }

    let mut last_error = None;
    for host in [
        "https://searchapi.eastmoney.com",
        "https://searchadapter.eastmoney.com",
    ] {
        match fetch_search(client, host, keyword).await {
            Ok(items) => return Ok(items),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("搜索失败")))
}

async fn fetch_search(client: &Client, host: &str, keyword: &str) -> Result<Vec<SearchItem>> {
    let url = format!("{host}/api/suggest/get");
    let response = client
        .get(&url)
        .query(&[
            ("input", keyword),
            ("type", "14"),
            ("token", SEARCH_TOKEN),
            ("count", "20"),
        ])
        .header("Referer", "https://quote.eastmoney.com/")
        .send()
        .await
        .context("请求东方财富搜索失败")?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("东方财富搜索返回 HTTP {status}：{}", truncate(&body, 200));
    }

    // 接口通常是 JSON，偶尔会包一层 JSONP。
    let trimmed = body.trim().trim_start_matches('\u{feff}');
    let json = if trimmed.starts_with('{') {
        trimmed
    } else {
        extract_jsonp(trimmed).ok_or_else(|| anyhow!("搜索接口返回格式异常"))?
    };
    let value: Value = serde_json::from_str(json).context("解析东方财富搜索 JSON 失败")?;
    let data = value
        .pointer("/QuotationCodeTable/Data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    Ok(data.iter().filter_map(parse_search_item).take(20).collect())
}

fn parse_search_item(value: &Value) -> Option<SearchItem> {
    let code = value
        .get("Code")
        .and_then(Value::as_str)?
        .trim()
        .to_string();
    if code.is_empty() {
        return None;
    }
    let name = value
        .get("Name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(&code)
        .to_string();
    let quote_id = value
        .get("QuoteID")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string);
    let classify = value
        .get("Classify")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    let security_type = value
        .get("SecurityTypeName")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();

    let kind = infer_search_kind(&classify, &security_type, &code, quote_id.as_deref())?;
    let symbol = match kind {
        "fund" => None,
        "stock" => Some(stock_search_symbol(&code, quote_id.as_deref())),
        _ => Some(code.clone()),
    };
    // 场外基金走 fundgz / 蛋卷，不要把它们的 150.* 之类的伪 secid 填进配置。
    let secid = match kind {
        "fund" if !is_exchange_fund(&code) => None,
        _ => quote_id,
    };
    let market = if security_type.is_empty() {
        None
    } else {
        Some(security_type)
    };

    Some(SearchItem {
        kind,
        name,
        code,
        symbol,
        secid,
        market,
    })
}

fn infer_search_kind(
    classify: &str,
    security_type: &str,
    code: &str,
    quote_id: Option<&str>,
) -> Option<&'static str> {
    let quote_id = quote_id.unwrap_or_default();
    let upper_code = code.to_uppercase();

    if classify.contains("index")
        || classify.contains("bond")
        || security_type.contains("指数")
        || security_type.contains("债")
    {
        return None;
    }

    if classify.contains("fund")
        || security_type.contains("基金")
        || security_type.to_uppercase().contains("ETF")
    {
        return Some("fund");
    }

    if classify.contains("gold")
        || classify == "sge"
        || security_type.contains("黄金")
        || security_type.contains("白银")
        || quote_id.starts_with("118.")
        || (quote_id.starts_with("113.") && upper_code.starts_with("AU"))
    {
        return Some("gold");
    }

    let stock_classify = matches!(
        classify,
        "astock" | "abstock" | "hkstock" | "hk" | "usstock" | "stock"
    );
    let stock_quote = quote_id.starts_with("1.")
        || quote_id.starts_with("0.")
        || quote_id.starts_with("105.")
        || quote_id.starts_with("106.")
        || quote_id.starts_with("107.")
        || quote_id.starts_with("116.");
    if stock_classify || stock_quote {
        return Some("stock");
    }

    None
}

fn stock_search_symbol(code: &str, quote_id: Option<&str>) -> String {
    if quote_id.is_some_and(|id| id.starts_with("116.")) && !code.to_uppercase().ends_with(".HK") {
        return format!("{code}.HK");
    }
    code.to_string()
}

#[derive(Debug, Clone)]
pub struct Quote {
    pub price: f64,
    pub prev_close: Option<f64>,
    pub name: Option<String>,
    /// 数据说明，例如“估值 2026-11-03 15:00”
    pub note: String,
    /// CNY / HKD / USD
    pub currency: &'static str,
    #[allow(dead_code)]
    pub source: &'static str,
}

impl Quote {
    pub fn change_amount(&self) -> Option<f64> {
        self.prev_close
            .filter(|prev| *prev > 0.0)
            .map(|prev| self.price - prev)
    }

    pub fn change_percent(&self) -> Option<f64> {
        self.prev_close
            .filter(|prev| *prev > 0.0)
            .map(|prev| (self.price / prev - 1.0) * 100.0)
    }
}

/// 国内金价参照：上海黄金交易所 Au99.99（元/克）。
pub const DOMESTIC_GOLD_SECID: &str = "118.AU9999";

/// 黄金相关资产附带的国内金价，便于对照黄金基金 / 国际金价。
#[derive(Debug, Clone, Serialize)]
pub struct GoldReference {
    pub price: f64,
    pub change_percent: Option<f64>,
    /// 例如「上海金 Au99.99（元/克）」
    pub note: String,
}

/// 是否属于黄金相关品种：黄金本身，或名称含「黄金 / Gold / 金ETF」的基金、股票。
pub fn is_gold_related(asset: &Asset, quote_name: Option<&str>) -> bool {
    if asset.kind == AssetKind::Gold {
        return true;
    }
    asset
        .name
        .as_deref()
        .into_iter()
        .chain(quote_name)
        .any(mentions_gold)
}

fn mentions_gold(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("黄金") || lower.contains("gold") || lower.contains("金etf")
}

/// 获取国内金价（上海金 Au99.99，元/克）。
pub async fn fetch_domestic_gold(client: &Client) -> Result<GoldReference> {
    let mut errors = Vec::new();
    match fetch_eastmoney(client, DOMESTIC_GOLD_SECID).await {
        Ok(quote) => return Ok(domestic_gold_reference(&quote)),
        Err(error) => errors.push(format!("{error:#}")),
    }
    match fetch_sina(client, "gds_AU9999").await {
        Ok(quote) => return Ok(domestic_gold_reference(&quote)),
        Err(error) => errors.push(format!("{error:#}")),
    }
    Err(anyhow!("{}", errors.join("；")))
}

fn domestic_gold_reference(quote: &Quote) -> GoldReference {
    GoldReference {
        price: quote.price,
        change_percent: quote.change_percent(),
        note: "上海金 Au99.99（元/克）".to_string(),
    }
}

pub async fn fetch_quote(client: &Client, asset: &Asset) -> Result<Quote> {
    if let Some(secid) = &asset.secid {
        return fetch_quote_by_secids(client, std::slice::from_ref(secid)).await;
    }

    match asset.kind {
        AssetKind::Stock => {
            let raw = asset
                .symbol
                .as_deref()
                .ok_or_else(|| anyhow!("股票缺少 symbol"))?;
            let secids = stock_secids(raw)?;
            fetch_quote_by_secids(client, &secids).await
        }
        AssetKind::Fund => fetch_fund(client, asset).await,
        AssetKind::Gold => fetch_gold(client, asset).await,
    }
}

async fn fetch_fund(client: &Client, asset: &Asset) -> Result<Quote> {
    let raw = asset
        .code
        .as_deref()
        .or(asset.symbol.as_deref())
        .ok_or_else(|| anyhow!("基金缺少 code"))?;

    if is_exchange_fund(raw) {
        let market = if raw.starts_with("15") { "0" } else { "1" };
        return fetch_quote_by_secids(client, &[format!("{market}.{raw}")]).await;
    }

    // 场外基金：估值 -> 蛋卷 -> 新浪 -> 天天基金净值历史，逐个回退。
    let mut errors = Vec::new();
    match fetch_fundgz(client, raw).await {
        Ok(quote) => return Ok(quote),
        Err(error) => errors.push(format!("估值接口：{error:#}")),
    }
    match fetch_danjuan(client, raw).await {
        Ok(quote) => return Ok(quote),
        Err(error) => errors.push(format!("蛋卷接口：{error:#}")),
    }
    match fetch_sina(client, &format!("f_{raw}")).await {
        Ok(quote) => return Ok(quote),
        Err(error) => errors.push(format!("新浪接口：{error:#}")),
    }
    match fetch_fund_lsjz(client, raw).await {
        Ok(quote) => return Ok(quote),
        Err(error) => errors.push(format!("净值历史接口：{error:#}")),
    }

    Err(anyhow!("基金 {raw} 获取失败。{}", errors.join("；")))
}

async fn fetch_gold(client: &Client, asset: &Asset) -> Result<Quote> {
    if let Some(symbol) = asset.symbol.as_deref() {
        let symbol = symbol.trim();
        if symbol.starts_with("hf_") || symbol.eq_ignore_ascii_case("XAU") {
            let sina_symbol = if symbol.eq_ignore_ascii_case("XAU") {
                "hf_XAU".to_string()
            } else {
                symbol.to_string()
            };
            return fetch_sina(client, &sina_symbol).await;
        }
    }

    let secids = gold_secids(asset.symbol.as_deref())?;
    fetch_quote_by_secids(client, &secids).await
}

/// 先按 secid 走东方财富，失败后按 secid 推导腾讯 / 新浪代码继续回退。
async fn fetch_quote_by_secids(client: &Client, secids: &[String]) -> Result<Quote> {
    let mut errors = Vec::new();
    for secid in secids {
        match fetch_eastmoney(client, secid).await {
            Ok(quote) => return Ok(quote),
            Err(error) => errors.push(format!("{error:#}")),
        }
    }

    let mut tried: Vec<String> = Vec::new();
    for secid in secids {
        for (source, symbol) in fallback_symbols(secid) {
            if tried.iter().any(|item| item == &symbol) {
                continue;
            }
            tried.push(symbol.clone());
            let result = match source {
                FallbackSource::Tencent => fetch_tencent(client, &symbol).await,
                FallbackSource::Sina => fetch_sina(client, &symbol).await,
            };
            match result {
                Ok(quote) => return Ok(quote),
                Err(error) => errors.push(format!("{error:#}")),
            }
        }
    }

    Err(anyhow!("{}", errors.join("；")))
}

#[derive(Debug, Clone, Copy)]
enum FallbackSource {
    Tencent,
    Sina,
}

/// 把东方财富 secid 映射成腾讯 / 新浪的行情代码。
fn fallback_symbols(secid: &str) -> Vec<(FallbackSource, String)> {
    let Some((market, code)) = secid.split_once('.') else {
        return Vec::new();
    };
    let code = code.trim();
    if code.is_empty() || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Vec::new();
    }

    match market {
        "1" => vec![
            (FallbackSource::Tencent, format!("sh{code}")),
            (FallbackSource::Sina, format!("sh{code}")),
        ],
        "0" => {
            // BJ exchange codes start with 4 / 8 / 92; everything else is SZ.
            let prefix = if code.starts_with('4') || code.starts_with('8') || code.starts_with("92")
            {
                "bj"
            } else {
                "sz"
            };
            vec![
                (FallbackSource::Tencent, format!("{prefix}{code}")),
                (FallbackSource::Sina, format!("{prefix}{code}")),
            ]
        }
        "116" => vec![(FallbackSource::Tencent, format!("hk{}", pad_left(code, 5)))],
        "105" | "106" | "107" => {
            let upper = code.to_uppercase();
            vec![
                (FallbackSource::Tencent, format!("us{upper}")),
                (FallbackSource::Sina, format!("gb_{}", upper.to_lowercase())),
            ]
        }
        "118" => vec![(FallbackSource::Sina, format!("gds_{}", code.to_uppercase()))],
        // Eastmoney 113.aum = SHFE gold main contract, Sina calls it nf_AU0.
        "113" => match code.to_lowercase().as_str() {
            "aum" => vec![(FallbackSource::Sina, "nf_AU0".to_string())],
            "agm" => vec![(FallbackSource::Sina, "nf_AG0".to_string())],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

async fn fetch_eastmoney(client: &Client, secid: &str) -> Result<Quote> {
    let url = format!(
        "https://push2.eastmoney.com/api/qt/stock/get?secid={secid}&fields=f43,f57,f58,f59,f60"
    );
    let response = client
        .get(&url)
        .header("Referer", "https://quote.eastmoney.com/")
        .send()
        .await
        .with_context(|| format!("请求东方财富行情失败（{secid}）"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "东方财富返回 HTTP {status}（{secid}）：{}",
            truncate(&body, 200)
        );
    }

    let value: Value = serde_json::from_str(&body)
        .with_context(|| format!("解析东方财富 JSON 失败（{secid}）"))?;
    if value.get("rc").and_then(Value::as_i64) != Some(0) {
        bail!("东方财富接口返回错误（{secid}）：{}", truncate(&body, 200));
    }
    let data = value
        .get("data")
        .filter(|data| !data.is_null())
        .ok_or_else(|| anyhow!("未找到证券 {secid}，请检查代码或改用 secid"))?;

    let raw_price = json_number(data.get("f43"))
        .ok_or_else(|| anyhow!("{secid} 没有最新价，可能停牌或代码错误"))?;
    let decimals = json_number(data.get("f59")).unwrap_or(2.0) as i32;
    let price = scale_price(raw_price, decimals);
    if !price.is_finite() || price <= 0.0 {
        bail!("{secid} 无有效报价，可能停牌或代码错误");
    }
    let prev_close = json_number(data.get("f60"))
        .map(|value| scale_price(value, decimals))
        .filter(|value| *value > 0.0);
    let name = data
        .get("f58")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_string);

    Ok(Quote {
        price,
        prev_close,
        name,
        note: "东方财富实时".to_string(),
        currency: currency_of_secid(secid),
        source: "eastmoney",
    })
}

async fn fetch_fundgz(client: &Client, code: &str) -> Result<Quote> {
    let timestamp = chrono::Utc::now().timestamp_millis();
    let url = format!("https://fundgz.1234567.com.cn/js/{code}.js?rt={timestamp}");
    let response = client
        .get(&url)
        .header("Referer", "https://fund.eastmoney.com/")
        .send()
        .await
        .context("请求天天基金估值失败")?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("天天基金返回 HTTP {status}");
    }

    let json = extract_jsonp(&body).ok_or_else(|| anyhow!("返回内容不是基金估值 JSONP"))?;
    let value: Value = serde_json::from_str(json).context("解析基金估值 JSON 失败")?;
    let price = json_number(value.get("gsz"))
        .filter(|price| *price > 0.0)
        .ok_or_else(|| anyhow!("该基金没有盘中估值，可能为货币基金、QDII 或已停止估值"))?;
    let prev_close = json_number(value.get("dwjz")).filter(|nav| *nav > 0.0);
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    let estimate_time = value
        .get("gztime")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let note = if estimate_time.is_empty() {
        "天天基金估值".to_string()
    } else {
        format!("估值 {estimate_time}")
    };

    Ok(Quote {
        price,
        prev_close,
        name,
        note,
        currency: "CNY",
        source: "fundgz",
    })
}

async fn fetch_danjuan(client: &Client, code: &str) -> Result<Quote> {
    let url = format!("https://danjuanfunds.com/djapi/fund/{code}");
    let response = client
        .get(&url)
        .header("Referer", "https://danjuanfunds.com/")
        .send()
        .await
        .context("请求蛋卷基金失败")?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("蛋卷基金返回 HTTP {status}");
    }
    let value: Value = serde_json::from_str(&body).context("解析蛋卷基金 JSON 失败")?;
    if value.get("result_code").and_then(Value::as_i64) != Some(0) {
        bail!("蛋卷基金返回错误：{}", truncate(&body, 200));
    }
    let data = value
        .get("data")
        .filter(|data| !data.is_null())
        .ok_or_else(|| anyhow!("蛋卷基金没有返回数据"))?;
    let name = data
        .get("fd_name")
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty())
        .map(str::to_string);

    // 净值历史一次取两条，用前一条净值计算日涨跌，避免面板上的日涨跌缺失。
    match fetch_danjuan_history(client, code).await {
        Ok((nav, prev_close, date)) => Ok(Quote {
            price: nav,
            prev_close,
            name,
            note: format!("最新净值 {date}"),
            currency: "CNY",
            source: "danjuan",
        }),
        Err(history_error) => {
            if let Some(derived) = data.get("fund_derived") {
                if let Some(nav) = json_number(derived.get("unit_nav")).filter(|nav| *nav > 0.0) {
                    let end_date = derived
                        .get("end_date")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let note = if end_date.is_empty() {
                        "蛋卷最新净值".to_string()
                    } else {
                        format!("最新净值 {end_date}")
                    };
                    return Ok(Quote {
                        price: nav,
                        prev_close: None,
                        name,
                        note,
                        currency: "CNY",
                        source: "danjuan",
                    });
                }
            }
            Err(history_error)
        }
    }
}

async fn fetch_danjuan_history(client: &Client, code: &str) -> Result<(f64, Option<f64>, String)> {
    let url = format!("https://danjuanfunds.com/djapi/fund/nav/history/{code}?page=1&size=2");
    let response = client
        .get(&url)
        .header("Referer", "https://danjuanfunds.com/")
        .send()
        .await
        .context("请求蛋卷基金净值历史失败")?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("蛋卷基金净值接口返回 HTTP {status}");
    }
    let value: Value = serde_json::from_str(&body).context("解析蛋卷基金净值 JSON 失败")?;
    if value.get("result_code").and_then(Value::as_i64) != Some(0) {
        bail!("蛋卷基金净值接口返回错误：{}", truncate(&body, 200));
    }
    let items = value
        .get("data")
        .and_then(|data| data.get("items"))
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| anyhow!("蛋卷基金没有净值数据"))?;
    let item = &items[0];
    let nav = json_number(item.get("nav"))
        .filter(|nav| *nav > 0.0)
        .ok_or_else(|| anyhow!("蛋卷基金净值格式异常"))?;
    let prev_close = items
        .get(1)
        .and_then(|item| json_number(item.get("nav")))
        .filter(|nav| *nav > 0.0);
    let date = item
        .get("date")
        .and_then(Value::as_str)
        .unwrap_or("未知日期")
        .to_string();
    Ok((nav, prev_close, date))
}

/// 场外基金申购状态（天天基金基础信息接口）。
///
/// 会随状态文件一起持久化，用于在面板展示并在定投到期时判断能否买入。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FundTradeStatus {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub name: Option<String>,
    /// SGZT，例如「开放申购」「限大额」「暂停申购」
    #[serde(default)]
    pub subscribe_status: Option<String>,
    /// SHZT，例如「开放赎回」「暂停赎回」
    #[serde(default)]
    pub redeem_status: Option<String>,
    /// BUY，接口给出的布尔可买标记
    #[serde(default)]
    pub buy_enabled: Option<bool>,
    /// ISBUY，1 表示可买，4 表示暂停申购
    #[serde(default)]
    pub buy_code: Option<i64>,
    /// MINSG，最小申购金额
    #[serde(default)]
    pub min_amount: Option<f64>,
    /// MAXSG，限额；已过滤 100000000000 这类“无上限”哨兵值
    #[serde(default)]
    pub max_amount: Option<f64>,
    /// ISABNORMAL，基金状态是否异常
    #[serde(default)]
    pub abnormal: bool,
    /// 状态查询时间（Unix 秒）
    #[serde(default)]
    pub checked_at: i64,
    /// 最近一次查询失败的原因；成功时为空
    #[serde(default)]
    pub error: Option<String>,
}

/// 本期定投金额的申购判断结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubscribeVerdict {
    /// 可以正常申购
    Open,
    /// 暂停申购 / 封闭 / 超过限额
    Blocked,
    /// 接口没有返回足够的信息
    Unknown,
}

impl FundTradeStatus {
    /// 判断本期金额能否申购，返回 (结论, 说明)。
    pub fn check_amount(&self, amount: f64) -> (SubscribeVerdict, Option<String>) {
        let status = self.subscribe_status.as_deref().unwrap_or_default().trim();

        if self.abnormal {
            return (
                SubscribeVerdict::Blocked,
                Some("基金状态异常（ISABNORMAL）".to_string()),
            );
        }
        if status.contains("暂停申购") || status.contains("封闭") {
            return (SubscribeVerdict::Blocked, Some(status.to_string()));
        }
        if self.buy_enabled == Some(false) || self.buy_code == Some(4) {
            let reason = if status.is_empty() {
                "接口标记为不可申购".to_string()
            } else {
                status.to_string()
            };
            return (SubscribeVerdict::Blocked, Some(reason));
        }
        if let Some(min) = self.min_amount {
            if amount < min {
                return (
                    SubscribeVerdict::Blocked,
                    Some(format!("本期金额低于最小申购金额 {min:.2}")),
                );
            }
        }
        if status.contains("限大额") {
            if let Some(max) = self.max_amount {
                if amount > max {
                    return (
                        SubscribeVerdict::Blocked,
                        Some(format!("限大额，单笔上限 {max:.2}")),
                    );
                }
            }
            if let Some(error) = &self.error {
                return (
                    SubscribeVerdict::Unknown,
                    Some(format!("状态查询失败：{error}")),
                );
            }
            return (SubscribeVerdict::Open, Some(status.to_string()));
        }
        if status.is_empty() && self.buy_enabled.is_none() && self.buy_code.is_none() {
            let reason = self
                .error
                .clone()
                .unwrap_or_else(|| "接口未返回申购状态".to_string());
            return (SubscribeVerdict::Unknown, Some(reason));
        }
        // 有查询错误时，即使缓存里是“开放申购”也要求人工确认，避免误判。
        if let Some(error) = &self.error {
            return (
                SubscribeVerdict::Unknown,
                Some(format!("状态查询失败：{error}")),
            );
        }

        let reason = if status.is_empty() {
            "可申购".to_string()
        } else {
            status.to_string()
        };
        (SubscribeVerdict::Open, Some(reason))
    }
}

/// 查询场外基金的申购状态。场内 ETF / LOF 不适用该接口。
pub async fn fetch_fund_trade_status(client: &Client, code: &str) -> Result<FundTradeStatus> {
    let code = code.trim();
    if code.is_empty() {
        bail!("基金代码不能为空");
    }

    let url = "https://fundmobapi.eastmoney.com/FundMNewApi/FundMNNBasicInformation";
    let response = client
        .get(url)
        .query(&[
            ("FCODE", code),
            ("deviceid", "Wap"),
            ("plat", "Wap"),
            ("product", "EFund"),
            ("version", "2.0.0"),
        ])
        .header("Referer", "https://fund.eastmoney.com/")
        .send()
        .await
        .with_context(|| format!("请求基金 {code} 申购状态失败"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("申购状态接口返回 HTTP {status}：{}", truncate(&body, 200));
    }

    let value: Value = serde_json::from_str(&body).context("解析基金申购状态 JSON 失败")?;
    let ok = value
        .get("ErrCode")
        .map(|code| json_i64(Some(code)) == Some(0))
        .unwrap_or(true);
    if !ok {
        bail!("申购状态接口返回错误：{}", truncate(&body, 200));
    }
    let data = value
        .get("Datas")
        .filter(|data| !data.is_null())
        .ok_or_else(|| anyhow!("基金 {code} 没有返回申购状态"))?;

    Ok(parse_fund_trade_status(data, code))
}

fn parse_fund_trade_status(data: &Value, fallback_code: &str) -> FundTradeStatus {
    FundTradeStatus {
        code: json_text(data.get("FCODE")).unwrap_or_else(|| fallback_code.to_string()),
        name: json_text(data.get("SHORTNAME")),
        subscribe_status: json_text(data.get("SGZT")),
        redeem_status: json_text(data.get("SHZT")),
        buy_enabled: json_bool(data.get("BUY")),
        buy_code: json_i64(data.get("ISBUY")),
        min_amount: json_number(data.get("MINSG")).filter(|value| *value > 0.0),
        max_amount: json_number(data.get("MAXSG")).filter(|value| *value > 0.0 && *value < 1.0e11),
        abnormal: json_i64(data.get("ISABNORMAL")) == Some(1),
        checked_at: chrono::Utc::now().timestamp(),
        error: None,
    }
}

/// 腾讯行情：`https://qt.gtimg.cn/q=sh601288`，覆盖 A 股 / 港股 / 美股 / 场内基金。
async fn fetch_tencent(client: &Client, symbol: &str) -> Result<Quote> {
    let url = format!("https://qt.gtimg.cn/q={symbol}");
    let response = client
        .get(&url)
        .header("Referer", "https://gu.qq.com/")
        .send()
        .await
        .with_context(|| format!("请求腾讯行情失败（{symbol}）"))?;
    let status = response.status();
    let body = read_gbk(response).await.context("读取腾讯行情响应失败")?;
    if !status.is_success() {
        bail!(
            "腾讯行情返回 HTTP {status}（{symbol}）：{}",
            truncate(&body, 200)
        );
    }
    parse_tencent_quote(symbol, &body)
}

fn parse_tencent_quote(symbol: &str, body: &str) -> Result<Quote> {
    let data = extract_quoted(body).ok_or_else(|| anyhow!("腾讯行情返回格式异常（{symbol}）"))?;
    let fields: Vec<&str> = data.split('~').collect();
    // 代码不存在时返回 v_pv_none_match="1"，字段数不足。
    if fields.len() < 5 {
        bail!("腾讯行情没有 {symbol} 的报价");
    }

    let price = fields[3]
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|price| *price > 0.0)
        .ok_or_else(|| anyhow!("腾讯行情没有 {symbol} 的有效价格"))?;
    let prev_close = fields[4]
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|price| *price > 0.0);
    let name = fields[1].trim();

    Ok(Quote {
        price,
        prev_close,
        name: (!name.is_empty()).then(|| name.to_string()),
        note: "腾讯行情".to_string(),
        currency: currency_of_symbol(symbol),
        source: "tencent",
    })
}

/// 新浪行情：覆盖 A 股 / 北交所 / 美股（`gb_`）/ 上海金（`gds_`）/ 场外基金（`f_`）/ 国际金价（`hf_`）。
async fn fetch_sina(client: &Client, symbol: &str) -> Result<Quote> {
    let url = format!("https://hq.sinajs.cn/list={symbol}");
    let response = client
        .get(&url)
        .header("Referer", "https://finance.sina.com.cn/")
        .send()
        .await
        .with_context(|| format!("请求新浪行情失败（{symbol}）"))?;
    let status = response.status();
    let body = read_gbk(response).await.context("读取新浪行情响应失败")?;
    if !status.is_success() {
        bail!(
            "新浪行情返回 HTTP {status}（{symbol}）：{}",
            truncate(&body, 200)
        );
    }
    parse_sina_quote(symbol, &body)
}

fn parse_sina_quote(symbol: &str, body: &str) -> Result<Quote> {
    let data = extract_quoted(body).ok_or_else(|| anyhow!("新浪行情返回格式异常（{symbol}）"))?;
    let fields: Vec<&str> = data.split(',').collect();
    let number = |index: usize| {
        fields
            .get(index)
            .and_then(|value: &&str| value.trim().parse::<f64>().ok())
            .filter(|value| *value > 0.0)
    };
    let text = |index: usize| {
        fields
            .get(index)
            .map(|value: &&str| value.trim())
            .filter(|value: &&str| !value.is_empty())
            .map(str::to_string)
    };

    // 场外基金：名称, 单位净值, 累计净值, 前一日净值, 日期, 规模
    if symbol.starts_with("f_") {
        let price = number(1).ok_or_else(|| anyhow!("新浪没有 {symbol} 的有效净值"))?;
        let date = text(4).unwrap_or_default();
        let note = if date.is_empty() {
            "新浪最新净值".to_string()
        } else {
            format!("最新净值 {date}")
        };
        return Ok(Quote {
            price,
            prev_close: number(3),
            name: text(0),
            note,
            currency: "CNY",
            source: "sina",
        });
    }

    // 国内期货主连：名称, 时间, 开, 高, 低, 昨收, ... 最新价(8), 结算价(9), 昨结算(10)
    if symbol.starts_with("nf_") {
        let price = number(8).ok_or_else(|| anyhow!("新浪没有 {symbol} 的有效报价"))?;
        return Ok(Quote {
            price,
            prev_close: number(10).or_else(|| number(5)),
            name: text(0),
            note: "新浪期货行情".to_string(),
            currency: "CNY",
            source: "sina",
        });
    }

    // 上海金 / 国际金价：现价, ..., 昨收(7), ..., 名称(13)
    if symbol.starts_with("gds_") || symbol.starts_with("hf_") {
        let price = number(0).ok_or_else(|| anyhow!("新浪没有 {symbol} 的有效报价"))?;
        let currency = if symbol.starts_with("hf_") {
            "USD"
        } else {
            "CNY"
        };
        let note = if currency == "USD" {
            "新浪国际金价（USD/盎司）".to_string()
        } else {
            "新浪黄金报价".to_string()
        };
        return Ok(Quote {
            price,
            prev_close: number(7),
            name: text(13),
            note,
            currency,
            source: "sina",
        });
    }

    // 美股：名称, 现价, 涨跌幅%, 时间, 涨跌额, 今开, 最高, 最低 ...
    if symbol.starts_with("gb_") {
        let price = number(1).ok_or_else(|| anyhow!("新浪没有 {symbol} 的有效报价"))?;
        let change = fields
            .get(4)
            .and_then(|value: &&str| value.trim().parse::<f64>().ok());
        let prev_close = change
            .map(|change| price - change)
            .filter(|value| *value > 0.0);
        return Ok(Quote {
            price,
            prev_close,
            name: text(0),
            note: "新浪美股行情".to_string(),
            currency: "USD",
            source: "sina",
        });
    }

    // A 股 / 北交所：名称, 今开, 昨收, 现价, 最高, 最低 ...
    let price = number(3).ok_or_else(|| anyhow!("新浪没有 {symbol} 的有效报价"))?;
    Ok(Quote {
        price,
        prev_close: number(2),
        name: text(0),
        note: "新浪行情".to_string(),
        currency: "CNY",
        source: "sina",
    })
}

fn currency_of_symbol(symbol: &str) -> &'static str {
    if symbol.starts_with("hk") {
        "HKD"
    } else if symbol.starts_with("us") {
        "USD"
    } else {
        "CNY"
    }
}

async fn read_gbk(response: reqwest::Response) -> Result<String> {
    let bytes = response.bytes().await.context("读取响应内容失败")?;
    let (text, _, _) = encoding_rs::GB18030.decode(&bytes);
    Ok(text.into_owned())
}

/// 天天基金净值历史（`api.fund.eastmoney.com`，与 push2 不是同一套接口）。
async fn fetch_fund_lsjz(client: &Client, code: &str) -> Result<Quote> {
    let url = "https://api.fund.eastmoney.com/f10/lsjz";
    let response = client
        .get(url)
        .query(&[("fundCode", code), ("pageIndex", "1"), ("pageSize", "2")])
        .header(
            "Referer",
            format!("https://fundf10.eastmoney.com/jjjz_{code}.html"),
        )
        .send()
        .await
        .context("请求基金净值历史失败")?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("基金净值历史返回 HTTP {status}：{}", truncate(&body, 200));
    }

    let value: Value = serde_json::from_str(&body).context("解析基金净值历史 JSON 失败")?;
    if value
        .get("ErrCode")
        .is_some_and(|err| json_i64(Some(err)) != Some(0))
    {
        bail!("基金净值历史返回错误：{}", truncate(&body, 200));
    }
    let items = value
        .pointer("/Data/LSJZList")
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
        .ok_or_else(|| anyhow!("基金净值历史没有数据"))?;

    let nav = json_number(items[0].get("DWJZ"))
        .filter(|nav| *nav > 0.0)
        .ok_or_else(|| anyhow!("基金净值历史格式异常"))?;
    let date = items[0]
        .get("FSRQ")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let prev_close = items
        .get(1)
        .and_then(|item| json_number(item.get("DWJZ")))
        .filter(|nav| *nav > 0.0)
        .or_else(|| {
            let percent = json_number(items[0].get("JZZZL"))?;
            let prev = nav / (1.0 + percent / 100.0);
            (prev > 0.0).then_some(prev)
        });

    let note = if date.is_empty() {
        "最新净值".to_string()
    } else {
        format!("最新净值 {date}")
    };
    Ok(Quote {
        price: nav,
        prev_close,
        name: None,
        note,
        currency: "CNY",
        source: "eastmoney",
    })
}

pub fn stock_secids(raw: &str) -> Result<Vec<String>> {
    let symbol = raw.trim();
    if symbol.is_empty() {
        bail!("股票代码不能为空");
    }

    // 已经是 secid，例如 1.600000 / 116.00700 / 105.AAPL
    if let Some((left, right)) = symbol.split_once('.') {
        if !left.is_empty()
            && left.chars().all(|c| c.is_ascii_digit())
            && !right.is_empty()
            && !matches!(
                right.to_uppercase().as_str(),
                "SH" | "SZ" | "BJ" | "HK" | "US"
            )
        {
            return Ok(vec![symbol.to_string()]);
        }
    }

    // 后缀写法：600000.SH / 00700.HK / AAPL.US
    if let Some((code, suffix)) = symbol.rsplit_once('.') {
        return match suffix.to_uppercase().as_str() {
            "SH" => Ok(vec![format!("1.{code}")]),
            "SZ" | "BJ" => Ok(vec![format!("0.{code}")]),
            "HK" => Ok(vec![format!("116.{}", pad_left(code, 5))]),
            "US" => Ok(us_secids(&code.to_uppercase())),
            _ => Err(anyhow!("无法识别的股票代码：{symbol}，可改用 secid 配置")),
        };
    }

    let upper = symbol.to_uppercase();
    // 只有前缀后面的部分全是数字时才当作市场前缀，避免把 SHOP / USB 这类美股代码误判
    if let Some(code) = upper.strip_prefix("SH") {
        if !code.is_empty() && code.chars().all(|c| c.is_ascii_digit()) {
            return Ok(vec![format!("1.{code}")]);
        }
    }
    if let Some(code) = upper.strip_prefix("SZ") {
        if !code.is_empty() && code.chars().all(|c| c.is_ascii_digit()) {
            return Ok(vec![format!("0.{code}")]);
        }
    }
    if let Some(code) = upper.strip_prefix("BJ") {
        if !code.is_empty() && code.chars().all(|c| c.is_ascii_digit()) {
            return Ok(vec![format!("0.{code}")]);
        }
    }
    if let Some(code) = upper.strip_prefix("HK") {
        if !code.is_empty() && code.chars().all(|c| c.is_ascii_digit()) {
            return Ok(vec![format!("116.{}", pad_left(code, 5))]);
        }
    }

    if symbol.chars().all(|c| c.is_ascii_digit()) {
        return match symbol.len() {
            6 => {
                // 92xxxx is BJ exchange; 6xxxxx and 900xxx (SH B share) use market 1.
                let market = if symbol.starts_with('6')
                    || (symbol.starts_with('9') && !symbol.starts_with("92"))
                {
                    "1"
                } else {
                    "0"
                };
                Ok(vec![format!("{market}.{symbol}")])
            }
            4 | 5 => Ok(vec![format!("116.{}", pad_left(symbol, 5))]),
            _ => Err(anyhow!(
                "无法识别的股票代码：{symbol}（6 位为 A 股，4-5 位为港股）"
            )),
        };
    }

    if symbol.chars().all(|c| c.is_ascii_alphabetic()) {
        return Ok(us_secids(&upper));
    }

    Err(anyhow!("无法识别的股票代码：{symbol}，可改用 secid 配置"))
}

pub fn gold_secids(raw: Option<&str>) -> Result<Vec<String>> {
    let Some(raw) = raw.map(str::trim).filter(|raw| !raw.is_empty()) else {
        return Ok(vec!["118.AU9999".to_string()]);
    };
    let upper = raw.to_uppercase();
    let normalized = upper.replace([' ', '_'], "");

    let secid = match normalized.as_str() {
        "AU9999" | "AU99.99" | "GDSAU9999" | "SGEAU9999" => "118.AU9999",
        "AUTD" | "AU(T+D)" | "GDSAUTD" => "118.AUTD",
        "AU100G" | "GDSAU100G" => "118.AU100G",
        "AG9999" => "118.AG9999",
        "AU0" | "NFAU0" | "AUM" => "113.aum",
        _ => {
            if raw.contains('.') {
                raw
            } else {
                return Ok(vec![format!("118.{upper}")]);
            }
        }
    };

    Ok(vec![secid.to_string()])
}

pub fn is_exchange_fund(code: &str) -> bool {
    let code = code.trim();
    code.len() == 6
        && code.chars().all(|c| c.is_ascii_digit())
        && (code.starts_with('5') || code.starts_with("15"))
}

fn us_secids(code: &str) -> Vec<String> {
    // 105 = NASDAQ，106 = NYSE，107 = AMEX；依次尝试
    vec![
        format!("105.{code}"),
        format!("106.{code}"),
        format!("107.{code}"),
    ]
}

fn pad_left(value: &str, width: usize) -> String {
    if value.len() >= width {
        value.to_string()
    } else {
        format!("{}{}", "0".repeat(width - value.len()), value)
    }
}

fn currency_of_secid(secid: &str) -> &'static str {
    if secid.starts_with("116.") {
        "HKD"
    } else if secid.starts_with("105.") || secid.starts_with("106.") || secid.starts_with("107.") {
        "USD"
    } else {
        "CNY"
    }
}

fn scale_price(value: f64, decimals: i32) -> f64 {
    value / 10f64.powi(decimals)
}

fn json_number(value: Option<&Value>) -> Option<f64> {
    value.and_then(|value| {
        value.as_f64().or_else(|| {
            value
                .as_str()
                .and_then(|text| text.trim().parse::<f64>().ok())
        })
    })
}

fn json_bool(value: Option<&Value>) -> Option<bool> {
    value.and_then(|value| {
        value.as_bool().or_else(|| {
            value.as_str().and_then(|text| match text.trim() {
                "true" | "True" | "1" => Some(true),
                "false" | "False" | "0" => Some(false),
                _ => None,
            })
        })
    })
}

fn json_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))
    })
}

fn json_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty() && *text != "--")
        .map(str::to_string)
}

// Extract the quoted payload of `var xxx="...";`.
fn extract_quoted(body: &str) -> Option<&str> {
    let start = body.find('"')? + 1;
    let end = body.rfind('"')?;
    if end < start {
        return None;
    }
    Some(body[start..end].trim())
}

fn extract_jsonp(body: &str) -> Option<&str> {
    let start = body.find('(')? + 1;
    let end = body.rfind(')')?;
    if end <= start {
        return None;
    }
    Some(body[start..end].trim())
}

fn truncate(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_a_share_codes() {
        assert_eq!(stock_secids("sh600000").unwrap(), vec!["1.600000"]);
        assert_eq!(stock_secids("600000").unwrap(), vec!["1.600000"]);
        assert_eq!(stock_secids("600000.SH").unwrap(), vec!["1.600000"]);
        assert_eq!(stock_secids("000001").unwrap(), vec!["0.000001"]);
        assert_eq!(stock_secids("sz000001").unwrap(), vec!["0.000001"]);
        assert_eq!(stock_secids("1.600000").unwrap(), vec!["1.600000"]);
    }

    #[test]
    fn maps_hk_and_us_codes() {
        assert_eq!(stock_secids("00700.HK").unwrap(), vec!["116.00700"]);
        assert_eq!(stock_secids("hk700").unwrap(), vec!["116.00700"]);
        assert_eq!(
            stock_secids("AAPL").unwrap(),
            vec!["105.AAPL", "106.AAPL", "107.AAPL"]
        );
    }

    #[test]
    fn maps_gold_codes() {
        assert_eq!(gold_secids(None).unwrap(), vec!["118.AU9999"]);
        assert_eq!(gold_secids(Some("AU9999")).unwrap(), vec!["118.AU9999"]);
        assert_eq!(gold_secids(Some("114.AU9999")).unwrap(), vec!["114.AU9999"]);
        assert_eq!(gold_secids(Some("nf_AU0")).unwrap(), vec!["113.aum"]);
    }

    #[test]
    fn detects_exchange_funds() {
        assert!(is_exchange_fund("518880"));
        assert!(is_exchange_fund("159915"));
        assert!(!is_exchange_fund("161725"));
    }

    #[test]
    fn detects_gold_related_assets() {
        let asset = |kind: AssetKind, name: Option<&str>| Asset {
            kind,
            name: name.map(str::to_string),
            symbol: None,
            secid: None,
            code: None,
            cost: None,
            quantity: None,
            principal: None,
            as_of: None,
            trades: Vec::new(),
            threshold_percent: None,
            target_price: None,
            sip: None,
        };

        assert!(is_gold_related(&asset(AssetKind::Gold, None), None));
        assert!(is_gold_related(
            &asset(AssetKind::Fund, Some("华安黄金ETF联接A")),
            None
        ));
        assert!(is_gold_related(
            &asset(AssetKind::Fund, None),
            Some("黄金ETF")
        ));
        assert!(is_gold_related(
            &asset(AssetKind::Stock, Some("山东黄金")),
            None
        ));
        assert!(!is_gold_related(
            &asset(AssetKind::Fund, Some("招商中证白酒")),
            Some("白酒基金")
        ));
    }

    #[test]
    fn parses_fund_jsonp() {
        let body = r#"jsonpgz({"fundcode":"161725","name":"招商中证白酒","gsz":"0.6120"});"#;
        assert_eq!(
            extract_jsonp(body).unwrap(),
            r#"{"fundcode":"161725","name":"招商中证白酒","gsz":"0.6120"}"#
        );
    }

    #[test]
    fn scales_prices() {
        assert!((scale_price(1010.0, 2) - 10.10).abs() < 1e-9);
        assert!((scale_price(7989.0, 3) - 7.989).abs() < 1e-9);
    }

    #[test]
    fn currency_by_market() {
        assert_eq!(currency_of_secid("116.00700"), "HKD");
        assert_eq!(currency_of_secid("105.AAPL"), "USD");
        assert_eq!(currency_of_secid("1.600000"), "CNY");
    }

    #[test]
    fn detects_blocked_fund_status() {
        let suspended = parse_fund_trade_status(
            &serde_json::json!({
                "FCODE": "501018",
                "SGZT": "暂停申购",
                "SHZT": "开放赎回",
                "ISBUY": "4",
                "BUY": false,
                "MINSG": "10",
                "MAXSG": "1"
            }),
            "501018",
        );
        assert_eq!(
            suspended.check_amount(500.0),
            (SubscribeVerdict::Blocked, Some("暂停申购".to_string()))
        );

        let limited = parse_fund_trade_status(
            &serde_json::json!({
                "FCODE": "161725",
                "SGZT": "限大额",
                "ISBUY": "1",
                "BUY": true,
                "MAXSG": "500000"
            }),
            "161725",
        );
        assert_eq!(limited.check_amount(500.0).0, SubscribeVerdict::Open);
        assert_eq!(limited.check_amount(600000.0).0, SubscribeVerdict::Blocked);
        assert_eq!(limited.max_amount, Some(500000.0));

        let open = parse_fund_trade_status(
            &serde_json::json!({
                "FCODE": "001618",
                "SGZT": "开放申购",
                "ISBUY": "1",
                "BUY": true,
                "MINSG": "10",
                "MAXSG": "100000000000"
            }),
            "001618",
        );
        assert_eq!(open.check_amount(500.0).0, SubscribeVerdict::Open);
        assert_eq!(
            open.check_amount(5.0).0,
            SubscribeVerdict::Blocked,
            "低于最小申购金额也应阻断"
        );
        assert_eq!(open.max_amount, None, "无上限哨兵值应被过滤");
    }

    #[test]
    fn unknown_when_status_missing() {
        let status = parse_fund_trade_status(&serde_json::json!({}), "000000");
        assert_eq!(status.check_amount(100.0).0, SubscribeVerdict::Unknown);
    }

    #[test]
    fn parses_search_results() {
        let value: Value = serde_json::from_str(
            r#"{
                "QuotationCodeTable": {
                    "Data": [
                        {"Code":"600000","Name":"浦发银行","JYS":"SH","Classify":"AStock","SecurityTypeName":"沪A","QuoteID":"1.600000"},
                        {"Code":"161725","Name":"招商中证白酒","Classify":"OTCFUND","SecurityTypeName":"基金","QuoteID":"150.161725"},
                        {"Code":"518880","Name":"黄金ETF","Classify":"Fund","SecurityTypeName":"基金","QuoteID":"1.518880"},
                        {"Code":"00700","Name":"腾讯控股","Classify":"HK","SecurityTypeName":"港股","QuoteID":"116.00700"},
                        {"Code":"AU9999","Name":"黄金9999","Classify":"SGE","SecurityTypeName":"现货","QuoteID":"118.AU9999"},
                        {"Code":"000001","Name":"上证指数","Classify":"Index","SecurityTypeName":"指数","QuoteID":"1.000001"},
                        {"Code":"113050","Name":"南银转债","Classify":"ConvertibleBond","SecurityTypeName":"可转债","QuoteID":"1.113050"}
                    ]
                }
            }"#,
        )
        .unwrap();
        let items: Vec<SearchItem> = value
            .pointer("/QuotationCodeTable/Data")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .filter_map(parse_search_item)
            .collect();

        assert_eq!(items.len(), 5);
        assert_eq!(items[0].kind, "stock");
        assert_eq!(items[0].symbol.as_deref(), Some("600000"));
        assert_eq!(items[1].kind, "fund");
        assert_eq!(items[1].symbol, None);
        assert_eq!(items[1].secid, None, "场外基金不应使用 150.* 作为 secid");
        assert_eq!(items[2].kind, "fund");
        assert_eq!(
            items[2].secid.as_deref(),
            Some("1.518880"),
            "场内基金保留 secid"
        );
        assert_eq!(items[3].kind, "stock");
        assert_eq!(items[3].symbol.as_deref(), Some("00700.HK"));
        assert_eq!(items[4].kind, "gold");
        assert_eq!(items[4].secid.as_deref(), Some("118.AU9999"));
    }

    #[test]
    fn parses_tencent_quotes() {
        let body = r#"v_sh601288="1~农业银行~601288~7.26~7.25~7.22~3194410";"#;
        let quote = parse_tencent_quote("sh601288", body).unwrap();
        assert!((quote.price - 7.26).abs() < 1e-9);
        assert_eq!(quote.prev_close, Some(7.25));
        assert_eq!(quote.name.as_deref(), Some("农业银行"));
        assert_eq!(quote.currency, "CNY");

        let body = r#"v_hk00700="100~腾讯控股~00700~398.200~409.600~404.000~18617972.0";"#;
        let quote = parse_tencent_quote("hk00700", body).unwrap();
        assert_eq!(quote.currency, "HKD");
        assert!((quote.change_percent().unwrap() + 2.78).abs() < 0.01);

        let body = r#"v_usAAPL="200~苹果~AAPL.OQ~355.63~348.51~349.55~34453477";"#;
        let quote = parse_tencent_quote("usAAPL", body).unwrap();
        assert_eq!(quote.currency, "USD");
    }

    #[test]
    fn rejects_missing_tencent_symbol() {
        assert!(parse_tencent_quote("sh999999", r#"v_pv_none_match="1";"#).is_err());
    }

    #[test]
    fn parses_sina_a_share_and_gold() {
        let body = r#"var hq_str_sh601288="农业银行,7.220,7.250,7.260,7.270,7.170,7.250,7.260,319440992,2310105525.000";"#;
        let quote = parse_sina_quote("sh601288", body).unwrap();
        assert!((quote.price - 7.26).abs() < 1e-9);
        assert_eq!(quote.prev_close, Some(7.25));
        assert_eq!(quote.name.as_deref(), Some("农业银行"));

        let body = r#"var hq_str_gds_AU9999="851.33,0,851.00,851.33,871.40,850.02,15:29:50,872.98,869.00,525218,1.00,669.00,2026-11-12,沪金99";"#;
        let quote = parse_sina_quote("gds_AU9999", body).unwrap();
        assert!((quote.price - 851.33).abs() < 1e-9);
        assert_eq!(quote.prev_close, Some(872.98));
        assert_eq!(quote.name.as_deref(), Some("沪金99"));
        assert_eq!(quote.currency, "CNY");
    }

    #[test]
    fn parses_sina_fund_and_us() {
        let body =
            r#"var hq_str_f_161725="招商中证白酒指数A,0.5663,2.2824,0.585,2026-11-11,394.159";"#;
        let quote = parse_sina_quote("f_161725", body).unwrap();
        assert!((quote.price - 0.5663).abs() < 1e-9);
        assert_eq!(quote.prev_close, Some(0.585));
        assert!(quote.note.contains("2026-11-11"));

        let body = r#"var hq_str_gb_aapl="苹果,355.6300,2.04,2026-11-12 09:16:15,7.1200,349.5450,355.9100,349.3250";"#;
        let quote = parse_sina_quote("gb_aapl", body).unwrap();
        assert_eq!(quote.currency, "USD");
        assert!((quote.prev_close.unwrap() - 348.51).abs() < 1e-6);
    }

    #[test]
    fn maps_fallback_symbols() {
        let symbols = |secid: &str| {
            fallback_symbols(secid)
                .into_iter()
                .map(|(_, symbol)| symbol)
                .collect::<Vec<_>>()
        };
        assert_eq!(symbols("1.601288"), vec!["sh601288", "sh601288"]);
        assert_eq!(symbols("0.000001"), vec!["sz000001", "sz000001"]);
        assert_eq!(symbols("0.920002"), vec!["bj920002", "bj920002"]);
        assert_eq!(symbols("116.00700"), vec!["hk00700"]);
        assert_eq!(symbols("105.AAPL"), vec!["usAAPL", "gb_aapl"]);
        assert_eq!(symbols("118.AU9999"), vec!["gds_AU9999"]);
        assert_eq!(symbols("113.aum"), vec!["nf_AU0"]);
        assert!(symbols("114.AU9999").is_empty());
    }

    #[test]
    fn parses_sina_futures_quote() {
        let body = r#"var hq_str_nf_AU0="黄金连续,150000,869.400,870.120,851.480,852.500,852.360,852.580,852.500,860.460,870.500,1,2,129365.000,209762,沪,黄金,2026-11-12";"#;
        let quote = parse_sina_quote("nf_AU0", body).unwrap();
        assert!((quote.price - 852.5).abs() < 1e-9);
        assert_eq!(quote.prev_close, Some(870.5));
        assert_eq!(quote.name.as_deref(), Some("黄金连续"));
    }
}
