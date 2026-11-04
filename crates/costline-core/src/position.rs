use std::collections::BTreeSet;

use anyhow::{bail, Result};
use serde::Serialize;

use crate::config::{Asset, Trade, TradeSide};

const EPSILON: f64 = 1e-9;

/// 由「期初持仓 + 成交流水」推导出的当前持仓。
///
/// 采用摊薄成本（方案 B）：卖出净额直接从剩余本金中扣除，
/// 因此卖出后成本价 = 剩余本金 / 剩余数量，可能下降甚至变为负数。
#[derive(Debug, Clone, Default, Serialize)]
pub struct Position {
    /// 当前持有数量：股票为股数，基金为份额，黄金为克数
    pub quantity: f64,
    /// 摊薄后的剩余本金。负数表示本金已全部收回，且已锁定等额盈利。
    pub principal: f64,
    /// principal / quantity；清仓后为 None
    pub cost: Option<f64>,
    /// 是否配置了数量或成交流水（没有持仓信息的资产不计算盈亏）
    pub has_position: bool,
    /// 本金已收回：仍有持仓，但剩余本金 <= 0
    pub recovered: bool,
    /// 已清仓
    pub closed: bool,
    /// 累计买入金额（含费用）
    pub invested: f64,
    /// 累计卖出净额（已扣费用）
    pub proceeds: f64,
}

impl Position {
    /// 严格推导；卖出数量超过持有数量时返回错误。
    ///
    /// `extra` 是来自交割单等外部来源的成交流水（例如状态文件中的导入记录）。
    pub fn derive(asset: &Asset, extra: &[Trade]) -> Result<Self> {
        let mut position = Self {
            quantity: asset.quantity.unwrap_or(0.0),
            principal: asset
                .principal
                .unwrap_or_else(|| asset.cost.unwrap_or(0.0) * asset.quantity.unwrap_or(0.0)),
            has_position: asset.quantity.is_some()
                || asset.principal.is_some()
                || !asset.trades.is_empty()
                || !extra.is_empty(),
            ..Self::default()
        };
        position.invested = position.principal;

        for trade in ordered_trades(asset, extra) {
            if !trade.quantity.is_finite() || trade.quantity <= 0.0 {
                bail!("成交数量必须大于 0");
            }
            if !trade.price.is_finite() || trade.price <= 0.0 {
                bail!("成交价格必须大于 0");
            }
            if !trade.fee.is_finite() || trade.fee < 0.0 {
                bail!("手续费不能为负数");
            }

            match trade.side {
                TradeSide::Buy => {
                    let amount = trade.quantity * trade.price + trade.fee;
                    position.quantity += trade.quantity;
                    position.principal += amount;
                    position.invested += amount;
                }
                TradeSide::Sell => {
                    if trade.quantity > position.quantity + EPSILON {
                        bail!(
                            "卖出 {} 超过持有数量 {}",
                            trim_number(trade.quantity),
                            trim_number(position.quantity)
                        );
                    }
                    let net = trade.quantity * trade.price - trade.fee;
                    position.quantity = (position.quantity - trade.quantity).max(0.0);
                    position.principal -= net;
                    position.proceeds += net;
                }
            }
        }

        position.finish();
        Ok(position)
    }

    /// 宽松推导：配置存在错误时退化为仅使用期初持仓，避免盯盘界面整体失败。
    pub fn derive_lossy(asset: &Asset, extra: &[Trade]) -> Self {
        Self::derive(asset, extra).unwrap_or_else(|_| {
            let quantity = asset.quantity.unwrap_or(0.0);
            let principal = asset
                .principal
                .unwrap_or_else(|| asset.cost.unwrap_or(0.0) * quantity);
            let mut position = Self {
                quantity,
                principal,
                has_position: asset.quantity.is_some()
                    || asset.principal.is_some()
                    || !asset.trades.is_empty()
                    || !extra.is_empty(),
                ..Self::default()
            };
            position.finish();
            position
        })
    }

    /// 按现价计算总盈亏：市值 - 剩余本金。
    ///
    /// 清仓后剩余本金为正表示已实现亏损，为负表示已实现盈利。
    pub fn market_pnl(&self, price: f64) -> Option<f64> {
        if !self.has_position {
            return None;
        }
        Some(price * self.quantity - self.principal)
    }

    fn finish(&mut self) {
        if self.principal.abs() < EPSILON {
            self.principal = 0.0;
        }
        self.closed = self.has_position && self.quantity <= EPSILON;
        self.recovered = self.has_position && !self.closed && self.principal <= 0.0;
        self.cost = if self.quantity > EPSILON {
            Some(self.principal / self.quantity)
        } else {
            None
        };
    }
}

/// 按日期排序；没有日期的记录（手动录入）视为最早，保持原有顺序。
fn ordered_trades<'a>(asset: &'a Asset, extra: &'a [Trade]) -> Vec<&'a Trade> {
    let as_of = asset.as_of.as_deref();
    // 同一笔成交既写在配置里、又被交割单导入时，只保留导入的一条。
    // 仅对有成交编号的记录去重，避免把同一天同价同量的两笔真实成交误判为重复。
    let imported: BTreeSet<String> = extra
        .iter()
        .filter(|trade| has_id(trade))
        .map(Trade::key)
        .collect();
    let mut trades: Vec<&Trade> = asset
        .trades
        .iter()
        .filter(|trade| !(has_id(trade) && imported.contains(&trade.key())))
        .chain(extra.iter())
        .filter(|trade| match (trade.date.as_deref(), as_of) {
            // 期初持仓已经包含 as_of 当天及以前的成交，不重复应用。
            (Some(date), Some(as_of)) => date > as_of,
            _ => true,
        })
        .collect();
    trades.sort_by(|left, right| {
        left.date
            .as_deref()
            .unwrap_or("")
            .cmp(right.date.as_deref().unwrap_or(""))
    });
    trades
}

fn has_id(trade: &Trade) -> bool {
    trade
        .id
        .as_deref()
        .map(str::trim)
        .is_some_and(|id| !id.is_empty())
}

fn trim_number(value: f64) -> String {
    let text = format!("{value:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Asset, AssetKind, Trade, TradeSide};

    fn asset(
        cost: Option<f64>,
        quantity: Option<f64>,
        principal: Option<f64>,
        trades: Vec<Trade>,
    ) -> Asset {
        Asset {
            kind: AssetKind::Stock,
            name: None,
            symbol: Some("sh600000".into()),
            secid: None,
            code: None,
            cost,
            quantity,
            principal,
            as_of: None,
            trades,
            threshold_percent: None,
            target_price: None,
            sip: None,
        }
    }

    fn trade(side: TradeSide, quantity: f64, price: f64) -> Trade {
        Trade {
            id: None,
            side,
            quantity,
            price,
            fee: 0.0,
            date: None,
            note: None,
        }
    }

    #[test]
    fn sell_reduces_principal_and_cost() {
        // 本金 10000，1000 股，成本 10；卖出 500 股得 6000。
        let asset = asset(
            None,
            Some(1000.0),
            Some(10000.0),
            vec![trade(TradeSide::Sell, 500.0, 12.0)],
        );
        let position = Position::derive(&asset, &[]).unwrap();
        assert_eq!(position.quantity, 500.0);
        assert_eq!(position.principal, 4000.0);
        assert_eq!(position.cost, Some(8.0));
        assert!(!position.recovered);
    }

    #[test]
    fn sell_fee_reduces_proceeds() {
        let mut sell = trade(TradeSide::Sell, 500.0, 12.0);
        sell.fee = 5.0;
        let asset = asset(None, Some(1000.0), Some(10000.0), vec![sell]);
        let position = Position::derive(&asset, &[]).unwrap();
        assert_eq!(position.principal, 4005.0);
    }

    #[test]
    fn fully_recovered_principal_is_muted_but_keeps_position() {
        let asset = asset(
            None,
            Some(1000.0),
            Some(10000.0),
            vec![trade(TradeSide::Sell, 500.0, 24.0)],
        );
        let position = Position::derive(&asset, &[]).unwrap();
        assert_eq!(position.quantity, 500.0);
        assert_eq!(position.principal, -2000.0);
        assert!(position.recovered);
        assert_eq!(position.cost, Some(-4.0));
    }

    #[test]
    fn buy_adds_principal_and_quantity() {
        let asset = asset(
            Some(10.0),
            Some(1000.0),
            None,
            vec![trade(TradeSide::Buy, 100.0, 20.0)],
        );
        let position = Position::derive(&asset, &[]).unwrap();
        assert_eq!(position.quantity, 1100.0);
        assert_eq!(position.principal, 12000.0);
        assert!((position.cost.unwrap() - 12000.0 / 1100.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_oversell() {
        let asset = asset(
            None,
            Some(100.0),
            Some(1000.0),
            vec![trade(TradeSide::Sell, 200.0, 12.0)],
        );
        assert!(Position::derive(&asset, &[]).is_err());
    }

    #[test]
    fn as_of_skips_already_counted_trades() {
        let mut old_sell = trade(TradeSide::Sell, 500.0, 12.0);
        old_sell.date = Some("2026-10-01".into());
        let mut new_sell = trade(TradeSide::Sell, 100.0, 12.0);
        new_sell.date = Some("2026-11-05".into());
        let mut asset = asset(None, Some(1000.0), Some(10000.0), vec![old_sell, new_sell]);
        asset.as_of = Some("2026-11-01".into());
        let position = Position::derive(&asset, &[]).unwrap();
        assert_eq!(position.quantity, 900.0);
        assert_eq!(position.principal, 8800.0);
    }

    #[test]
    fn market_pnl_uses_remaining_principal() {
        let asset = asset(
            None,
            Some(1000.0),
            Some(10000.0),
            vec![trade(TradeSide::Sell, 500.0, 24.0)],
        );
        let position = Position::derive(&asset, &[]).unwrap();
        // 剩余 500 股，现价 20 → 市值 10000，剩余本金 -2000。
        assert_eq!(position.market_pnl(20.0), Some(12000.0));
    }

    #[test]
    fn same_trade_in_config_and_import_is_counted_once() {
        let mut sell = trade(TradeSide::Sell, 500.0, 12.0);
        sell.id = Some("S001".into());
        sell.date = Some("2026-10-20".into());
        let asset = asset(None, Some(1000.0), Some(10000.0), vec![sell.clone()]);
        let position = Position::derive(&asset, &[sell]).unwrap();
        assert_eq!(position.quantity, 500.0);
        assert_eq!(position.principal, 4000.0);
        assert_eq!(position.cost, Some(8.0));
    }

    #[test]
    fn identical_trades_without_id_are_not_deduplicated() {
        // 没有成交编号时，两笔同价同量的成交可能都是真的，不能合并。
        let first = trade(TradeSide::Buy, 100.0, 10.0);
        let second = trade(TradeSide::Buy, 100.0, 10.0);
        let asset = asset(None, None, None, vec![first.clone()]);
        let position = Position::derive(&asset, &[second]).unwrap();
        assert_eq!(position.quantity, 200.0);
        assert_eq!(position.principal, 2000.0);
    }
}
