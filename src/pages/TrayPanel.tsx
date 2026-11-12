import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  AlertCircle,
  ArrowUpRight,
  Bell,
  CalendarClock,
  Eye,
  EyeOff,
  Pause,
  Play,
  RefreshCw,
} from "lucide-react";
import { api } from "../api";
import type { NoticeRecord, Row, Snapshot } from "../types";
import {
  formatAmount,
  formatMoney,
  formatPercent,
  formatPrice,
  formatTime,
} from "../format";

const REDACTED = "••••";
const MAX_ROWS = 5;

const KIND_LABEL: Record<string, string> = {
  stock: "股票",
  fund: "基金",
  gold: "黄金",
};

/** 托盘下拉面板：只保留盯盘时最需要一眼看到的信息。 */
export function TrayPanel() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [notice, setNotice] = useState<NoticeRecord | null>(null);
  const [busy, setBusy] = useState(false);
  const [hideAmounts, setHideAmounts] = useState(
    () => localStorage.getItem("costline-hide-amounts") === "1",
  );
  const panelRef = useRef<HTMLDivElement>(null);

  // 主题 / 金额隐藏和主面板共用 localStorage，切换后同步过来。
  useEffect(() => {
    function syncTheme() {
      const mode = localStorage.getItem("costline-theme");
      const resolved =
        mode === "light" || mode === "dark"
          ? mode
          : window.matchMedia("(prefers-color-scheme: dark)").matches
            ? "dark"
            : "light";
      document.documentElement.dataset.theme = resolved;
      document.documentElement.style.colorScheme = resolved;
    }
    function syncAmounts() {
      setHideAmounts(localStorage.getItem("costline-hide-amounts") === "1");
    }
    syncTheme();
    window.addEventListener("storage", syncTheme);
    window.addEventListener("storage", syncAmounts);
    return () => {
      window.removeEventListener("storage", syncTheme);
      window.removeEventListener("storage", syncAmounts);
    };
  }, []);

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (event.key === "Escape") void api.hideTrayPanel();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const loadNotice = useCallback(() => {
    void api
      .getNoticeHistory()
      .then((items) => setNotice(items[0] ?? null))
      .catch(() => {});
  }, []);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    const track = (unlisten: () => void) => {
      if (disposed) unlisten();
      else stops.push(unlisten);
    };

    void api.getSnapshot().then(setSnapshot).catch(() => {});
    loadNotice();

    listen<Snapshot>("snapshot", (event) => setSnapshot(event.payload)).then(track);
    listen("notice", loadNotice).then(track);
    listen("notice-cleared", () => setNotice(null)).then(track);

    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, [loadNotice]);

  // 内容高度变化时让窗口贴合内容，避免面板下方留一大块空白。
  useEffect(() => {
    const node = panelRef.current;
    if (!node) return;
    let last = 0;
    let frame = 0;
    const measure = () => {
      frame = 0;
      const height = Math.ceil(node.getBoundingClientRect().height);
      if (height > 0 && Math.abs(height - last) >= 1) {
        last = height;
        void api.resizeTrayPanel(height + 2);
      }
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(measure);
    };
    schedule();
    const observer = new ResizeObserver(schedule);
    observer.observe(node);
    return () => {
      observer.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);

  const rows = snapshot?.rows ?? [];
  const assetsCount = snapshot?.assets_count ?? rows.length;
  const recovered = rows.filter((row) => row.above).length;
  const triggered = rows.filter((row) => row.triggered).length;
  const sipDue = rows.filter((row) => row.sip?.enabled && row.sip.due_today);

  const totals = useMemo(() => {
    const map = new Map<string, number>();
    for (const row of rows) {
      if (row.pnl != null) {
        map.set(row.currency, (map.get(row.currency) ?? 0) + row.pnl);
      }
    }
    return [...map.entries()];
  }, [rows]);

  // 重要的排前面：已触发 > 最接近触发价 > 已回本 > 已清仓。
  const focused = useMemo(() => {
    const score = (row: Row) => {
      if (row.triggered) return 0;
      if (!row.above && !row.closed) return 1;
      if (row.above) return 2;
      return 3;
    };
    const gap = (row: Row) => {
      if (row.target == null || row.target <= 0) return Number.POSITIVE_INFINITY;
      return (row.target - row.price) / row.target;
    };
    return [...rows]
      .sort((a, b) => score(a) - score(b) || gap(a) - gap(b))
      .slice(0, MAX_ROWS);
  }, [rows]);

  async function checkNow() {
    setBusy(true);
    try {
      setSnapshot(await api.checkNow());
    } catch {
      // 错误会写进 snapshot.last_error，这里不额外弹提示。
    } finally {
      setBusy(false);
    }
  }

  async function toggleWatch() {
    if (!snapshot) return;
    setBusy(true);
    try {
      setSnapshot(await api.setWatching(!snapshot.watching));
    } catch {
      // 同上。
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="tray-panel" ref={panelRef}>
      <header className="tray-head">
        <div className="tray-head-main">
          <span className="tray-brand">
            <span
              className={snapshot?.watching ? "tray-dot live" : "tray-dot"}
            />
            costline
          </span>
          <span className="tray-head-meta">
            {snapshot
              ? snapshot.watching
                ? "自动盯盘中"
                : "已暂停"
              : "正在加载…"}
            <span className="tray-sep">·</span>
            {snapshot?.last_check
              ? `检查于 ${formatTime(snapshot.last_check)}`
              : "等待首次检查"}
          </span>
          <button
            type="button"
            className="tray-icon-btn"
            title={hideAmounts ? "显示金额" : "隐藏金额"}
            aria-label={hideAmounts ? "显示金额" : "隐藏金额"}
            onClick={() => {
              const next = !hideAmounts;
              setHideAmounts(next);
              localStorage.setItem("costline-hide-amounts", next ? "1" : "0");
            }}
          >
            {hideAmounts ? <Eye size={14} /> : <EyeOff size={14} />}
          </button>
        </div>

        <div className="tray-summary">
          <span className="tray-summary-label">浮动盈亏</span>
          {totals.length === 0 ? (
            <strong className="tray-pnl muted">—</strong>
          ) : hideAmounts ? (
            <strong className="tray-pnl muted">{REDACTED}</strong>
          ) : (
            <div className="tray-pnl-row">
              {totals.map(([currency, value]) => (
                <strong
                  key={currency}
                  className={value >= 0 ? "tray-pnl up" : "tray-pnl down"}
                >
                  {formatMoney(value, currency)}
                </strong>
              ))}
            </div>
          )}
          <div className="tray-metrics">
            <span>
              持仓 <strong>{assetsCount}</strong>
            </span>
            <span>
              已回本 <strong>{recovered}</strong>
            </span>
            <span className={triggered > 0 ? "warn" : undefined}>
              触发 <strong>{triggered}</strong>
            </span>
          </div>
        </div>
      </header>

      {snapshot?.last_error ? (
        <div className="tray-error" title={snapshot.last_error}>
          <AlertCircle size={13} />
          <span>{snapshot.last_error}</span>
        </div>
      ) : null}

      <section className="tray-block">
        <div className="tray-block-head">
          <span className="tray-block-title">重点资产</span>
          <button
            type="button"
            className="tray-link"
            onClick={() => void api.openDashboard("dashboard")}
          >
            全部
            <ArrowUpRight size={12} />
          </button>
        </div>

        {rows.length === 0 ? (
          <div className="tray-empty">
            {assetsCount === 0
              ? "还没有添加资产"
              : "正在获取第一轮行情…"}
          </div>
        ) : (
          <div className="tray-rows">
            {focused.map((row) => {
              const status = rowStatus(row);
              return (
                <div className="tray-row" key={row.key}>
                  <div className="tray-row-main">
                    <span className="tray-row-name" title={row.name}>
                      {row.name}
                      <em>{KIND_LABEL[row.kind] ?? row.kind}</em>
                    </span>
                    <span className="tray-row-price mono">
                      {formatPrice(row.price)}
                    </span>
                  </div>
                  <div className="tray-row-sub">
                    <span className={`tray-row-state ${status.tone}`}>
                      {status.text}
                    </span>
                    <span
                      className={
                        row.change_percent == null
                          ? "tray-row-change muted"
                          : row.change_percent >= 0
                            ? "tray-row-change up"
                            : "tray-row-change down"
                      }
                    >
                      {formatPercent(row.change_percent)}
                    </span>
                    {row.pnl != null && !hideAmounts ? (
                      <span
                        className={
                          row.pnl >= 0
                            ? "tray-row-pnl up"
                            : "tray-row-pnl down"
                        }
                      >
                        {formatMoney(row.pnl, row.currency)}
                      </span>
                    ) : null}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      {sipDue.length > 0 ? (
        <section className="tray-block">
          <div className="tray-block-head">
            <span className="tray-block-title">
              <CalendarClock size={13} />
              今日定投
            </span>
          </div>
          {sipDue.map((row) => {
            const sip = row.sip!;
            const blocked = sip.trade?.blocked ?? false;
            return (
              <div className="tray-sip" key={row.key}>
                <span className="tray-sip-name">{row.name}</span>
                <span className="tray-sip-amount mono">
                  {hideAmounts
                    ? REDACTED
                    : formatAmount(sip.amount, row.currency)}
                </span>
                <span
                  className={
                    blocked ? "tray-sip-note blocked" : "tray-sip-note"
                  }
                  title={sip.trade?.reason ?? undefined}
                >
                  {blocked ? "本期不买入" : sip.schedule}
                </span>
              </div>
            );
          })}
        </section>
      ) : null}

      {notice ? (
        <section className="tray-block">
          <button
            type="button"
            className="tray-notice"
            onClick={() => void api.openDashboard("history")}
            title="查看消息历史"
          >
            <span className="tray-notice-head">
              <span className="tray-block-title">
                <Bell size={13} />
                最近提醒
              </span>
              <em>{timeAgo(notice.at)}</em>
            </span>
            <strong>{notice.title}</strong>
            <span className="tray-notice-body">{notice.body}</span>
          </button>
        </section>
      ) : null}

      <footer className="tray-foot">
        <button
          type="button"
          className="tray-action"
          onClick={checkNow}
          disabled={busy}
        >
          <RefreshCw size={14} className={busy ? "spin" : ""} />
          检查
        </button>
        <button
          type="button"
          className="tray-action"
          onClick={toggleWatch}
          disabled={busy || !snapshot}
        >
          {snapshot?.watching ? <Pause size={14} /> : <Play size={14} />}
          {snapshot?.watching ? "暂停" : "继续"}
        </button>
        <button
          type="button"
          className="tray-primary"
          onClick={() => void api.openDashboard("dashboard")}
        >
          打开面板
          <ArrowUpRight size={14} />
        </button>
      </footer>
    </div>
  );
}

type StatusTone = "warn" | "ok" | "muted";

function rowStatus(row: Row): { text: string; tone: StatusTone } {
  if (row.triggered) return { text: "已触发提醒", tone: "warn" };
  if (row.closed) return { text: "已清仓", tone: "muted" };
  if (row.recovered) return { text: "本金已收回", tone: "ok" };
  if (row.above) return { text: "已回本", tone: "ok" };
  if (row.target != null && row.target > 0) {
    const gap = ((row.target - row.price) / row.target) * 100;
    if (gap <= 0) return { text: "已越过触发价", tone: "warn" };
    return { text: `距触发 ${gap.toFixed(2)}%`, tone: "muted" };
  }
  return { text: "盯盘中", tone: "muted" };
}

function timeAgo(timestamp: number): string {
  const diff = Math.floor(Date.now() / 1000) - timestamp;
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  const date = new Date(timestamp * 1000);
  return `${date.getMonth() + 1}-${String(date.getDate()).padStart(2, "0")}`;
}
