import { useEffect, useRef, useState, type ReactNode } from "react";
import {
  Bell,
  Boxes,
  ChevronDown,
  ChevronRight,
  FileCode2,
  FileText,
  Files,
  GitBranch,
  ListChecks,
  Play,
  Search,
  Settings2,
  Share2,
  Star,
  Table2,
} from "lucide-react";
import {
  DISGUISE_PERSONAS,
  formatSigned,
  personaAlias,
  personaCode,
  type DisguisePersonaId,
} from "../disguise";
import { formatPercent, formatPrice } from "../format";
import type { Row, Snapshot } from "../types";

type StatusTone = "ok" | "warn" | "dim";

export function DisguiseScreen({
  persona,
  snapshot,
  onExit,
}: {
  persona: DisguisePersonaId;
  snapshot: Snapshot | null;
  onExit: () => void;
}) {
  const rows = snapshot?.rows ?? [];
  switch (persona) {
    case "dev":
      return <DevWorkspace rows={rows} onExit={onExit} />;
    case "ops":
      return <OpsBoard rows={rows} onExit={onExit} />;
    case "product":
      return <ProductDoc rows={rows} onExit={onExit} />;
    case "finance":
      return <FinanceSheet rows={rows} onExit={onExit} />;
  }
}

export function MaskedBoard({
  persona,
  snapshot,
  onExit,
}: {
  persona: DisguisePersonaId;
  snapshot: Snapshot | null;
  onExit: () => void;
}) {
  const meta = DISGUISE_PERSONAS[persona];
  const rows = snapshot?.rows ?? [];
  const above = rows.filter((row) => row.above).length;
  const alerts = rows.filter((row) => row.triggered).length;
  const totals = new Map<string, number>();
  for (const row of rows) {
    if (row.pnl != null) {
      totals.set(row.currency, (totals.get(row.currency) ?? 0) + row.pnl);
    }
  }
  const clock = useClock();
  const peek = usePeek();

  return (
    <div className="dg-app mk-shell">
      <header className="mk-top">
        <SecretExit onExit={onExit} className="mk-brand">
          <span className="mk-logo" style={{ background: meta.accent }}>
            {meta.brand.slice(0, 1)}
          </span>
          <span className="mk-brand-text">
            <strong>{meta.brand}</strong>
            <span>{meta.brandSub}</span>
          </span>
        </SecretExit>
        <nav className="mk-nav">
          {meta.nav.map((item, index) => (
            <span key={item} className={index === 0 ? "active" : ""}>
              {item}
            </span>
          ))}
        </nav>
        <div className="mk-top-right">
          <span className={snapshot?.watching ? "mk-sync live" : "mk-sync"}>
            <i />
            {snapshot?.watching ? "同步正常" : "同步暂停"}
          </span>
          <span className="mk-updated">更新于 {clock}</span>
          <span className="mk-avatar">{meta.avatar}</span>
        </div>
      </header>

      <main className="mk-main">
        <div className="mk-headline">
          <div>
            <h1>{meta.detailTitle}</h1>
            <p>{meta.footer}</p>
          </div>
          <span className="mk-range">近 30 天</span>
        </div>

        <section className="mk-metrics">
          <MaskMetric
            label={meta.totals.monitors}
            value={String(rows.length || snapshot?.assets_count || 0)}
            sub="监测中"
          />
          <MaskMetric
            label={meta.totals.ok}
            value={String(above)}
            sub={
              rows.length
                ? `占比 ${Math.round((above / rows.length) * 100)}%`
                : "—"
            }
            tone="ok"
          />
          <MaskMetric
            label={meta.totals.alerts}
            value={String(alerts)}
            sub={alerts > 0 ? "需要关注" : "暂无异常"}
            tone={alerts > 0 ? "warn" : undefined}
          />
          <div className="mk-metric">
            <span className="mk-metric-label">{meta.totals.delta}</span>
            {totals.size === 0 ? (
              <strong className="mk-mono">—</strong>
            ) : (
              <div className="mk-deltas">
                {[...totals.entries()].map(([currency, value]) => (
                  <strong
                    key={currency}
                    className={`mk-mono ${value >= 0 ? "pos" : "neg"}`}
                  >
                    {formatSigned(value)}
                    <em>{currency}</em>
                  </strong>
                ))}
              </div>
            )}
            <span className="mk-metric-sub">累计偏差</span>
          </div>
        </section>

        <section className="mk-panel">
          <div className="mk-panel-head">
            <div>
              <h2>{meta.detailTitle}</h2>
              <p>{meta.footer}</p>
            </div>
            <div className="mk-filters">
              <span className="active">全部</span>
              <span>重点关注</span>
              <span>已归档</span>
            </div>
          </div>

          {rows.length === 0 ? (
            <div className="mk-empty">暂无同步数据，等待下一次采集。</div>
          ) : (
            <div className="mk-table">
              <div className="mk-head-row">
                <span>{meta.columns.name}</span>
                <span>{meta.columns.value}</span>
                <span>{meta.columns.baseline}</span>
                <span>{meta.columns.change}</span>
                <span>{meta.columns.delta}</span>
                <span>{meta.columns.status}</span>
              </div>
              {rows.map((row, index) => {
                const status = row.triggered
                  ? { text: meta.status.triggered, tone: "warn" }
                  : row.above
                    ? { text: meta.status.above, tone: "ok" }
                    : { text: meta.status.below, tone: "dim" };
                return (
                  <div className="mk-row" key={row.key}>
                    <div className="mk-cell mk-name">
                      <strong>
                        {peek ? row.name : personaAlias(persona, index)}
                      </strong>
                      <span>
                        {peek
                          ? `${personaAlias(persona, index)} · ${meta.kindLabel}`
                          : `${personaCode(persona, index)} · ${meta.kindLabel}`}
                      </span>
                    </div>
                    <div className="mk-cell">
                      <strong className="mk-mono">{formatPrice(row.price)}</strong>
                      <span>当前值</span>
                    </div>
                    <div className="mk-cell">
                      <strong className="mk-mono">{formatPrice(row.target)}</strong>
                      <span>基线 {formatPrice(row.cost)}</span>
                    </div>
                    <div className="mk-cell">
                      <span
                        className={
                          row.change_percent == null
                            ? "mk-mono dim"
                            : row.change_percent >= 0
                              ? "mk-mono pos"
                              : "mk-mono neg"
                        }
                      >
                        {formatPercent(row.change_percent)}
                      </span>
                    </div>
                    <div className="mk-cell">
                      <span
                        className={
                          row.pnl == null
                            ? "dim"
                            : row.pnl >= 0
                              ? "mk-mono pos"
                              : "mk-mono neg"
                        }
                      >
                        {row.pnl == null ? "—" : formatSigned(row.pnl)}
                      </span>
                    </div>
                    <div className="mk-cell">
                      <span className={`mk-tag ${status.tone}`}>{status.text}</span>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      </main>
    </div>
  );
}

function MaskMetric({
  label,
  value,
  sub,
  tone,
}: {
  label: string;
  value: string;
  sub: string;
  tone?: "ok" | "warn";
}) {
  return (
    <div className={tone ? `mk-metric ${tone}` : "mk-metric"}>
      <span className="mk-metric-label">{label}</span>
      <strong className="mk-mono">{value}</strong>
      <span className="mk-metric-sub">{sub}</span>
    </div>
  );
}

/* ---------- 全屏伪装：开发 ---------- */

const DEV_CODE = `import type { RetryOptions } from "./types";
import { sleep } from "./sleep";

const DEFAULTS: RetryOptions = {
  attempts: 3,
  baseDelay: 200,
  factor: 2,
};

export async function withRetry<T>(
  task: () => Promise<T>,
  options: RetryOptions = DEFAULTS,
): Promise<T> {
  let lastError: unknown;

  for (let attempt = 0; attempt < options.attempts; attempt += 1) {
    try {
      return await task();
    } catch (error) {
      lastError = error;
      const delay = options.baseDelay * (options.factor ?? 1) ** attempt;
      await sleep(delay);
    }
  }

  throw lastError;
}

// TODO: 把重试次数上报到监控面板`.split("\n");

const DEV_KEYWORDS = new Set([
  "import",
  "type",
  "from",
  "const",
  "export",
  "async",
  "function",
  "let",
  "for",
  "try",
  "catch",
  "return",
  "await",
  "throw",
]);

const DEV_TOKEN =
  /(\/\/[^\n]*|"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|`(?:[^`\\]|\\.)*`|\b(?:import|type|from|const|export|async|function|let|for|try|catch|return|await|throw)\b|\b[A-Z][A-Za-z0-9_]*\b|\b\d+(?:\.\d+)?\b)/g;

function highlightCode(line: string) {
  return line.split(DEV_TOKEN).map((part, index) => {
    if (!part) return null;
    if (part.startsWith("//")) {
      return (
        <span key={index} className="dv-c">
          {part}
        </span>
      );
    }
    if (
      part.startsWith('"') ||
      part.startsWith("'") ||
      part.startsWith("`")
    ) {
      return (
        <span key={index} className="dv-s">
          {part}
        </span>
      );
    }
    if (/^\d+(\.\d+)?$/.test(part)) {
      return (
        <span key={index} className="dv-n">
          {part}
        </span>
      );
    }
    if (DEV_KEYWORDS.has(part)) {
      return (
        <span key={index} className="dv-k">
          {part}
        </span>
      );
    }
    if (/^[A-Z]/.test(part)) {
      return (
        <span key={index} className="dv-t">
          {part}
        </span>
      );
    }
    return <span key={index}>{part}</span>;
  });
}

const DEV_TREE: {
  depth: number;
  name: string;
  folder?: boolean;
  open?: boolean;
  active?: boolean;
}[] = [
  { depth: 0, name: "orbit-web", folder: true, open: true },
  { depth: 1, name: "src", folder: true, open: true },
  { depth: 2, name: "lib", folder: true, open: true },
  { depth: 3, name: "retry.ts", active: true },
  { depth: 3, name: "sleep.ts" },
  { depth: 3, name: "http.ts" },
  { depth: 2, name: "components", folder: true, open: true },
  { depth: 3, name: "checkout.tsx" },
  { depth: 3, name: "price-tag.tsx" },
  { depth: 2, name: "app.tsx" },
  { depth: 1, name: "tests", folder: true },
  { depth: 2, name: "retry.test.ts" },
  { depth: 1, name: "package.json" },
  { depth: 1, name: "tsconfig.json" },
];

function DevWorkspace({ rows, onExit }: { rows: Row[]; onExit: () => void }) {
  const peek = usePeek();
  const above = rows.filter((row) => row.above).length;
  const alerts = rows.filter((row) => row.triggered).length;
  const logs = rows.slice(0, 6);

  return (
    <div className="dg-app dv-shell">
      <div className="dv-top">
        <SecretExit onExit={onExit} className="dv-brand">
          <span className="dv-logo">O</span>
          <strong>orbit</strong>
          <span className="dv-slash">/</span>
          <span className="dv-repo">web-console</span>
        </SecretExit>
        <span className="dv-branch">
          <GitBranch size={12} /> feat/checkout-retry
        </span>
        <div className="dv-search">
          <Search size={13} />
          <span>搜索文件、符号或命令</span>
          <kbd>⌘K</kbd>
        </div>
        <div className="dv-top-right">
          <span className={alerts > 0 ? "dv-ci warn" : "dv-ci"}>
            {alerts > 0 ? `${alerts} job failed` : "CI passing"}
          </span>
          <span className="dv-avatar">YK</span>
        </div>
      </div>

      <div className="dv-body">
        <div className="dv-rail">
          <Files size={17} />
          <Search size={17} />
          <GitBranch size={17} />
          <Play size={17} />
          <Boxes size={17} />
          <Settings2 size={17} className="dv-rail-last" />
        </div>

        <aside className="dv-side">
          <div className="dv-side-head">资源管理器</div>
          <ul className="dv-tree">
            {DEV_TREE.map((item, index) => (
              <li
                key={`${item.name}-${index}`}
                className={item.active ? "active" : ""}
                style={{ paddingLeft: 10 + item.depth * 13 }}
              >
                {item.folder ? (
                  item.open ? (
                    <ChevronDown size={12} />
                  ) : (
                    <ChevronRight size={12} />
                  )
                ) : (
                  <FileCode2 size={12} className="dv-file-icon" />
                )}
                <span>{item.name}</span>
              </li>
            ))}
          </ul>
          <div className="dv-side-head">大纲</div>
          <ul className="dv-outline">
            <li>RetryOptions</li>
            <li>DEFAULTS</li>
            <li>withRetry</li>
          </ul>
        </aside>

        <main className="dv-main">
          <div className="dv-tabs">
            <span className="dv-tab active">
              retry.ts <i>●</i>
            </span>
            <span className="dv-tab">checkout.tsx</span>
            <span className="dv-tab">retry.test.ts</span>
          </div>
          <div className="dv-breadcrumb">
            src <b>›</b> lib <b>›</b> retry.ts
            <span>TypeScript</span>
          </div>
          <div className="dv-editor">
            {DEV_CODE.map((line, index) => (
              <div className="dv-line" key={index}>
                <span className="dv-ln">{index + 1}</span>
                <span className="dv-src">{highlightCode(line)}</span>
              </div>
            ))}
          </div>
          <div className="dv-term">
            <div className="dv-term-tabs">
              <span className="active">终端</span>
              <span>输出</span>
              <span>问题</span>
              <span>调试控制台</span>
            </div>
            <pre className="dv-term-body">
              <span className="dv-term-dim">$ pnpm run health:check</span>
              {"\n"}
              {logs.map((row, index) => (
                <span
                  key={row.key}
                  className={
                    row.triggered
                      ? "dv-term-warn"
                      : row.above
                        ? "dv-term-ok"
                        : "dv-term-dim"
                  }
                >
                  {row.triggered ? "!" : "✓"} {personaCode("dev", index)}{" "}
                  {peek ? row.name : personaAlias("dev", index)}
                  {"  "}
                  {formatPrice(row.price)} / {formatPrice(row.target)}
                  {"  "}
                  {formatPercent(row.change_percent)}
                  {"\n"}
                </span>
              ))}
              {rows.length > 6 ? `  … 其余 ${rows.length - 6} 项已折叠\n` : ""}
              {"\n  "}
              {above} 达标 · {alerts} 告警 · {rows.length - above - alerts} 观察
              {"\n"}
              {"  Duration  1.42s\n\n"}
              <span className="dv-term-dim">$ git status --short</span>
              {"\n M src/lib/retry.ts"}
            </pre>
          </div>
        </main>

        <aside className="dv-right">
          <div className="dv-side-head">监控 · 服务健康</div>
          {rows.length === 0 ? (
            <div className="dv-test">等待下一轮采集…</div>
          ) : (
            rows.map((row, index) => {
              const status = statusOf(row, "dev");
              return (
                <div className="dv-svc" key={row.key}>
                  <div className="dv-svc-head">
                    <strong>
                      {peek ? row.name : personaAlias("dev", index)}
                    </strong>
                    <span className={`dv-badge ${status.tone}`}>
                      {status.text}
                    </span>
                  </div>
                  <div className="dv-svc-meta">
                    <span>{formatPrice(row.price)}</span>
                    <span>基线 {formatPrice(row.target)}</span>
                    <span className={changeTone(row.change_percent)}>
                      {formatPercent(row.change_percent)}
                    </span>
                  </div>
                </div>
              );
            })
          )}
          <div className="dv-side-head">测试 · retry.ts</div>
          <div className="dv-test ok">✓ 6 通过</div>
          <div className="dv-test">○ 1 跳过</div>
          <div className="dv-side-head">检查</div>
          <div className="dv-test">0 错误 · 2 警告</div>
          <div className="dv-side-head">审阅</div>
          <div className="dv-review">
            <span className="dv-avatar small">LW</span>
            <div>
              <strong>Lena Wu</strong>
              <span>已批准本次变更</span>
            </div>
          </div>
        </aside>
      </div>

      <div className="dv-status">
        <span>
          <GitBranch size={12} /> feat/checkout-retry
        </span>
        <span>0 错误</span>
        <span>2 警告</span>
        <span className="dv-status-grow" />
        <span>UTF-8</span>
        <span>Ln 24, Col 18</span>
        <span>TypeScript 5.9</span>
      </div>
    </div>
  );
}

/* ---------- 全屏伪装：运营 ---------- */

function OpsBoard({ rows, onExit }: { rows: Row[]; onExit: () => void }) {
  const clock = useClock();
  const peek = usePeek();
  const above = rows.filter((row) => row.above).length;
  const alerts = rows.filter((row) => row.triggered).length;
  const changes = rows
    .map((row) => row.change_percent)
    .filter((value): value is number => value != null);
  const avgChange = changes.length
    ? changes.reduce((sum, value) => sum + value, 0) / changes.length
    : null;
  const totals = pnlTotals(rows);
  const totalText =
    totals.size === 0
      ? "—"
      : [...totals.entries()]
          .map(([currency, value]) => `${formatSigned(value)} ${currency}`)
          .join(" / ");
  const metrics = [
    {
      label: "渠道",
      value: String(rows.length),
      delta: "在监测",
      tone: "pos",
    },
    {
      label: "达标",
      value: String(above),
      delta: rows.length ? `${Math.round((above / rows.length) * 100)}%` : "—",
      tone: "pos",
    },
    {
      label: "异动",
      value: String(alerts),
      delta: alerts > 0 ? "需关注" : "无异常",
      tone: alerts > 0 ? "neg" : "pos",
    },
    {
      label: "平均波动",
      value: formatPercent(avgChange),
      delta: "本期",
      tone: avgChange != null && avgChange < 0 ? "neg" : "pos",
    },
  ];

  return (
    <div className="dg-app op-shell">
      <header className="op-top">
        <SecretExit onExit={onExit} className="op-brand">
          <span className="op-logo">增</span>
          <span className="op-brand-text">
            <strong>增长罗盘</strong>
            <span>Growth Console</span>
          </span>
        </SecretExit>
        <nav className="op-nav">
          {["总览", "渠道", "活动", "用户", "报表"].map((item, index) => (
            <span key={item} className={index === 0 ? "active" : ""}>
              {item}
            </span>
          ))}
        </nav>
        <div className="op-top-right">
          <span className="op-range">10.01 – 11.03</span>
          <Bell size={16} />
          <span className="op-avatar">林</span>
        </div>
      </header>

      <main className="op-main">
        <div className="op-headline">
          <div>
            <h1>渠道总览</h1>
            <p>数据更新于 {clock} · 口径：自然月 · 对比上期</p>
          </div>
          <div className="op-actions">
            <span className="op-btn">导出报表</span>
            <span className="op-btn primary">新建活动</span>
          </div>
        </div>

        <section className="op-metrics">
          {metrics.map((metric) => (
            <div className="op-metric" key={metric.label}>
              <span>{metric.label}</span>
              <strong>{metric.value}</strong>
              <em className={metric.tone}>{metric.delta}</em>
            </div>
          ))}
        </section>

        <section className="op-grid">
          <div className="op-panel op-chart">
            <div className="op-panel-head">
              <div>
                <h2>成交趋势</h2>
                <p>近 30 天成交额</p>
              </div>
              <div className="op-legend">
                <span className="active">成交额</span>
                <span>订单量</span>
              </div>
            </div>
            <svg viewBox="0 0 720 220" className="op-svg" preserveAspectRatio="none">
              <defs>
                <linearGradient id="op-area" x1="0" y1="0" x2="0" y2="1">
                  <stop offset="0%" stopColor="#f59e0b" stopOpacity="0.26" />
                  <stop offset="100%" stopColor="#f59e0b" stopOpacity="0" />
                </linearGradient>
              </defs>
              {[40, 80, 120, 160].map((y) => (
                <line
                  key={y}
                  x1="0"
                  y1={y}
                  x2="720"
                  y2={y}
                  stroke="#e8ecf2"
                  strokeWidth="1"
                />
              ))}
              <path
                d="M0 168 C40 150, 70 158, 110 140 S180 120, 220 132 S300 90, 350 104 S430 70, 490 88 S570 52, 630 66 S690 40, 720 48 L720 220 L0 220 Z"
                fill="url(#op-area)"
              />
              <path
                d="M0 168 C40 150, 70 158, 110 140 S180 120, 220 132 S300 90, 350 104 S430 70, 490 88 S570 52, 630 66 S690 40, 720 48"
                fill="none"
                stroke="#f59e0b"
                strokeWidth="2.5"
              />
              <circle cx="630" cy="66" r="4" fill="#fff" stroke="#f59e0b" strokeWidth="2" />
            </svg>
            <div className="op-axis">
              <span>10.01</span>
              <span>10.10</span>
              <span>10.20</span>
              <span>10.31</span>
              <span>11.03</span>
            </div>
          </div>

          <div className="op-panel op-todo">
            <div className="op-panel-head">
              <div>
                <h2>待处理</h2>
                <p>4 项需要今天跟进</p>
              </div>
              <ListChecks size={17} />
            </div>
            <ul>
              <li>
                <i />
                <div>
                  <strong>信息流素材审核</strong>
                  <span>今天 10:30 · 市场组</span>
                </div>
              </li>
              <li>
                <i />
                <div>
                  <strong>双十一会场排期确认</strong>
                  <span>今天 14:00 · 活动组</span>
                </div>
              </li>
              <li>
                <i />
                <div>
                  <strong>渠道周报数据补齐</strong>
                  <span>明天 11:00 · 数据组</span>
                </div>
              </li>
              <li>
                <i className="done" />
                <div>
                  <strong>私域社群复盘</strong>
                  <span>已完成 · 昨天</span>
                </div>
              </li>
            </ul>
          </div>
        </section>

        <section className="op-panel op-table-panel">
          <div className="op-panel-head">
            <div>
              <h2>渠道明细</h2>
              <p>累计偏差 {totalText}</p>
            </div>
            <span className="op-link">查看全部</span>
          </div>
          <div className="op-table">
            <div className="op-tr op-th">
              <span>渠道</span>
              <span>当前值</span>
              <span>目标值</span>
              <span>波动</span>
              <span>偏差</span>
              <span>状态</span>
            </div>
            {rows.length === 0 ? (
              <div className="op-empty">暂无渠道数据，等待下一轮同步。</div>
            ) : (
              rows.map((row, index) => {
                const status = statusOf(row, "ops");
                return (
                  <div className="op-tr" key={row.key}>
                    <span>{peek ? row.name : personaAlias("ops", index)}</span>
                    <span>{formatPrice(row.price)}</span>
                    <span>{formatPrice(row.target)}</span>
                    <span className={changeTone(row.change_percent)}>
                      {formatPercent(row.change_percent)}
                    </span>
                    <span className={changeTone(row.pnl)}>
                      {row.pnl == null ? "—" : formatSigned(row.pnl)}
                    </span>
                    <span className={`op-tag ${status.tone}`}>{status.text}</span>
                  </div>
                );
              })
            )}
          </div>
        </section>
      </main>
    </div>
  );
}

/* ---------- 全屏伪装：产品 ---------- */

const PRODUCT_OWNERS = ["林雨", "陈默", "周岚"];

function ProductDoc({ rows, onExit }: { rows: Row[]; onExit: () => void }) {
  const clock = useClock();
  const peek = usePeek();
  return (
    <div className="dg-app pd-shell">
      <header className="pd-top">
        <SecretExit onExit={onExit} className="pd-brand">
          <span className="pd-logo">需</span>
          <strong>需求雷达</strong>
        </SecretExit>
        <nav className="pd-nav">
          {["需求池", "迭代", "文档", "评审"].map((item, index) => (
            <span key={item} className={index === 2 ? "active" : ""}>
              {item}
            </span>
          ))}
        </nav>
        <div className="pd-top-right">
          <Share2 size={16} />
          <Star size={16} />
          <span className="pd-avatar">陈</span>
        </div>
      </header>

      <div className="pd-body">
        <aside className="pd-side">
          <div className="pd-side-title">需求池 · Q4</div>
          <div className="pd-side-item active">全部需求 <em>18</em></div>
          <div className="pd-side-item">进行中 <em>6</em></div>
          <div className="pd-side-item">待评审 <em>4</em></div>
          <div className="pd-side-item">已上线 <em>8</em></div>
          <div className="pd-side-title">我的项目</div>
          <div className="pd-side-item">增长实验</div>
          <div className="pd-side-item">会员体系</div>
          <div className="pd-side-item">数据看板</div>
          <div className="pd-side-title">快捷入口</div>
          <div className="pd-side-item">
            <FileText size={13} /> 文档模板
          </div>
          <div className="pd-side-item">
            <ListChecks size={13} /> 评审清单
          </div>
        </aside>

        <main className="pd-doc">
          <div className="pd-doc-head">
            <span className="pd-tag">进行中</span>
            <span className="pd-updated">更新于 {clock} · 陈默</span>
          </div>
          <h1>Q4 增长实验方案</h1>
          <p className="pd-lead">
            围绕新客转化与复购两条主线，拆分为 3 组对照实验，统一在 11 月完成首轮验证。
          </p>

          <h2>1. 背景</h2>
          <p>
            上季度新客次周留存为 28.4%，低于目标 3.6 个百分点；结算链路存在两处明显流失点，
            需要在本季度内完成验证与收敛。
          </p>

          <h2>2. 目标</h2>
          <table className="pd-table">
            <thead>
              <tr>
                <th>需求</th>
                <th>当前值</th>
                <th>计划 / 阈值</th>
                <th>进度</th>
                <th>负责人</th>
                <th>状态</th>
              </tr>
            </thead>
            <tbody>
              {rows.length === 0 ? (
                <tr>
                  <td colSpan={6}>暂无需求数据，等待下一轮同步。</td>
                </tr>
              ) : (
                rows.map((row, index) => {
                  const status = statusOf(row, "product");
                  return (
                    <tr key={row.key}>
                      <td>{peek ? row.name : personaAlias("product", index)}</td>
                      <td>{formatPrice(row.price)}</td>
                      <td>{formatPrice(row.target)}</td>
                      <td className={changeTone(row.change_percent)}>
                        {formatPercent(row.change_percent)}
                      </td>
                      <td>{PRODUCT_OWNERS[index % PRODUCT_OWNERS.length]}</td>
                      <td>{status.text}</td>
                    </tr>
                  );
                })
              )}
            </tbody>
          </table>

          <h2>3. 实验分组</h2>
          <ul>
            <li>
              <strong>实验 A</strong>：结算页默认勾选优惠券，观察支付完成率变化。
            </li>
            <li>
              <strong>实验 B</strong>：新客引导由 3 步压缩为 2 步，观察次周留存。
            </li>
            <li>
              <strong>实验 C</strong>：复购提醒改为站内信 + 短信组合触达。
            </li>
          </ul>

          <div className="pd-callout">
            <strong>评审结论</strong>
            <p>实验 A 与 B 优先级最高，本周内完成埋点验收；实验 C 等待短信模板审批。</p>
          </div>

          <h2>4. 节奏</h2>
          <p>
            11 月 08 日完成开发与灰度，11 月 12 日全量上线，11 月 20 日输出首轮复盘。
            所有实验统一使用增长数据仓口径，避免与周报数据产生偏差。
          </p>
        </main>

        <aside className="pd-outline">
          <div className="pd-outline-title">目录</div>
          <div className="pd-outline-item active">背景</div>
          <div className="pd-outline-item">目标</div>
          <div className="pd-outline-item">实验分组</div>
          <div className="pd-outline-item">节奏</div>
          <div className="pd-outline-title">评论 · 2</div>
          <div className="pd-comment">
            <span className="pd-comment-avatar">林</span>
            <div>
              <strong>林雨</strong>
              <p>实验 B 的引导文案今天可以给到。</p>
            </div>
          </div>
          <div className="pd-comment">
            <span className="pd-comment-avatar">周</span>
            <div>
              <strong>周岚</strong>
              <p>短信预算已确认，按原计划推进。</p>
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}

/* ---------- 全屏伪装：财务 ---------- */

const FINANCE_COLUMNS = ["A", "B", "C", "D", "E", "F"];

function FinanceSheet({ rows, onExit }: { rows: Row[]; onExit: () => void }) {
  const peek = usePeek();
  const currencies = new Set(rows.map((row) => row.currency));
  const singleCurrency = rows.length > 0 && currencies.size === 1;
  const totalBudget = rows.reduce((sum, row) => sum + row.target, 0);
  const totalActual = rows.reduce((sum, row) => sum + row.price, 0);
  const totalRatio =
    totalBudget > 0 ? `${((totalActual / totalBudget) * 100).toFixed(1)}%` : "—";
  const overspent = rows.filter((row) => row.triggered).length;
  const avgRatio = rows.length
    ? `${(
        (rows.reduce(
          (sum, row) => sum + (row.target > 0 ? row.price / row.target : 1),
          0,
        ) /
          rows.length) *
        100
      ).toFixed(1)}%`
    : "—";

  return (
    <div className="dg-app fn-shell">
      <header className="fn-top">
        <SecretExit onExit={onExit} className="fn-brand">
          <span className="fn-logo">
            <Table2 size={16} />
          </span>
          <strong>费用中心</strong>
          <span className="fn-file">预算执行表 · 2026 Q4</span>
        </SecretExit>
        <div className="fn-menu">
          <span>文件</span>
          <span>编辑</span>
          <span>插入</span>
          <span>格式</span>
          <span>数据</span>
          <span>工具</span>
        </div>
        <div className="fn-top-right">
          <span className="fn-collab">3 人协作</span>
          <span className="fn-avatar">周</span>
        </div>
      </header>

      <div className="fn-toolbar">
        <span className="fn-tool">撤销</span>
        <span className="fn-tool">重做</span>
        <i />
        <span className="fn-tool">¥</span>
        <span className="fn-tool">%</span>
        <span className="fn-tool">自动求和</span>
        <i />
        <span className="fn-tool">筛选</span>
        <span className="fn-tool">冻结首行</span>
        <span className="fn-saved">已保存</span>
      </div>

      <div className="fn-formula">
        <span className="fn-fx">fx</span>
        <div>=SUM(C2:C{rows.length + 1})</div>
      </div>

      <div className="fn-sheet-wrap">
        <table className="fn-sheet">
          <thead>
            <tr>
              <th className="fn-corner" />
              {FINANCE_COLUMNS.map((column) => (
                <th key={column}>{column}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.length === 0 ? (
              <tr>
                <th>1</th>
                <td colSpan={6}>暂无科目数据，等待下一轮同步。</td>
              </tr>
            ) : (
              rows.map((row, index) => {
                const status = statusOf(row, "finance");
                const ratio =
                  row.target > 0
                    ? `${((row.price / row.target) * 100).toFixed(1)}%`
                    : "—";
                return (
                  <tr key={row.key}>
                    <th>{index + 1}</th>
                    <td>{peek ? row.name : personaAlias("finance", index)}</td>
                    <td className="fn-num">{formatAmount(row.target)}</td>
                    <td className="fn-num">{formatAmount(row.price)}</td>
                    <td className="fn-num">{ratio}</td>
                    <td className={`fn-num ${changeTone(row.change_percent)}`}>
                      {formatPercent(row.change_percent)}
                    </td>
                    <td className={row.triggered ? "fn-bad" : "fn-good"}>
                      {status.text}
                    </td>
                  </tr>
                );
              })
            )}
            {singleCurrency ? (
              <tr className="fn-total">
                <th>合计</th>
                <td>—</td>
                <td className="fn-num">{formatAmount(totalBudget)}</td>
                <td className="fn-num">{formatAmount(totalActual)}</td>
                <td className="fn-num">{totalRatio}</td>
                <td className="fn-num">—</td>
                <td className={overspent > 0 ? "fn-bad" : "fn-good"}>
                  {overspent > 0 ? `${overspent} 项超支` : "全部正常"}
                </td>
              </tr>
            ) : null}
          </tbody>
        </table>
      </div>

      <div className="fn-tabs">
        <span className="active">预算总表</span>
        <span>人力成本</span>
        <span>市场费用</span>
        <span>差旅</span>
        <span className="fn-add">+</span>
      </div>
      <div className="fn-status">
        <span>就绪</span>
        <span>平均执行率 {avgRatio}</span>
        <span>{overspent} 项超支</span>
        <span className="fn-status-grow" />
        <span>视图 100%</span>
        <span>中文（简体）</span>
      </div>
    </div>
  );
}

/* ---------- shared ---------- */

function statusOf(
  row: Row,
  persona: DisguisePersonaId,
): { text: string; tone: StatusTone } {
  const meta = DISGUISE_PERSONAS[persona];
  if (row.triggered) return { text: meta.status.triggered, tone: "warn" };
  if (row.above) return { text: meta.status.above, tone: "ok" };
  return { text: meta.status.below, tone: "dim" };
}

function pnlTotals(rows: Row[]): Map<string, number> {
  const totals = new Map<string, number>();
  for (const row of rows) {
    if (row.pnl != null) {
      totals.set(row.currency, (totals.get(row.currency) ?? 0) + row.pnl);
    }
  }
  return totals;
}

function changeTone(value: number | null): "pos" | "neg" | "dim" {
  if (value == null) return "dim";
  return value >= 0 ? "pos" : "neg";
}

function formatAmount(value: number): string {
  return value.toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

/** 按住 Alt / Option 时临时把伪装名称换回真实资产名。 */
function usePeek(): boolean {
  const [peek, setPeek] = useState(false);

  useEffect(() => {
    function down(event: KeyboardEvent) {
      if (event.key === "Alt") setPeek(true);
    }
    function up(event: KeyboardEvent) {
      if (event.key === "Alt") setPeek(false);
    }
    function reset() {
      setPeek(false);
    }
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", reset);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", reset);
    };
  }, []);

  return peek;
}

function SecretExit({
  onExit,
  className = "",
  children,
}: {
  onExit: () => void;
  className?: string;
  children: ReactNode;
}) {
  const clicks = useRef(0);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  function handleClick() {
    clicks.current += 1;
    if (clicks.current >= 3) {
      clicks.current = 0;
      window.clearTimeout(timer.current);
      onExit();
      return;
    }
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      clicks.current = 0;
    }, 900);
  }

  return (
    <button type="button" className={`dg-secret ${className}`} onClick={handleClick}>
      {children}
    </button>
  );
}

function useClock(): string {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(new Date()), 15000);
    return () => window.clearInterval(timer);
  }, []);
  return now.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  });
}
