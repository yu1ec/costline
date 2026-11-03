use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use chrono::Local;
use costline_core::{config, monitor, monitor::Row};
use reqwest::Client;

pub async fn run_once(
    config: &config::Config,
    client: &Client,
    state_path: &Path,
    notify_enabled: bool,
    force_notify: bool,
    json_output: bool,
) -> Result<()> {
    if config.assets.is_empty() {
        if json_output {
            println!("[]");
        } else {
            eprintln!(
                "[costline] 配置中没有资产，请编辑配置文件后重试（costline init 可生成示例）"
            );
        }
        return Ok(());
    }

    let report = monitor::check(config, client, state_path, notify_enabled, force_notify).await;

    if json_output {
        println!("{}", serde_json::to_string_pretty(&report.rows)?);
    } else {
        print_table(&report.rows);
    }
    let _ = std::io::stdout().flush();

    for message in report.skipped.iter().chain(report.errors.iter()) {
        eprintln!("[costline] {message}");
    }

    if !report.triggered.is_empty() {
        if let Some(reason) = &report.notify_skipped {
            eprintln!(
                "[costline] 已触发 {} 项，但{}：{}",
                report.triggered.len(),
                reason,
                report
                    .triggered
                    .iter()
                    .map(|row| row.name.as_str())
                    .collect::<Vec<_>>()
                    .join("、")
            );
        }
        for result in &report.notify_results {
            match &result.error {
                None => eprintln!("[costline] 通知已发送：{}", result.label),
                Some(error) => {
                    eprintln!("[costline] 通知发送失败（{}）：{error}", result.label)
                }
            }
        }
        if !report.notify_results.is_empty() && report.notify_success_count() == 0 {
            eprintln!("[costline] 所有通知渠道均失败，下一轮会重试");
        }
    }

    if !report.sip_due.is_empty() {
        let items = report
            .sip_due
            .iter()
            .map(|item| {
                format!(
                    "{} {}{:.2}（{}）",
                    item.name,
                    monitor::currency_symbol(&item.currency),
                    item.amount,
                    item.schedule
                )
            })
            .collect::<Vec<_>>()
            .join("、");
        eprintln!("[costline] 今日定投提醒：{items}");
        if report.sip_notify_success_count() == 0 {
            if let Some(reason) = &report.sip_notify_skipped {
                eprintln!("[costline] 定投提醒未发送：{reason}");
            } else {
                eprintln!("[costline] 定投提醒发送失败，下一轮会重试");
            }
        }
    }

    if let Some(error) = report.fatal_error() {
        if !report.notify_results.is_empty() && report.notify_success_count() == 0
            || report.total > 0 && report.failures == report.total
        {
            bail!("{error}");
        }
    }
    Ok(())
}

pub async fn watch(config_path: &Path, state_path: &Path, notify_enabled: bool) -> Result<()> {
    let mut config = config::load(config_path)?;
    let mut client = monitor::build_client(&config)?;
    let mut timeout_secs = config.timeout_secs;

    println!(
        "[costline] 开始盯盘，每 {} 秒检查一次，按 Ctrl+C 退出",
        config.interval_secs
    );

    loop {
        match config::load(config_path) {
            Ok(new_config) => {
                if new_config.timeout_secs != timeout_secs {
                    client = monitor::build_client(&new_config)?;
                    timeout_secs = new_config.timeout_secs;
                }
                config = new_config;
            }
            Err(error) => {
                eprintln!("[costline] 重新加载配置失败，继续使用旧配置：{error:#}");
            }
        }

        let started = Instant::now();
        println!(
            "\n[costline] {} 检查行情",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );
        if let Err(error) =
            run_once(&config, &client, state_path, notify_enabled, false, false).await
        {
            eprintln!("[costline] 本轮检查失败：{error:#}");
        }

        let elapsed = started.elapsed().as_secs();
        let sleep_secs = config.interval_secs.saturating_sub(elapsed).max(1);
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                if let Err(error) = result {
                    eprintln!("[costline] 监听 Ctrl+C 失败：{error}");
                }
                println!("\n[costline] 已退出");
                break;
            }
            _ = tokio::time::sleep(Duration::from_secs(sleep_secs)) => {}
        }
    }

    Ok(())
}

fn print_table(rows: &[Row]) {
    if rows.is_empty() {
        println!("没有可展示的行情");
        return;
    }

    println!(
        "{} {} {} {} {} {} {} {} {} 备注",
        pad_right("名称", 16),
        pad_right("类型", 4),
        pad_right("标识", 12),
        pad_left("现价", 10),
        pad_left("成本", 10),
        pad_left("目标", 10),
        pad_left("日涨跌", 8),
        pad_left("浮动盈亏", 14),
        pad_right("信号", 8)
    );
    println!("{}", "-".repeat(117));

    for row in rows {
        let signal = if row.triggered {
            "🔔 提醒"
        } else if row.above {
            "✅ 回本"
        } else {
            "—"
        };
        let pnl = row
            .pnl
            .map(|value| monitor::fmt_signed_money(value, row.currency))
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{} {} {} {} {} {} {} {} {} {}",
            pad_right(&truncate_display(&row.name, 16), 16),
            pad_right(row.kind, 4),
            pad_right(&truncate_display(&row.id, 12), 12),
            pad_left(&monitor::fmt_price(row.price), 10),
            pad_left(
                &row.cost
                    .map(monitor::fmt_price)
                    .unwrap_or_else(|| "-".to_string()),
                10
            ),
            pad_left(&monitor::fmt_price(row.target), 10),
            pad_left(&monitor::fmt_percent(row.change_percent), 8),
            pad_left(&pnl, 14),
            pad_right(signal, 8),
            truncate_display(&row.note, 16)
        );
    }
}

fn display_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

fn char_width(character: char) -> usize {
    match character as u32 {
        0x1100..=0x115F
        | 0x2E80..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE4F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1FAFF
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

fn pad_right(text: &str, width: usize) -> String {
    let current = display_width(text);
    if current >= width {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(width - current))
    }
}

fn pad_left(text: &str, width: usize) -> String {
    let current = display_width(text);
    if current >= width {
        text.to_string()
    } else {
        format!("{}{text}", " ".repeat(width - current))
    }
}

fn truncate_display(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_string();
    }
    let mut result = String::new();
    let mut current = 0;
    for character in text.chars() {
        let character_width = char_width(character);
        if current + character_width + 1 > width {
            break;
        }
        result.push(character);
        current += character_width;
    }
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aligns_cjk_text() {
        assert_eq!(display_width("浦发银行"), 8);
        assert_eq!(pad_right("浦发银行", 16), "浦发银行        ");
        assert_eq!(truncate_display("招商中证白酒指数", 8), "招商中…");
    }
}
