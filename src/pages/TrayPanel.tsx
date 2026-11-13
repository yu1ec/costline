import {
  Fragment,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { listen } from "@tauri-apps/api/event";
import {
  AlertCircle,
  ArrowUpRight,
  Bell,
  CalendarClock,
  ChevronDown,
  ChevronUp,
  Eye,
  EyeOff,
  GripVertical,
  Minus,
  Pause,
  Play,
  Plus,
  RefreshCw,
  RotateCcw,
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
const TRAY_ROWS_KEY = "costline-tray-rows";
const TRAY_ORDER_KEY = "costline-tray-order";
const DEFAULT_TRAY_ROWS = 5;
const MIN_TRAY_ROWS = 1;
const MAX_TRAY_ROWS = 20;

function loadTrayRows(): number {
  const value = Number(localStorage.getItem(TRAY_ROWS_KEY));
  if (!Number.isFinite(value) || value < MIN_TRAY_ROWS) return DEFAULT_TRAY_ROWS;
  return Math.min(MAX_TRAY_ROWS, Math.trunc(value));
}

/** 手动置顶顺序，存的是资产 key；没存过的条目继续按盯盘优先级排序。 */
function loadTrayOrder(): string[] {
  try {
    const raw = localStorage.getItem(TRAY_ORDER_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    return Array.isArray(parsed)
      ? parsed.filter((key): key is string => typeof key === "string")
      : [];
  } catch {
    return [];
  }
}

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
  const [maxRows, setMaxRows] = useState(loadTrayRows);
  const [order, setOrder] = useState<string[]>(loadTrayOrder);
  const [dragKey, setDragKey] = useState<string | null>(null);
  const [dropKey, setDropKey] = useState<string | null>(null);
  // 落点在目标行上半区还是下半区：决定插入到它前面还是后面。
  const [dropAfter, setDropAfter] = useState(false);
  // 被拎起来的行原地留一个等高的空位，避免列表在起拖瞬间塌一下。
  const [dragHeight, setDragHeight] = useState(0);
  // 松手后短暂加一个回落动画，做出「塞进槽位」的手感。
  const [justDropped, setJustDropped] = useState<string | null>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  const rowsRef = useRef<HTMLDivElement>(null);
  const rowRefs = useRef(new Map<string, HTMLDivElement>());
  // 用 pointer 事件自己实现拖动：WKWebView 里 HTML5 drag 事件不一定触发。
  const dragState = useRef<{
    key: string;
    pointerId: number;
    startY: number;
    active: boolean;
  } | null>(null);
  const dropKeyRef = useRef<string | null>(null);
  const dropAfterRef = useRef(false);
  const settleTimer = useRef<number | null>(null);

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

  // 手动拖过的条目按固定顺序排在最前，其余仍按：已触发 > 最接近触发价 > 已回本 > 已清仓。
  const focused = useMemo(() => {
    const rank = new Map(order.map((key, index) => [key, index]));
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
      .sort((a, b) => {
        const ra = rank.get(a.key) ?? Number.POSITIVE_INFINITY;
        const rb = rank.get(b.key) ?? Number.POSITIVE_INFINITY;
        if (ra !== rb) return ra - rb;
        return score(a) - score(b) || gap(a) - gap(b);
      })
      .slice(0, maxRows);
  }, [rows, maxRows, order]);

  /** 把 fromKey 插到 toKey 的前面（after=false）或后面（after=true）。 */
  function moveRowTo(fromKey: string, toKey: string, after: boolean) {
    if (fromKey === toKey) return;
    const visible = focused.map((row) => row.key);
    const from = visible.indexOf(fromKey);
    const to = visible.indexOf(toKey);
    if (from < 0 || to < 0) return;
    const next = visible.filter((key) => key !== fromKey);
    next.splice(next.indexOf(toKey) + (after ? 1 : 0), 0, fromKey);
    // 只固定拖到目标位置及以上的顺序，下面继续自动排序。
    const pinned = next.slice(0, next.indexOf(fromKey) + 1);
    const existing = new Set(rows.map((row) => row.key));
    const merged = [
      ...pinned,
      ...order.filter((key) => !pinned.includes(key) && existing.has(key)),
    ];
    setOrder(merged);
    localStorage.setItem(TRAY_ORDER_KEY, JSON.stringify(merged));
  }

  function resetOrder() {
    setOrder([]);
    localStorage.removeItem(TRAY_ORDER_KEY);
  }

  function updateDropTarget(key: string | null, after: boolean) {
    if (dropKeyRef.current === key && dropAfterRef.current === after) return;
    dropKeyRef.current = key;
    dropAfterRef.current = after;
    setDropKey(key);
    setDropAfter(after);
  }

  /** 起拖时把整行从文档流里拎出来，固定在按下时的位置，之后只改 transform。 */
  function liftRow(el: HTMLDivElement) {
    const rect = el.getBoundingClientRect();
    el.style.position = "fixed";
    el.style.left = `${rect.left}px`;
    el.style.top = `${rect.top}px`;
    el.style.width = `${rect.width}px`;
    el.style.height = `${rect.height}px`;
    el.style.margin = "0";
    el.style.zIndex = "50";
    el.style.willChange = "transform";
    el.style.transform = "translate3d(0, 0, 0)";
  }

  function moveLift(el: HTMLDivElement, dy: number) {
    el.style.transform = `translate3d(0, ${dy}px, 0)`;
  }

  function dropLift(el: HTMLDivElement) {
    el.style.position = "";
    el.style.left = "";
    el.style.top = "";
    el.style.width = "";
    el.style.height = "";
    el.style.margin = "";
    el.style.zIndex = "";
    el.style.willChange = "";
    el.style.transform = "";
  }

  function startRowDrag(
    event: ReactPointerEvent<HTMLDivElement>,
    key: string,
  ) {
    if (event.button !== 0) return;
    // 行内的上下按钮负责点击，不要被拖动逻辑吞掉。
    if ((event.target as Element | null)?.closest("button")) return;
    // 提前吃掉 mousedown 的默认行为，避免拖动行时顺手选中文字。
    event.preventDefault();
    dragState.current = {
      key,
      pointerId: event.pointerId,
      startY: event.clientY,
      active: false,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function moveRowDrag(event: ReactPointerEvent<HTMLDivElement>) {
    const state = dragState.current;
    if (!state || state.pointerId !== event.pointerId) return;
    const el = rowRefs.current.get(state.key) ?? null;
    if (!state.active) {
      // 超过 4px 才算拖动，避免把普通点击当成拖拽。
      if (Math.abs(event.clientY - state.startY) < 4) return;
      state.active = true;
      // 拖动期间彻底屏蔽选区，并清掉可能已经产生的选中文字。
      window.getSelection()?.removeAllRanges();
      document.documentElement.classList.add("row-dragging");
      setDragHeight(el?.offsetHeight ?? 0);
      setDragKey(state.key);
      if (el) liftRow(el);
    }
    event.preventDefault();
    if (el) moveLift(el, event.clientY - state.startY);
    const container = rowsRef.current;
    if (container) {
      const rect = container.getBoundingClientRect();
      if (event.clientY < rect.top + 24) {
        container.scrollTop -= 8;
      } else if (event.clientY > rect.bottom - 24) {
        container.scrollTop += 8;
      }
    }
    // 被拖的行跟着指针走，命中判断要跳过它自己，找下面真正悬停的那一行。
    let hit: HTMLElement | null = null;
    for (const node of document.elementsFromPoint(
      event.clientX,
      event.clientY,
    )) {
      const row = (node as Element).closest<HTMLElement>("[data-row-key]");
      if (row && row.dataset.rowKey !== state.key) {
        hit = row;
        break;
      }
    }
    if (!hit) {
      // 指针落在让位产生的空隙里时，退回到最近的一行，避免落点反复跳动。
      const bounds = rowsRef.current?.getBoundingClientRect();
      if (
        bounds &&
        event.clientY >= bounds.top &&
        event.clientY <= bounds.bottom
      ) {
        let nearest = Number.POSITIVE_INFINITY;
        for (const [key, node] of rowRefs.current) {
          if (key === state.key) continue;
          const rect = node.getBoundingClientRect();
          const distance = Math.abs(
            event.clientY - (rect.top + rect.height / 2),
          );
          if (distance < nearest) {
            nearest = distance;
            hit = node;
          }
        }
      }
    }
    if (!hit) {
      updateDropTarget(null, false);
      return;
    }
    // 落在目标行上半区插它前面，下半区插它后面。
    const rect = hit.getBoundingClientRect();
    updateDropTarget(
      hit.dataset.rowKey ?? null,
      event.clientY > rect.top + rect.height / 2,
    );
  }

  function endRowDrag(event: ReactPointerEvent<HTMLDivElement>) {
    const state = dragState.current;
    if (!state || state.pointerId !== event.pointerId) return;
    dragState.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    const el = rowRefs.current.get(state.key);
    if (el) dropLift(el);
    const target = dropKeyRef.current;
    const after = dropAfterRef.current;
    document.documentElement.classList.remove("row-dragging");
    setDragKey(null);
    setDragHeight(0);
    updateDropTarget(null, false);
    if (state.active && target) {
      moveRowTo(state.key, target, after);
      // 松手后让落位的行轻轻回落一下，做出「塞进槽位」的手感。
      setJustDropped(state.key);
      if (settleTimer.current) window.clearTimeout(settleTimer.current);
      settleTimer.current = window.setTimeout(() => setJustDropped(null), 260);
    }
  }

  useEffect(
    () => () => {
      if (settleTimer.current) window.clearTimeout(settleTimer.current);
    },
    [],
  );

  function changeRows(delta: number) {
    setMaxRows((current) => {
      const next = Math.min(
        MAX_TRAY_ROWS,
        Math.max(MIN_TRAY_ROWS, current + delta),
      );
      if (next !== current) localStorage.setItem(TRAY_ROWS_KEY, String(next));
      return next;
    });
  }

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

  // 拖动中：其余行按落点平移，给被拖的行腾出插入槽位。
  const dragIndex = dragKey
    ? focused.findIndex((row) => row.key === dragKey)
    : -1;
  const dragGapAt = (() => {
    if (!dragKey || dragIndex < 0 || !dropKey || dropKey === dragKey) return null;
    const rest = focused.map((row) => row.key).filter((key) => key !== dragKey);
    const hovered = rest.indexOf(dropKey);
    return hovered < 0 ? null : hovered + (dropAfter ? 1 : 0);
  })();

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
          <div className="tray-block-tools">
            {order.length > 0 ? (
              <button
                type="button"
                className="tray-icon-btn tray-order-reset"
                title="恢复默认排序"
                aria-label="恢复默认排序"
                onClick={resetOrder}
              >
                <RotateCcw size={12} />
              </button>
            ) : null}
            <span
              className="tray-rows-control"
              title={`重点资产显示 ${maxRows} 条（${MIN_TRAY_ROWS}-${MAX_TRAY_ROWS}）`}
            >
              <button
                type="button"
                className="tray-step"
                aria-label="减少重点资产显示数量"
                disabled={maxRows <= MIN_TRAY_ROWS}
                onClick={() => changeRows(-1)}
              >
                <Minus size={11} />
              </button>
              <span className="tray-rows-count">{maxRows}</span>
              <button
                type="button"
                className="tray-step"
                aria-label="增加重点资产显示数量"
                disabled={maxRows >= MAX_TRAY_ROWS}
                onClick={() => changeRows(1)}
              >
                <Plus size={11} />
              </button>
            </span>
            <button
              type="button"
              className="tray-link"
              onClick={() => void api.openDashboard("dashboard")}
            >
              全部
              <ArrowUpRight size={12} />
            </button>
          </div>
        </div>

        {rows.length === 0 ? (
          <div className="tray-empty">
            {assetsCount === 0
              ? "还没有添加资产"
              : "正在获取第一轮行情…"}
          </div>
        ) : (
          <div
            className={dragKey ? "tray-rows is-dragging" : "tray-rows"}
            ref={rowsRef}
          >
            {focused.map((row, index) => {
              const status = rowStatus(row);
              const dragging = dragKey === row.key;
              const dropTarget = dropKey === row.key && !dragging;
              // 被拎起来的行让出高度后，其余行按落点上下平移，视觉上就是给它腾出槽位。
              let shift = 0;
              if (dropKey && !dragging && dragGapAt != null && dragHeight > 0) {
                if (index > dragIndex) shift -= dragHeight;
                const restIndex = index < dragIndex ? index : index - 1;
                if (restIndex >= dragGapAt) shift += dragHeight;
              }
              // 被拖走的行原地留个等高空位，列表不会在起拖瞬间塌一下。
              const gap = dragging && dragHeight > 0;
              // 上下按钮是拖动的兜底：点击按钮不会被拖动逻辑接管。
              const prev = index > 0 ? focused[index - 1] : null;
              const next =
                index < focused.length - 1 ? focused[index + 1] : null;
              return (
                <Fragment key={row.key}>
                  {gap ? (
                    <div
                      className="tray-row-gap"
                      style={{ height: dragHeight }}
                      aria-hidden="true"
                    />
                  ) : null}
                  <div
                    className={[
                      "tray-row",
                      dragging ? "dragging" : "",
                      dropTarget
                        ? dropAfter
                          ? "drop-after"
                          : "drop-before"
                        : "",
                      justDropped === row.key && !dragging ? "dropped" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    ref={(node) => {
                      if (node) rowRefs.current.set(row.key, node);
                      else rowRefs.current.delete(row.key);
                    }}
                    style={
                      shift
                        ? { transform: `translateY(${shift}px)` }
                        : undefined
                    }
                    data-row-key={row.key}
                    onPointerDown={(event) => startRowDrag(event, row.key)}
                    onPointerMove={moveRowDrag}
                    onPointerUp={endRowDrag}
                    onPointerCancel={endRowDrag}
                    onLostPointerCapture={endRowDrag}
                  >
                    <span className="tray-row-move">
                      <button
                        type="button"
                        className="tray-move"
                        title="上移"
                        aria-label={`上移 ${row.name}`}
                        draggable={false}
                        disabled={!prev}
                        onClick={() => {
                          if (prev) moveRowTo(row.key, prev.key, false);
                        }}
                      >
                        <ChevronUp size={11} />
                      </button>
                      <button
                        type="button"
                        className="tray-move"
                        title="下移"
                        aria-label={`下移 ${row.name}`}
                        disabled={!next}
                        onClick={() => {
                          if (next) moveRowTo(row.key, next.key, true);
                        }}
                      >
                        <ChevronDown size={11} />
                      </button>
                    </span>
                    <span
                      className="tray-row-grip"
                      title="按住拖动调整顺序"
                      aria-hidden="true"
                    >
                      <GripVertical size={12} />
                    </span>
                    <div className="tray-row-body">
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
                  </div>
                </Fragment>
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

