use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use reqwest::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::config::Channel;

pub struct Notice {
    pub title: String,
    pub body: String,
}

pub struct SendResult {
    pub label: &'static str,
    pub result: Result<()>,
}

pub async fn send_all(client: &Client, channels: &[Channel], notice: &Notice) -> Vec<SendResult> {
    let mut results = Vec::new();
    for channel in channels {
        let label = channel.label();
        let result = send_one(client, channel, notice).await;
        results.push(SendResult { label, result });
    }
    results
}

async fn send_one(client: &Client, channel: &Channel, notice: &Notice) -> Result<()> {
    match channel {
        Channel::Bark {
            key,
            server,
            group,
            sound,
            level,
        } => {
            send_bark(
                client,
                key,
                server.as_deref(),
                group.as_deref(),
                sound.as_deref(),
                level.as_deref(),
                notice,
            )
            .await
        }
        Channel::Serverchan {
            sendkey,
            base_url,
            version,
        } => send_serverchan(client, sendkey, base_url.as_deref(), *version, notice).await,
        Channel::Pushplus { token, topic } => {
            send_pushplus(client, token, topic.as_deref(), notice).await
        }
        Channel::Telegram { bot_token, chat_id } => {
            send_telegram(client, bot_token, chat_id, notice).await
        }
        Channel::Wecom { webhook } => send_wecom(client, webhook, notice).await,
        Channel::Dingtalk { webhook, secret } => {
            send_dingtalk(client, webhook, secret.as_deref(), notice).await
        }
        Channel::Feishu { webhook, secret } => {
            send_feishu(client, webhook, secret.as_deref(), notice).await
        }
        Channel::Ntfy {
            topic,
            server,
            token,
        } => send_ntfy(client, topic, server.as_deref(), token.as_deref(), notice).await,
        Channel::Gotify { server, token } => send_gotify(client, server, token, notice).await,
        Channel::Webhook { url, headers } => {
            send_webhook(client, url, headers.as_ref(), notice).await
        }
        Channel::Desktop { sound } => send_desktop(sound.as_deref(), notice).await,
    }
}

async fn send_bark(
    client: &Client,
    key: &str,
    server: Option<&str>,
    group: Option<&str>,
    sound: Option<&str>,
    level: Option<&str>,
    notice: &Notice,
) -> Result<()> {
    let server = server
        .unwrap_or("https://api.day.app")
        .trim_end_matches('/');
    let url = format!("{server}/{key}");
    let mut payload = serde_json::Map::new();
    payload.insert("title".into(), json!(notice.title));
    payload.insert("body".into(), json!(truncate_bytes(&notice.body, 3000)));
    if let Some(group) = group {
        payload.insert("group".into(), json!(group));
    }
    if let Some(sound) = sound {
        payload.insert("sound".into(), json!(sound));
    }
    if let Some(level) = level {
        payload.insert("level".into(), json!(level));
    }

    let response = client
        .post(&url)
        .json(&Value::Object(payload))
        .send()
        .await
        .context("请求 Bark 失败")?;
    let body = ensure_success(response, "Bark").await?;
    if let Some(value) = parse_json(&body) {
        if let Some(code) = value.get("code").and_then(Value::as_i64) {
            if code != 200 {
                bail!("Bark 返回错误：{}", truncate_chars(&body, 300));
            }
        }
    }
    Ok(())
}

async fn send_serverchan(
    client: &Client,
    sendkey: &str,
    base_url: Option<&str>,
    version: Option<u8>,
    notice: &Notice,
) -> Result<()> {
    let url = if let Some(base_url) = base_url {
        base_url.to_string()
    } else if version == Some(2) || (sendkey.starts_with("SCT") && !sendkey.starts_with("sctp")) {
        format!("https://sctapi.ftqq.com/{sendkey}.send")
    } else {
        format!("https://{sendkey}.push.ft07.com/send")
    };

    let body_text = truncate_bytes(&notice.body, 30000);
    let response = client
        .post(&url)
        .form(&[
            ("title", notice.title.as_str()),
            ("desp", body_text.as_str()),
        ])
        .send()
        .await
        .context("请求 Server酱失败")?;
    let body = ensure_success(response, "Server酱").await?;
    let value = parse_json(&body).ok_or_else(|| anyhow!("Server酱返回内容不是 JSON"))?;
    let ok = value.get("code").and_then(Value::as_i64) == Some(0)
        || value.get("errno").and_then(Value::as_i64) == Some(0);
    if !ok {
        bail!("Server酱返回错误：{}", truncate_chars(&body, 300));
    }
    Ok(())
}

async fn send_pushplus(
    client: &Client,
    token: &str,
    topic: Option<&str>,
    notice: &Notice,
) -> Result<()> {
    let mut payload = json!({
        "token": token,
        "title": notice.title,
        "content": truncate_bytes(&notice.body, 18000),
        "template": "markdown",
    });
    if let Some(topic) = topic {
        payload["topic"] = json!(topic);
    }

    let response = client
        .post("https://www.pushplus.plus/send")
        .json(&payload)
        .send()
        .await
        .context("请求 PushPlus 失败")?;
    let body = ensure_success(response, "PushPlus").await?;
    let value = parse_json(&body).ok_or_else(|| anyhow!("PushPlus 返回内容不是 JSON"))?;
    if value.get("code").and_then(Value::as_i64) != Some(200) {
        bail!("PushPlus 返回错误：{}", truncate_chars(&body, 300));
    }
    Ok(())
}

async fn send_telegram(
    client: &Client,
    bot_token: &str,
    chat_id: &str,
    notice: &Notice,
) -> Result<()> {
    let url = format!("https://api.telegram.org/bot{bot_token}/sendMessage");
    let text = truncate_chars(&format!("{}\n\n{}", notice.title, notice.body), 3800);
    let payload = json!({
        "chat_id": chat_id,
        "text": text,
        "disable_web_page_preview": true,
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .context("请求 Telegram 失败")?;
    let body = ensure_success(response, "Telegram").await?;
    let value = parse_json(&body).ok_or_else(|| anyhow!("Telegram 返回内容不是 JSON"))?;
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        bail!(
            "Telegram 返回错误：{}",
            value
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("未知错误")
        );
    }
    Ok(())
}

async fn send_wecom(client: &Client, webhook: &str, notice: &Notice) -> Result<()> {
    let content = truncate_bytes(&format!("**{}**\n{}", notice.title, notice.body), 3800);
    let payload = json!({
        "msgtype": "markdown",
        "markdown": { "content": content },
    });

    let response = client
        .post(webhook)
        .json(&payload)
        .send()
        .await
        .context("请求企业微信失败")?;
    let body = ensure_success(response, "企业微信").await?;
    let value = parse_json(&body).ok_or_else(|| anyhow!("企业微信返回内容不是 JSON"))?;
    if value.get("errcode").and_then(Value::as_i64) != Some(0) {
        bail!("企业微信返回错误：{}", truncate_chars(&body, 300));
    }
    Ok(())
}

async fn send_dingtalk(
    client: &Client,
    webhook: &str,
    secret: Option<&str>,
    notice: &Notice,
) -> Result<()> {
    let url = if let Some(secret) = secret {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let string_to_sign = format!("{timestamp}\n{secret}");
        let sign = percent_encode(&base64_hmac(secret.as_bytes(), string_to_sign.as_bytes()));
        if webhook.contains('?') {
            format!("{webhook}&timestamp={timestamp}&sign={sign}")
        } else {
            format!("{webhook}?timestamp={timestamp}&sign={sign}")
        }
    } else {
        webhook.to_string()
    };

    let text = truncate_bytes(&format!("### {}\n\n{}", notice.title, notice.body), 18000);
    let payload = json!({
        "msgtype": "markdown",
        "markdown": {
            "title": notice.title,
            "text": text,
        },
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .context("请求钉钉失败")?;
    let body = ensure_success(response, "钉钉").await?;
    let value = parse_json(&body).ok_or_else(|| anyhow!("钉钉返回内容不是 JSON"))?;
    if value.get("errcode").and_then(Value::as_i64) != Some(0) {
        bail!("钉钉返回错误：{}", truncate_chars(&body, 300));
    }
    Ok(())
}

async fn send_feishu(
    client: &Client,
    webhook: &str,
    secret: Option<&str>,
    notice: &Notice,
) -> Result<()> {
    let text = truncate_bytes(&format!("{}\n\n{}", notice.title, notice.body), 28000);
    let mut payload = json!({
        "msg_type": "text",
        "content": { "text": text },
    });

    if let Some(secret) = secret {
        let timestamp = chrono::Utc::now().timestamp();
        let string_to_sign = format!("{timestamp}\n{secret}");
        // 飞书签名：以 timestamp + "\n" + secret 为密钥，对空字符串做 HmacSHA256
        let sign = base64_hmac(string_to_sign.as_bytes(), b"");
        payload["timestamp"] = json!(timestamp.to_string());
        payload["sign"] = json!(sign);
    }

    let response = client
        .post(webhook)
        .json(&payload)
        .send()
        .await
        .context("请求飞书失败")?;
    let body = ensure_success(response, "飞书").await?;
    let value = parse_json(&body).ok_or_else(|| anyhow!("飞书返回内容不是 JSON"))?;
    if value.get("code").and_then(Value::as_i64) != Some(0) {
        bail!("飞书返回错误：{}", truncate_chars(&body, 300));
    }
    Ok(())
}

async fn send_ntfy(
    client: &Client,
    topic: &str,
    server: Option<&str>,
    token: Option<&str>,
    notice: &Notice,
) -> Result<()> {
    let server = server.unwrap_or("https://ntfy.sh").trim_end_matches('/');
    let topic = topic.trim_start_matches('/');
    let url = format!("{server}/{topic}");
    let mut request = client
        .post(&url)
        .header("X-Title", &notice.title)
        .header("X-Priority", "4")
        .body(truncate_bytes(
            &format!("{}\n\n{}", notice.title, notice.body),
            3800,
        ));
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }

    let response = request.send().await.context("请求 ntfy 失败")?;
    ensure_success(response, "ntfy").await?;
    Ok(())
}

async fn send_gotify(client: &Client, server: &str, token: &str, notice: &Notice) -> Result<()> {
    let server = server.trim_end_matches('/');
    let url = format!("{server}/message?token={token}");
    let payload = json!({
        "title": notice.title,
        "message": truncate_bytes(&notice.body, 28000),
        "priority": 5,
    });

    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .await
        .context("请求 Gotify 失败")?;
    ensure_success(response, "Gotify").await?;
    Ok(())
}

async fn send_webhook(
    client: &Client,
    url: &str,
    headers: Option<&std::collections::BTreeMap<String, String>>,
    notice: &Notice,
) -> Result<()> {
    let payload = json!({
        "title": notice.title,
        "content": notice.body,
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "source": "costline",
    });

    let mut request = client.post(url).json(&payload);
    if let Some(headers) = headers {
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
    }
    let response = request.send().await.context("请求 Webhook 失败")?;
    ensure_success(response, "Webhook").await?;
    Ok(())
}

#[cfg(target_os = "macos")]
async fn send_desktop(sound: Option<&str>, notice: &Notice) -> Result<()> {
    let body = truncate_chars(&notice.body.replace('\n', " "), 300);
    let sound_clause = sound
        .map(|sound| format!(" sound name \"{}\"", escape_applescript(sound)))
        .unwrap_or_default();
    let script = format!(
        "display notification \"{}\" with title \"{}\"{}",
        escape_applescript(&body),
        escape_applescript(&notice.title),
        sound_clause
    );
    let output = tokio::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .await
        .context("执行 osascript 失败")?;
    if !output.status.success() {
        bail!(
            "macOS 本地通知失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
async fn send_desktop(_sound: Option<&str>, notice: &Notice) -> Result<()> {
    let output = tokio::process::Command::new("notify-send")
        .arg(&notice.title)
        .arg(&notice.body)
        .output()
        .await
        .context("执行 notify-send 失败，请先安装 libnotify")?;
    if !output.status.success() {
        bail!(
            "Linux 本地通知失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
async fn send_desktop(_sound: Option<&str>, _notice: &Notice) -> Result<()> {
    bail!("当前系统不支持本地通知，请使用网络通知渠道")
}

async fn ensure_success(response: reqwest::Response, service: &str) -> Result<String> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!(
            "{service} 返回 HTTP {status}：{}",
            truncate_chars(&body, 300)
        );
    }
    Ok(body)
}

fn parse_json(body: &str) -> Option<Value> {
    serde_json::from_str(body).ok()
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max_chars).collect::<String>())
    }
}

/// 按 UTF-8 字节数截断，避免把中文字符截成半个
fn truncate_bytes(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = max_bytes.saturating_sub(3);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

#[cfg(target_os = "macos")]
fn escape_applescript(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn base64_hmac(key: &[u8], message: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(hmac_sha256(key, message))
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut key_block = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        let digest = Sha256::digest(key);
        key_block[..digest.len()].copy_from_slice(&digest);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0u8; BLOCK_SIZE];
    let mut outer_pad = [0u8; BLOCK_SIZE];
    for index in 0..BLOCK_SIZE {
        inner_pad[index] = key_block[index] ^ 0x36;
        outer_pad[index] = key_block[index] ^ 0x5c;
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner_hash);
    let result = outer.finalize();

    let mut output = [0u8; 32];
    output.copy_from_slice(&result);
    output
}

fn percent_encode(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(value.len() * 3);
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => {
                encoded.push('%');
                encoded.push(HEX[(byte >> 4) as usize] as char);
                encoded.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc4231_vector() {
        let digest = hmac_sha256(b"key", b"The quick brown fox jumps over the lazy dog");
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            hex,
            "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }

    #[test]
    fn percent_encoding_escapes_signature() {
        assert_eq!(percent_encode("abc+/="), "abc%2B%2F%3D");
        assert_eq!(percent_encode("a-b_c.d~"), "a-b_c.d~");
    }

    #[test]
    fn truncates_long_text() {
        assert_eq!(truncate_chars("12345", 10), "12345");
        assert_eq!(truncate_chars("123456", 5), "12345…");
        assert_eq!(truncate_bytes("中文中文", 100), "中文中文");
        assert_eq!(truncate_bytes("中文中文", 7), "中…");
    }
}
