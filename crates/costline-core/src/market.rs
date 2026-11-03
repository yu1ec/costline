use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde::Serialize;
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
    pub fn change_percent(&self) -> Option<f64> {
        self.prev_close
            .filter(|prev| *prev > 0.0)
            .map(|prev| (self.price / prev - 1.0) * 100.0)
    }
}

pub async fn fetch_quote(client: &Client, asset: &Asset) -> Result<Quote> {
    if let Some(secid) = &asset.secid {
        return fetch_eastmoney_candidates(client, std::slice::from_ref(secid)).await;
    }

    match asset.kind {
        AssetKind::Stock => {
            let raw = asset
                .symbol
                .as_deref()
                .ok_or_else(|| anyhow!("股票缺少 symbol"))?;
            let secids = stock_secids(raw)?;
            fetch_eastmoney_candidates(client, &secids).await
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
        return fetch_eastmoney_candidates(client, &[format!("{market}.{raw}")]).await;
    }

    match fetch_fundgz(client, raw).await {
        Ok(quote) => Ok(quote),
        Err(primary_error) => match fetch_danjuan(client, raw).await {
            Ok(quote) => Ok(quote),
            Err(fallback_error) => bail!(
                "基金 {raw} 获取失败。估值接口：{primary_error:#}；备用接口：{fallback_error:#}"
            ),
        },
    }
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
            return fetch_sina_gold(client, &sina_symbol).await;
        }
    }

    let secids = gold_secids(asset.symbol.as_deref())?;
    fetch_eastmoney_candidates(client, &secids).await
}

async fn fetch_eastmoney_candidates(client: &Client, secids: &[String]) -> Result<Quote> {
    let mut last_error = None;
    for secid in secids {
        match fetch_eastmoney(client, secid).await {
            Ok(quote) => return Ok(quote),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("没有可用的 secid")))
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

    let (nav, date) = fetch_danjuan_history(client, code).await?;
    Ok(Quote {
        price: nav,
        prev_close: None,
        name,
        note: format!("最新净值 {date}"),
        currency: "CNY",
        source: "danjuan",
    })
}

async fn fetch_danjuan_history(client: &Client, code: &str) -> Result<(f64, String)> {
    let url = format!("https://danjuanfunds.com/djapi/fund/nav/history/{code}?page=1&size=1");
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
    let item = value
        .get("data")
        .and_then(|data| data.get("items"))
        .and_then(|items| items.get(0))
        .ok_or_else(|| anyhow!("蛋卷基金没有净值数据"))?;
    let nav = json_number(item.get("nav"))
        .filter(|nav| *nav > 0.0)
        .ok_or_else(|| anyhow!("蛋卷基金净值格式异常"))?;
    let date = item
        .get("date")
        .and_then(Value::as_str)
        .unwrap_or("未知日期")
        .to_string();
    Ok((nav, date))
}

async fn fetch_sina_gold(client: &Client, symbol: &str) -> Result<Quote> {
    let url = format!("https://hq.sinajs.cn/list={symbol}");
    let response = client
        .get(&url)
        .header("Referer", "https://finance.sina.com.cn/")
        .send()
        .await
        .context("请求新浪黄金行情失败")?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("新浪返回 HTTP {status}");
    }

    let start = body.find('"').ok_or_else(|| anyhow!("新浪返回格式异常"))?;
    let end = body.rfind('"').ok_or_else(|| anyhow!("新浪返回格式异常"))?;
    if end <= start {
        bail!("新浪返回格式异常");
    }
    let data = &body[start + 1..end];
    if data.trim().is_empty() {
        bail!("新浪没有返回 {symbol} 的报价");
    }

    let fields: Vec<&str> = data.split(',').collect();
    let (price_index, prev_index) = if symbol.starts_with("hf_") {
        (0, 7)
    } else {
        (8, 5)
    };
    let price = fields
        .get(price_index)
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|price| *price > 0.0)
        .ok_or_else(|| anyhow!("无法解析新浪 {symbol} 的报价"))?;
    let prev_close = fields
        .get(prev_index)
        .and_then(|value| value.trim().parse::<f64>().ok())
        .filter(|price| *price > 0.0);
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

    Ok(Quote {
        price,
        prev_close,
        name: None,
        note,
        currency,
        source: "sina",
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
                let market = if symbol.starts_with('6') || symbol.starts_with('9') {
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
}
