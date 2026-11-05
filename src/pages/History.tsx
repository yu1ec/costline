import { useCallback, useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Bell,
  ChevronDown,
  ChevronUp,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { api } from "../api";
import type { NoticeKind, NoticeRecord } from "../types";
import { EmptyState, Tag } from "../components/ui";

type Filter = "all" | NoticeKind;

const KIND_META: Record<
  NoticeKind,
  { label: string; tone: "ok" | "warn" | "muted" | "accent" }
> = {
  price: { label: "回本提醒", tone: "warn" },
  sip: { label: "定投提醒", tone: "accent" },
  test: { label: "测试消息", tone: "muted" },
};

const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "全部" },
  { value: "price", label: "回本提醒" },
  { value: "sip", label: "定投提醒" },
  { value: "test", label: "测试消息" },
];

export function History({
  focusId,
  focusNonce,
  onToast,
}: {
  focusId: string | null;
  focusNonce: number;
  onToast: (text: string, tone?: "ok" | "error") => void;
}) {
  const [records, setRecords] = useState<NoticeRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [highlight, setHighlight] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);

  const load = useCallback(async (showSpinner = false) => {
    if (showSpinner) setRefreshing(true);
    try {
      const items = await api.getNoticeHistory();
      setRecords(items);
      setError(null);
    } catch (loadError) {
      setError(errorMessage(loadError));
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let disposed = false;
    const stops: (() => void)[] = [];
    listen<NoticeRecord>("notice", (event) => {
      setRecords((current) => [
        event.payload,
        ...current.filter((record) => record.id !== event.payload.id),
      ]);
    }).then((stop) => {
      if (disposed) stop();
      else stops.push(stop);
    });
    listen("notice-cleared", () => setRecords([])).then((stop) => {
      if (disposed) stop();
      else stops.push(stop);
    });
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, []);

  // 从定投提醒的「查看」跳转过来时，滚到对应消息并展开。
  useEffect(() => {
    if (!focusId || loading) return;
    setFilter("all");
    setExpanded((current) => new Set(current).add(focusId));
    setHighlight(focusId);
    const scroll = setTimeout(() => {
      document
        .getElementById(`notice-${focusId}`)
        ?.scrollIntoView({ behavior: "smooth", block: "center" });
    }, 80);
    const clear = setTimeout(() => setHighlight(null), 2400);
    return () => {
      clearTimeout(scroll);
      clearTimeout(clear);
    };
  }, [focusId, focusNonce, loading]);

  const counts = useMemo(() => {
    const result: Record<Filter, number> = {
      all: records.length,
      price: 0,
      sip: 0,
      test: 0,
    };
    for (const record of records) result[record.kind] += 1;
    return result;
  }, [records]);

  const groups = useMemo(() => {
    const sorted = [...records].sort(
      (a, b) => b.at - a.at || b.id.localeCompare(a.id),
    );
    const visible =
      filter === "all" ? sorted : sorted.filter((record) => record.kind === filter);
    const grouped = new Map<string, { label: string; items: NoticeRecord[] }>();
    for (const record of visible) {
      const key = dayKey(record.at);
      const group = grouped.get(key);
      if (group) group.items.push(record);
      else grouped.set(key, { label: dayLabel(record.at), items: [record] });
    }
    return [...grouped.values()];
  }, [records, filter]);

  function toggle(id: string) {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  useEffect(() => {
    if (!confirmClear) return;
    const timer = setTimeout(() => setConfirmClear(false), 3000);
    return () => clearTimeout(timer);
  }, [confirmClear]);

  async function clearAll() {
    if (!confirmClear) {
      setConfirmClear(true);
      return;
    }
    setConfirmClear(false);
    setClearing(true);
    try {
      await api.clearNoticeHistory();
      setRecords([]);
      onToast("消息历史已清空");
    } catch (clearError) {
      onToast(errorMessage(clearError), "error");
    } finally {
      setClearing(false);
    }
  }

  return (
    <div className="page history-page">
      <section className="panel history-panel">
        <div className="panel-head">
          <div>
            <h2>消息历史</h2>
            <p>
              保存已经成功推送出去的通知，最多保留最近 300 条。点击消息可展开完整内容。
            </p>
          </div>
          <div className="panel-actions">
            <button
              type="button"
              className="btn ghost"
              onClick={() => void load(true)}
              disabled={refreshing}
            >
              <RefreshCw size={15} className={refreshing ? "spin" : ""} />
              刷新
            </button>
            <button
              type="button"
              className={confirmClear ? "btn ghost danger" : "btn ghost"}
              onClick={() => void clearAll()}
              disabled={records.length === 0 || clearing}
              title={confirmClear ? "再次点击确认清空" : "清空消息历史"}
            >
              <Trash2 size={15} />
              {confirmClear ? "确认清空" : "清空"}
            </button>
          </div>
        </div>

        <div className="history-filters" role="tablist">
          {FILTERS.map((item) => (
            <button
              key={item.value}
              type="button"
              role="tab"
              aria-selected={filter === item.value}
              className={
                filter === item.value ? "history-filter active" : "history-filter"
              }
              onClick={() => setFilter(item.value)}
            >
              {item.label}
              <em>{counts[item.value]}</em>
            </button>
          ))}
        </div>

        {error ? (
          <div className="banner error">
            <div>
              <strong>读取消息历史失败</strong>
              <p>{error}</p>
            </div>
          </div>
        ) : null}

        {loading ? (
          <div className="loading-state">
            <RefreshCw size={18} className="spin" />
            <span>正在读取消息历史…</span>
          </div>
        ) : groups.length === 0 ? (
          <EmptyState
            icon={<Bell size={22} />}
            title={records.length === 0 ? "还没有推送过消息" : "没有符合筛选的消息"}
            description={
              records.length === 0
                ? "回本提醒、定投提醒和通知测试都会记录在这里，方便事后查看。"
                : "换一个筛选条件看看。"
            }
          />
        ) : (
          groups.map((group) => (
            <section className="history-group" key={group.label}>
              <h3>
                {group.label}
                <span>{group.items.length} 条</span>
              </h3>
              <div className="history-list">
                {group.items.map((record) => {
                  const meta = KIND_META[record.kind] ?? KIND_META.price;
                  const open = expanded.has(record.id);
                  return (
                    <article
                      key={record.id}
                      id={`notice-${record.id}`}
                      className={
                        highlight === record.id
                          ? "history-card highlight"
                          : "history-card"
                      }
                    >
                      <div className="history-card-head">
                        <div className="history-card-title">
                          <Tag tone={meta.tone}>{meta.label}</Tag>
                          <strong>{record.title}</strong>
                        </div>
                        <span className="history-time">{formatClock(record.at)}</span>
                      </div>
                      <div className="history-channels">
                        {record.channels.map((channel) => (
                          <span
                            key={`${record.id}-${channel.label}`}
                            className={
                              channel.ok
                                ? "history-channel ok"
                                : "history-channel fail"
                            }
                            title={channel.error ?? undefined}
                          >
                            <i aria-hidden />
                            {channel.label}
                          </span>
                        ))}
                      </div>
                      <p className={open ? "history-body open" : "history-body"}>
                        {record.body}
                      </p>
                      <button
                        type="button"
                        className="history-toggle"
                        onClick={() => toggle(record.id)}
                      >
                        {open ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
                        {open ? "收起" : "展开全文"}
                      </button>
                    </article>
                  );
                })}
              </div>
            </section>
          ))
        )}
      </section>
    </div>
  );
}

function startOfDay(timestamp: number): number {
  const date = new Date(timestamp * 1000);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

function dayKey(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
}

function dayLabel(timestamp: number): string {
  const today = startOfDay(Date.now() / 1000);
  const target = startOfDay(timestamp);
  const diff = Math.round((today - target) / 86_400_000);
  if (diff === 0) return "今天";
  if (diff === 1) return "昨天";
  const date = new Date(timestamp * 1000);
  return `${date.getFullYear()} 年 ${date.getMonth() + 1} 月 ${date.getDate()} 日`;
}

function formatClock(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
