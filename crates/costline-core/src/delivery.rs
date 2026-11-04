use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDate;
use serde::Serialize;

use crate::config::{self, Config, Trade, TradeSide};
use crate::position::Position;
use crate::state::{ImportedFile, State};

/// 一次交割单导入的结果。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ImportReport {
    /// 成功解析的文件数
    pub files: usize,
    /// 新增成交笔数
    pub added: usize,
    /// 因重复而忽略的笔数
    pub duplicates: usize,
    /// 无法识别或校验失败的笔数
    pub skipped: usize,
    /// 交割单里存在、但配置中没有对应资产的代码：代码 -> 笔数
    pub unmatched: BTreeMap<String, usize>,
    /// 文件级错误（编码、表头、读取失败等）
    pub errors: Vec<String>,
    /// 行级提示（卖出超过持仓、日期异常等）
    pub warnings: Vec<String>,
}

impl ImportReport {
    pub fn is_empty(&self) -> bool {
        self.files == 0
            && self.added == 0
            && self.duplicates == 0
            && self.skipped == 0
            && self.unmatched.is_empty()
            && self.errors.is_empty()
            && self.warnings.is_empty()
    }

    /// 一行摘要，用于 CLI / 面板提示。
    pub fn summary(&self) -> String {
        let mut parts = vec![format!("新增 {} 笔", self.added)];
        if self.duplicates > 0 {
            parts.push(format!("重复 {} 笔", self.duplicates));
        }
        if self.skipped > 0 {
            parts.push(format!("跳过 {} 笔", self.skipped));
        }
        if !self.unmatched.is_empty() {
            let codes = self
                .unmatched
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join("、");
            parts.push(format!("未匹配资产 {codes}"));
        }
        parts.join("，")
    }

    fn merge(&mut self, stats: FileStats) {
        self.added += stats.added;
        self.duplicates += stats.duplicates;
        self.skipped += stats.skipped;
        for (code, count) in stats.unmatched {
            *self.unmatched.entry(code).or_insert(0) += count;
        }
        self.warnings.extend(stats.warnings);
    }
}

#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub config: Config,
    /// 是否有新的成交流水写入状态
    pub changed: bool,
    pub report: ImportReport,
}

#[derive(Debug, Default)]
struct FileStats {
    added: usize,
    duplicates: usize,
    skipped: usize,
    unmatched: BTreeMap<String, usize>,
    warnings: Vec<String>,
}

/// 扫描交割单并写入状态文件（不修改配置文件）。
///
/// `path_override` 可以是单个文件或目录；为空时使用 `config.import.dir`。
/// `force` 会忽略文件指纹缓存，但不会重复写入已经导入过的成交。
pub fn sync(
    config_path: &Path,
    state_path: &Path,
    path_override: Option<&Path>,
    force: bool,
) -> Result<SyncOutcome> {
    let config = config::load(config_path)?;
    let Some(path) = resolve_path(config_path, &config, path_override) else {
        return Ok(SyncOutcome {
            config,
            changed: false,
            report: ImportReport::default(),
        });
    };

    let mut report = ImportReport::default();
    if !path.exists() {
        report
            .errors
            .push(format!("交割单路径不存在：{}", path.display()));
        return Ok(SyncOutcome {
            config,
            changed: false,
            report,
        });
    }

    let files = collect_files(&path);
    if files.is_empty() {
        report
            .errors
            .push(format!("目录中没有 CSV / TXT 交割单：{}", path.display()));
        return Ok(SyncOutcome {
            config,
            changed: false,
            report,
        });
    }

    let mut state = State::load(state_path);
    let mut state_changed = false;
    let mut changed = false;

    for file in files {
        let fingerprint = fingerprint(&file);
        let cache_key = file.to_string_lossy().to_string();
        if !force {
            if let Some(cached) = state.imports.get(&cache_key) {
                if Some(cached) == fingerprint.as_ref() {
                    continue;
                }
            }
        }

        match import_file(&config, &mut state, &file) {
            Ok(stats) => {
                report.files += 1;
                if stats.added > 0 {
                    changed = true;
                }
                report.merge(stats);
                if let Some(fingerprint) = fingerprint {
                    if state.imports.insert(cache_key, fingerprint).is_none() {
                        state_changed = true;
                    }
                }
            }
            Err(error) => report.errors.push(format!("{}：{error:#}", file.display())),
        }
    }

    if changed || state_changed {
        state.save(state_path).context("保存导入的成交流水失败")?;
    }

    Ok(SyncOutcome {
        config,
        changed,
        report,
    })
}

fn resolve_path(
    config_path: &Path,
    config: &Config,
    override_path: Option<&Path>,
) -> Option<PathBuf> {
    let raw = match override_path {
        Some(path) => path.to_path_buf(),
        None => PathBuf::from(config.import.dir()?),
    };
    let expanded = expand_tilde(&raw);
    if expanded.is_absolute() {
        Some(expanded)
    } else {
        Some(
            config_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(expanded),
        )
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    if text == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    path.to_path_buf()
}

fn collect_files(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return if is_statement_file(path) {
            vec![path.to_path_buf()]
        } else {
            Vec::new()
        };
    }
    let mut files = Vec::new();
    walk(path, 0, &mut files);
    files.sort();
    files
}

fn walk(dir: &Path, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            walk(&path, depth + 1, files);
        } else if file_type.is_file() && is_statement_file(&path) {
            files.push(path);
        }
    }
}

fn is_statement_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "csv" | "txt" | "tsv"
            )
        })
        .unwrap_or(false)
}

fn fingerprint(path: &Path) -> Option<ImportedFile> {
    let metadata = fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    Some(ImportedFile {
        len: metadata.len(),
        modified,
    })
}

fn import_file(config: &Config, state: &mut State, path: &Path) -> Result<FileStats> {
    let bytes = fs::read(path).with_context(|| format!("读取交割单失败：{}", path.display()))?;
    let text = decode(&bytes);
    if text.trim().is_empty() {
        return Ok(FileStats::default());
    }

    let delimiter = detect_delimiter(&text);
    let rows = parse_rows(&text, delimiter);
    let (header_index, columns) =
        find_header(&rows).ok_or_else(|| anyhow::anyhow!("未识别到交割单表头"))?;

    let source = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut stats = FileStats::default();

    for row in rows.iter().skip(header_index + 1) {
        let parsed = match parse_trade(row, &columns, &source) {
            Ok(Some(trade)) => trade,
            Ok(None) => {
                stats.skipped += 1;
                continue;
            }
            Err(message) => {
                stats.skipped += 1;
                stats.warnings.push(format!("{source}：{message}"));
                continue;
            }
        };

        let Some(asset_index) = find_asset(config, &parsed.code) else {
            *stats.unmatched.entry(parsed.code).or_insert(0) += 1;
            continue;
        };
        let asset = &config.assets[asset_index];
        let asset_key = asset.key();
        let trade_key = parsed.trade.key();

        if asset.trades.iter().any(|trade| trade.key() == trade_key)
            || state.has_trade(&asset_key, &trade_key)
        {
            stats.duplicates += 1;
            continue;
        }

        state.add_trade(&asset_key, parsed.trade);
        match Position::derive(asset, state.trades_for(&asset_key)) {
            Ok(_) => stats.added += 1,
            Err(error) => {
                if let Some(trades) = state.trades.get_mut(&asset_key) {
                    trades.pop();
                }
                stats.skipped += 1;
                stats.warnings.push(format!(
                    "{} {} 未导入：{error}",
                    parsed.code,
                    parsed.date.as_deref().unwrap_or("-")
                ));
            }
        }
    }

    Ok(stats)
}

/// 兼容 UTF-8（含 BOM）与券商常见的 GBK / GB18030 编码。
fn decode(bytes: &[u8]) -> String {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => {
            let (text, _, _) = encoding_rs::GB18030.decode(bytes);
            text.into_owned()
        }
    };
    text.trim_start_matches('\u{feff}').to_string()
}

fn detect_delimiter(text: &str) -> char {
    let line = text
        .lines()
        .take(30)
        .max_by_key(|line| line.matches([',', '\t', ';', '|']).count())
        .unwrap_or("");
    [',', '\t', ';', '|']
        .into_iter()
        .max_by_key(|delimiter| line.matches(*delimiter).count())
        .filter(|delimiter| line.matches(*delimiter).count() > 0)
        .unwrap_or(',')
}

/// 极简 CSV 解析：支持引号、转义引号、CRLF 和字段内换行。
fn parse_rows(text: &str, delimiter: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();

    while let Some(character) = chars.next() {
        if quoted {
            if character == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(character);
            }
            continue;
        }
        match character {
            '"' if field.trim().is_empty() => {
                field.clear();
                quoted = true;
            }
            c if c == delimiter => {
                row.push(field.trim().to_string());
                field.clear();
            }
            '\r' => {}
            '\n' => {
                row.push(field.trim().to_string());
                field.clear();
                push_row(&mut rows, &mut row);
            }
            c => field.push(c),
        }
    }

    row.push(field.trim().to_string());
    push_row(&mut rows, &mut row);
    rows
}

fn push_row(rows: &mut Vec<Vec<String>>, row: &mut Vec<String>) {
    if row.iter().any(|field| !field.is_empty()) {
        rows.push(std::mem::take(row));
    } else {
        row.clear();
    }
}

struct Columns {
    date: usize,
    code: usize,
    side: usize,
    quantity: Option<usize>,
    amount: Option<usize>,
    price: Option<usize>,
    fee: Option<usize>,
    fee_parts: Vec<usize>,
    id: Option<usize>,
}

const DATE_ALIASES: &[&str] = &[
    "成交日期",
    "交易日期",
    "发生日期",
    "清算日期",
    "成交时间",
    "委托日期",
    "日期",
];
const CODE_ALIASES: &[&str] = &["证券代码", "股票代码", "基金代码", "产品代码", "代码"];
const SIDE_ALIASES: &[&str] = &[
    "买卖标志",
    "买卖方向",
    "交易方向",
    "业务名称",
    "业务类型",
    "成交类型",
    "操作",
    "摘要",
];
const QUANTITY_ALIASES: &[&str] = &[
    "成交数量",
    "成交股数",
    "成交份额",
    "发生数量",
    "数量",
    "份额",
    "股数",
];
const AMOUNT_ALIASES: &[&str] = &["成交金额", "发生金额", "成交额", "金额"];
const PRICE_ALIASES: &[&str] = &["成交价格", "成交均价", "成交价", "成交净值", "价格", "净值"];
const FEE_ALIASES: &[&str] = &["总费用", "交易费用", "手续费", "费用", "佣金"];
const FEE_PART_ALIASES: &[&str] = &["佣金", "印花税", "过户费", "规费", "经手费", "证管费"];
const ID_ALIASES: &[&str] = &[
    "成交编号",
    "合同编号",
    "成交序号",
    "流水号",
    "委托编号",
    "序号",
];

fn find_header(rows: &[Vec<String>]) -> Option<(usize, Columns)> {
    for (index, row) in rows.iter().enumerate().take(30) {
        if let Some(columns) = Columns::from_header(row) {
            return Some((index, columns));
        }
    }
    None
}

impl Columns {
    fn from_header(row: &[String]) -> Option<Self> {
        let headers: Vec<String> = row.iter().map(|header| normalize_header(header)).collect();
        let date = find_column(&headers, DATE_ALIASES)?;
        let code = find_column(&headers, CODE_ALIASES)?;
        let side = find_column(&headers, SIDE_ALIASES)?;
        let quantity = find_column(&headers, QUANTITY_ALIASES);
        let amount = find_column(&headers, AMOUNT_ALIASES);
        if quantity.is_none() && amount.is_none() {
            return None;
        }
        let price = find_column(&headers, PRICE_ALIASES);
        if price.is_none() && (quantity.is_none() || amount.is_none()) {
            return None;
        }
        let fee = find_column(&headers, FEE_ALIASES);
        let fee_parts = FEE_PART_ALIASES
            .iter()
            .filter_map(|alias| find_column(&headers, &[*alias]))
            .collect();
        Some(Self {
            date,
            code,
            side,
            quantity,
            amount,
            price,
            fee,
            fee_parts,
            id: find_column(&headers, ID_ALIASES),
        })
    }
}

fn normalize_header(header: &str) -> String {
    header
        .trim()
        .trim_start_matches('\u{feff}')
        .chars()
        .filter(|character| !character.is_whitespace() && !matches!(character, '*' | '"' | '\''))
        .collect::<String>()
        .to_lowercase()
}

fn find_column(headers: &[String], aliases: &[&str]) -> Option<usize> {
    for alias in aliases {
        if let Some(index) = headers.iter().position(|header| header == alias) {
            return Some(index);
        }
    }
    for alias in aliases {
        if let Some(index) = headers.iter().position(|header| header.contains(alias)) {
            return Some(index);
        }
    }
    None
}

struct ParsedTrade {
    code: String,
    date: Option<String>,
    trade: Trade,
}

fn parse_trade(
    row: &[String],
    columns: &Columns,
    source: &str,
) -> Result<Option<ParsedTrade>, String> {
    let code = cell(row, columns.code).unwrap_or_default();
    if code.is_empty() {
        return Ok(None);
    }
    let side_text = cell(row, columns.side).unwrap_or_default();
    let Some(side) = parse_side(&side_text) else {
        return Ok(None);
    };

    let date = cell(row, columns.date)
        .and_then(|value| normalize_date(&value))
        .ok_or_else(|| format!("{code} 的成交日期无法识别"))?;

    let mut quantity = columns
        .quantity
        .and_then(|index| cell(row, index))
        .and_then(|value| parse_number(&value));
    let amount = columns
        .amount
        .and_then(|index| cell(row, index))
        .and_then(|value| parse_number(&value));
    let mut price = columns
        .price
        .and_then(|index| cell(row, index))
        .and_then(|value| parse_number(&value));

    if quantity.is_none() {
        if let (Some(amount), Some(price)) = (amount, price) {
            if amount > 0.0 && price > 0.0 {
                quantity = Some(amount / price);
            }
        }
    }
    if price.is_none() {
        if let (Some(amount), Some(quantity)) = (amount, price_quantity(quantity)) {
            if amount > 0.0 && quantity > 0.0 {
                price = Some(amount / quantity);
            }
        }
    }
    let Some(quantity) = quantity.filter(|value| *value > 0.0) else {
        return Ok(None);
    };
    let Some(price) = price.filter(|value| *value > 0.0) else {
        return Ok(None);
    };

    let fee = fee_of(row, columns);
    let id = columns
        .id
        .and_then(|index| cell(row, index))
        .filter(|value| !value.is_empty());

    Ok(Some(ParsedTrade {
        code,
        date: Some(date.clone()),
        trade: Trade {
            id,
            side,
            quantity,
            price,
            fee,
            date: Some(date),
            note: Some(source.to_string()),
        },
    }))
}

fn price_quantity(quantity: Option<f64>) -> Option<f64> {
    quantity.filter(|value| *value > 0.0)
}

fn fee_of(row: &[String], columns: &Columns) -> f64 {
    if let Some(total) = columns
        .fee
        .and_then(|index| cell(row, index))
        .and_then(|value| parse_number(&value))
    {
        return total.max(0.0);
    }
    columns
        .fee_parts
        .iter()
        .filter_map(|index| cell(row, *index))
        .filter_map(|value| parse_number(&value))
        .filter(|value| *value > 0.0)
        .sum()
}

fn cell(row: &[String], index: usize) -> Option<String> {
    row.get(index)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && value != "--" && value != "—")
}

fn parse_side(text: &str) -> Option<TradeSide> {
    let text = text.trim();
    if text.is_empty() || text.contains('撤') {
        return None;
    }
    if text.contains('买') || text.contains("申购") || text.contains("认购") {
        return Some(TradeSide::Buy);
    }
    if text.contains('卖') || text.contains("赎回") {
        return Some(TradeSide::Sell);
    }
    None
}

fn parse_number(text: &str) -> Option<f64> {
    let cleaned: String = text
        .trim()
        .chars()
        .filter(|character| {
            !matches!(
                character,
                ',' | '，' | '¥' | '￥' | '$' | ' ' | '\u{feff}' | '\''
            )
        })
        .collect();
    cleaned.parse::<f64>().ok()
}

fn normalize_date(text: &str) -> Option<String> {
    let parts: Vec<&str> = text
        .trim()
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .collect();
    let (year, month, day) = match parts.as_slice() {
        // 2026-11-05 / 2026/11/5 / 2026-11-05 09:30:00
        [year, month, day, ..] if year.len() == 4 => (
            year.parse::<i32>().ok()?,
            month.parse::<u32>().ok()?,
            day.parse::<u32>().ok()?,
        ),
        // 20261105 / 20261105 09:30:00
        [compact, ..] if compact.len() >= 8 => (
            compact[0..4].parse::<i32>().ok()?,
            compact[4..6].parse::<u32>().ok()?,
            compact[6..8].parse::<u32>().ok()?,
        ),
        _ => return None,
    };
    NaiveDate::from_ymd_opt(year, month, day)?;
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

fn find_asset(config: &Config, raw_code: &str) -> Option<usize> {
    let target = normalize_code(raw_code);
    if target.is_empty() {
        return None;
    }
    config.assets.iter().position(|asset| {
        [&asset.symbol, &asset.secid, &asset.code]
            .into_iter()
            .flatten()
            .any(|code| normalize_code(code) == target)
    })
}

/// 把 sh600000 / 600000.SH / 1.600000 / 00700.HK 等写法归一为可比较的代码。
fn normalize_code(raw: &str) -> String {
    let mut text: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .filter(|character| !matches!(character, ' ' | '\'' | '"'))
        .collect();
    if text.is_empty() {
        return text;
    }

    for prefix in ["sh", "sz", "bj", "hk"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            if !rest.is_empty()
                && rest
                    .chars()
                    .all(|character| character.is_ascii_digit() || character == '.')
            {
                text = rest.to_string();
                break;
            }
        }
    }

    if let Some((left, right)) = text.split_once('.') {
        if matches!(right, "sh" | "sz" | "bj" | "hk" | "us") {
            text = left.to_string();
        } else if !left.is_empty()
            && left.chars().all(|character| character.is_ascii_digit())
            && !right.is_empty()
        {
            // secid 写法：1.600000 / 116.00700
            text = right.to_string();
        }
    }

    if !text.is_empty() && text.chars().all(|character| character.is_ascii_digit()) {
        let trimmed = text.trim_start_matches('0');
        return if trimmed.is_empty() {
            "0".to_string()
        } else {
            trimmed.to_string()
        };
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Asset, AssetKind, Config};

    fn config_with_asset() -> Config {
        let mut config = Config::default();
        config.assets.push(Asset {
            kind: AssetKind::Stock,
            name: Some("浦发银行".into()),
            symbol: Some("sh600000".into()),
            secid: None,
            code: None,
            cost: None,
            quantity: Some(1000.0),
            principal: Some(10000.0),
            as_of: Some("2026-10-01".into()),
            trades: Vec::new(),
            threshold_percent: None,
            target_price: None,
            sip: None,
        });
        config
    }

    #[test]
    fn parses_common_broker_csv() {
        let text =
            "成交日期,证券代码,证券名称,买卖标志,成交数量,成交价格,成交金额,手续费,成交编号\n\
                    2026-11-05,600000,浦发银行,卖出,500,12.00,6000.00,3.00,S001\n";
        let rows = parse_rows(text, ',');
        let (header, columns) = find_header(&rows).expect("header");
        assert_eq!(header, 0);
        let parsed = parse_trade(&rows[1], &columns, "test.csv")
            .unwrap()
            .expect("row");
        assert_eq!(parsed.code, "600000");
        assert_eq!(parsed.trade.side, TradeSide::Sell);
        assert_eq!(parsed.trade.quantity, 500.0);
        assert_eq!(parsed.trade.price, 12.0);
        assert_eq!(parsed.trade.fee, 3.0);
        assert_eq!(parsed.trade.date.as_deref(), Some("2026-11-05"));
        assert_eq!(parsed.trade.id.as_deref(), Some("S001"));
    }

    #[test]
    fn imports_and_deduplicates() {
        let temp = std::env::temp_dir().join(format!(
            "costline-import-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));
        fs::create_dir_all(&temp).unwrap();
        let file = temp.join("交割单.csv");
        fs::write(
            &file,
            "成交日期,证券代码,买卖标志,成交数量,成交价格,手续费,成交编号\n\
             2026-11-05,600000,卖出,500,12.00,3.00,S001\n",
        )
        .unwrap();

        let mut config = config_with_asset();
        let mut state = State::default();
        let stats = import_file(&config, &mut state, &file).unwrap();
        assert_eq!(stats.added, 1);
        assert_eq!(state.trades_for("stock:sh600000").len(), 1);
        let position =
            Position::derive(&config.assets[0], state.trades_for("stock:sh600000")).unwrap();
        assert_eq!(position.principal, 4003.0);

        // 同一笔再次导入应被去重。
        let stats = import_file(&config, &mut state, &file).unwrap();
        assert_eq!(stats.duplicates, 1);
        assert_eq!(state.trades_for("stock:sh600000").len(), 1);

        // as_of 之前的成交不参与计算。
        config.assets[0].as_of = Some("2026-12-01".into());
        let position = config.assets[0].position_with(state.trades_for("stock:sh600000"));
        assert_eq!(position.principal, 10000.0);
        assert_eq!(position.quantity, 1000.0);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn rejects_oversell_import() {
        let mut config = config_with_asset();
        config.assets[0].quantity = Some(100.0);
        config.assets[0].principal = Some(1000.0);
        let mut state = State::default();
        let trade = Trade {
            id: Some("S002".into()),
            side: TradeSide::Sell,
            quantity: 500.0,
            price: 12.0,
            fee: 0.0,
            date: Some("2026-11-05".into()),
            note: None,
        };
        state.add_trade("stock:sh600000", trade);
        assert!(Position::derive(&config.assets[0], state.trades_for("stock:sh600000")).is_err());
    }

    #[test]
    fn normalizes_codes() {
        assert_eq!(normalize_code("sh600000"), "600000");
        assert_eq!(normalize_code("600000.SH"), "600000");
        assert_eq!(normalize_code("1.600000"), "600000");
        assert_eq!(normalize_code("00700.HK"), "700");
        assert_eq!(normalize_code("000001"), "1");
        assert_eq!(normalize_code("AAPL"), "aapl");
    }

    #[test]
    fn normalizes_dates() {
        assert_eq!(normalize_date("2026-11-05"), Some("2026-11-05".into()));
        assert_eq!(normalize_date("2026/11/5"), Some("2026-11-05".into()));
        assert_eq!(normalize_date("20261105"), Some("2026-11-05".into()));
        assert_eq!(
            normalize_date("2026-11-05 09:30:00"),
            Some("2026-11-05".into())
        );
        assert_eq!(normalize_date("2026-02-30"), None);
    }

    #[test]
    fn parses_quoted_csv() {
        let rows = parse_rows(
            "\"成交日期\",\"证券代码\"\n\"2026-11-05\",\"600000\"\n",
            ',',
        );
        assert_eq!(rows[1][1], "600000");
    }
}
