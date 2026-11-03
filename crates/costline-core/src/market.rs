use anyhow::{anyhow, bail, Context, Result};
use reqwest::Client;
use serde_json::Value;

use crate::config::{Asset, AssetKind};

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
}
