import {
  AlertCircle,
  Bell,
  CheckCircle2,
  LineChart,
  Plus,
  RefreshCw,
  Wallet,
} from "lucide-react";
import { useState } from "react";
import type { Config, Snapshot } from "../types";
import { emptyAssetDraft, type AssetDraft } from "../drafts";
import {
  currencySymbol,
  formatAmount,
  formatMoney,
  formatPercent,
  formatPrice,
  formatSignedPrice,
  formatSipDate,
  formatTime,
} from "../format";
import { EmptyState, Tag } from "../components/ui";
import { AssetModal } from "../components/AssetModal";

const REDACTED = "••••";

export function Dashboard({
  config,
  snapshot,
  busy,
  hideAmounts,
  onCheck,
  onToggleRealtime,
  onAddAsset,
  onOpenSettings,
}: {
  config: Config | null;
  snapshot: Snapshot | null;
  busy: boolean;
  hideAmounts: boolean;
  onCheck: () => void;
  onToggleRealtime: () => void;
  onAddAsset: (draft: AssetDraft) => Promise<boolean>;
  onOpenSettings: () => void;
}) {
  const [adding, setAdding] = useState(false);
  const rows = snapshot?.rows ?? [];
  const assetsCount = snapshot?.assets_count ?? config?.assets.length ?? 0;
  const recovered = rows.filter((row) => row.above).length;
  const triggered = rows.filter((row) => row.triggered).length;
  const totals = new Map<string, number>();
  for (const row of rows) {
    if (row.pnl != null) {
      totals.set(row.currency, (totals.get(row.currency) ?? 0) + row.pnl);
    }
  }

  return (
    <div className="page">
      <section className="summary-grid">
        <div className="summary-card">
          <div className="summary-icon">
            <Wallet size={18} />
          </div>
          <div>
            <span>持仓资产</span>
            <strong>{assetsCount}</strong>
          </div>
        </div>
        <div className="summary-card">
          <div className="summary-icon ok">
            <CheckCircle2 size={18} />
          </div>
          <div>
            <span>已回本</span>
            <strong>
              {recovered}
              <em> / {rows.length || assetsCount}</em>
            </strong>
          </div>
        </div>
        <div className={triggered > 0 ? "summary-card warn" : "summary-card"}>
          <div className="summary-icon warn">
            <Bell size={18} />
          </div>
          <div>
            <span>本轮触发</span>
            <strong>{triggered}</strong>
          </div>
        </div>
        <div className="summary-card wide-card">
          <div className="summary-icon">
            <LineChart size={18} />
          </div>
          <div>
            <span>浮动盈亏</span>
            {totals.size === 0 ? (
              <strong className="muted">—</strong>
            ) : hideAmounts ? (
              <strong className="muted">{REDACTED}</strong>
            ) : (
              <div className="pnl-list">
                {[...totals.entries()].map(([currency, value]) => (
                  <strong
                    key={currency}
                    className={value >= 0 ? "mono up" : "mono down"}
                  >
                    {formatMoney(value, currency)}
                  </strong>
                ))}
              </div>
            )}
          </div>
        </div>
      </section>

      {snapshot?.last_error ? (
        <div className="banner error">
          <AlertCircle size={18} />
          <div>
            <strong>部分数据获取失败</strong>
            <p>{snapshot.last_error}</p>
          </div>
        </div>
      ) : null}

      <section className="panel">
        <div className="panel-head">
          <div>
            <h2>行情</h2>
            <p>
              {snapshot?.watching ? "自动盯盘进行中" : "自动盯盘已暂停"}
              <span className="dot">·</span>
              {snapshot?.realtime ? "实时价格刷新中" : "实时价格已关闭"}
              <span className="dot">·</span>
              最近检查 {formatTime(snapshot?.last_check ?? null)}
              <span className="dot">·</span>
              每 {snapshot?.interval_secs ?? config?.interval_secs ?? 60} 秒刷新
            </p>
          </div>
          <div className="panel-actions">
            <label
              className="realtime-switch"
              title="开启后每 10 秒刷新一次基金、黄金和股票的行情"
            >
              <input
                type="checkbox"
                checked={Boolean(snapshot?.realtime)}
                disabled={busy}
                onChange={onToggleRealtime}
              />
              <span className="realtime-switch-track" aria-hidden />
              <span className="realtime-switch-text">
                <strong>实时价格</strong>
                <em>{snapshot?.realtime ? "每 10 秒刷新" : "已关闭"}</em>
              </span>
            </label>
            <button
              type="button"
              className="btn primary"
              onClick={() => setAdding(true)}
            >
              <Plus size={15} />
              添加持仓
            </button>
            <button type="button" className="btn" onClick={onCheck} disabled={busy}>
              <RefreshCw size={15} className={busy ? "spin" : ""} />
              {busy ? "检查中" : "立即检查"}
            </button>
          </div>
        </div>

        {assetsCount === 0 ? (
          <EmptyState
            icon={<Wallet size={22} />}
            title="还没有添加资产"
            description="添加你买入的股票、基金或黄金，填入成本价，价格越过成本线时会通知你。"
            action={
              <button type="button" className="btn primary" onClick={onOpenSettings}>
                去添加资产
              </button>
            }
          />
        ) : rows.length === 0 ? (
          <div className="loading-state">
            <RefreshCw size={18} className="spin" />
            <span>正在获取第一轮行情…</span>
          </div>
        ) : (
          <div className="quote-table">
            <div className="table-head">
              <span>资产</span>
              <span>现价</span>
              <span>成本 / 触发</span>
              <span>日涨跌</span>
              <span>浮动盈亏</span>
              <span>状态</span>
            </div>
            {rows.map((row) => (
              <div className="table-row" key={row.key}>
                <div className="asset-cell">
                  <div className="asset-title">
                    <span className="asset-name">{row.name}</span>
                    <Tag>{row.kind}</Tag>
                  </div>
                  <span className="asset-meta">
                    {row.id}
                    {row.note ? ` · ${row.note}` : ""}
                  </span>
                  {row.gold_price ? (
                    <span className="asset-gold" title={row.gold_price.note}>
                      国内金价{" "}
                      {row.gold_price.buy_price != null &&
                      row.gold_price.sell_price != null ? (
                        <>
                          买入{" "}
                          <span className="mono">
                            {formatPrice(row.gold_price.buy_price)}
                          </span>
                          {" · "}
                          卖出{" "}
                          <span className="mono">
                            {formatPrice(row.gold_price.sell_price)}
                          </span>
                        </>
                      ) : (
                        <span className="mono">
                          {formatPrice(row.gold_price.price)}
                        </span>
                      )}{" "}
                      元/克
                      {row.gold_price.change_percent != null
                        ? ` · ${formatPercent(row.gold_price.change_percent)}`
                        : ""}
                    </span>
                  ) : null}
                  {row.sip?.enabled ? (
                    <span
                      className={row.sip.due_today ? "asset-sip due" : "asset-sip"}
                    >
                      定投{" "}
                      {hideAmounts
                        ? REDACTED
                        : formatAmount(row.sip.amount, row.currency)}{" "}
                      · {row.sip.schedule}
                      {row.sip.due_today
                        ? " · 今日"
                        : row.sip.next_at
                          ? ` · 下次 ${formatSipDate(row.sip.next_at)}`
                          : ""}
                    </span>
                  ) : null}
                  {row.sip?.enabled && row.sip.trade ? (
                    <span
                      className={
                        row.sip.trade.blocked
                          ? "asset-sip trade blocked"
                          : "asset-sip trade"
                      }
                      title={row.sip.trade.reason ?? undefined}
                    >
                      申购状态 {row.sip.trade.status}
                      {row.sip.trade.max_amount != null
                        ? ` · 上限 ${formatAmount(
                            row.sip.trade.max_amount,
                            row.currency,
                          )}`
                        : ""}
                      {row.sip.trade.blocked ? " · 本期不买入" : ""}
                      {row.sip.trade.error ? " · 状态查询失败" : ""}
                    </span>
                  ) : null}
                </div>
                <div className="cell">
                  <strong className="mono price">{formatPrice(row.price)}</strong>
                  <span className="cell-sub">{row.currency}</span>
                  {snapshot?.realtime && row.change_percent != null ? (
                    <span
                      className={
                        row.change_percent >= 0
                          ? "cell-sub live up"
                          : "cell-sub live down"
                      }
                      title="实时涨跌（相对昨收），随实时价格一起刷新"
                    >
                      实时 {formatSignedPrice(row.change_amount)} (
                      {formatPercent(row.change_percent)})
                    </span>
                  ) : null}
                </div>
                <div className="cell">
                  <span className="mono">
                    {hideAmounts ? REDACTED : formatPrice(row.cost)}
                  </span>
                  <span className="cell-sub">
                    触发{" "}
                    <span className="mono">
                      {hideAmounts ? REDACTED : formatPrice(row.target)}
                    </span>
                  </span>
                  {row.principal != null ? (
                    <span className="cell-sub">
                      剩余本金{" "}
                      <span className="mono">
                        {hideAmounts
                          ? REDACTED
                          : formatMoney(row.principal, row.currency)}
                      </span>
                    </span>
                  ) : null}
                </div>
                <div className="cell">
                  <span
                    className={
                      row.change_percent == null
                        ? "mono muted"
                        : row.change_percent >= 0
                          ? "mono up"
                          : "mono down"
                    }
                  >
                    {formatPercent(row.change_percent)}
                  </span>
                </div>
                <div className="cell">
                  {hideAmounts ? (
                    <span className="muted">{REDACTED}</span>
                  ) : row.pnl == null ? (
                    <span className="muted">—</span>
                  ) : (
                    <span className={row.pnl >= 0 ? "mono up" : "mono down"}>
                      {formatMoney(row.pnl, row.currency)}
                    </span>
                  )}
                </div>
                <div className="cell">
                  {row.triggered ? (
                    <Tag tone="warn">已触发</Tag>
                  ) : row.closed ? (
                    <Tag>已清仓</Tag>
                  ) : row.recovered ? (
                    <Tag tone="ok">本金已收回</Tag>
                  ) : row.above ? (
                    <Tag tone="ok">已回本</Tag>
                  ) : (
                    <Tag>未回本</Tag>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}
      </section>

      <p className="footnote">
        行情来自东方财富 / 天天基金 / 蛋卷 / 新浪，默认每 {snapshot?.interval_secs ?? 60}{" "}
        秒刷新一次。港股与美股盈亏按原币种计算。
        {totals.size > 0
          ? ` 当前币种：${[...totals.keys()]
              .map((currency) => currencySymbol(currency) || currency)
              .join(" / ")}。`
          : ""}
      </p>

      {adding ? (
        <AssetModal
          initial={emptyAssetDraft()}
          index={null}
          globalThreshold={config?.threshold_percent ?? 0}
          onSave={async (draft) => {
            if (await onAddAsset(draft)) setAdding(false);
          }}
          onClose={() => setAdding(false)}
        />
      ) : null}
    </div>
  );
}
