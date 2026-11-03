import { useEffect, useState } from "react";
import { RefreshCw, Search } from "lucide-react";
import type { AssetDraft } from "../drafts";
import {
  ASSET_KINDS,
  WEEKDAY_LABELS,
  assetDerivedCost,
  assetDerivedTarget,
  assetDraftError,
  assetPrimaryCode,
  kindLabel,
  sipScheduleLabel,
} from "../drafts";
import { api } from "../api";
import { formatPrice } from "../format";
import type { AssetKind, AssetSearchItem, SipFrequency } from "../types";
import {
  Field,
  Modal,
  NumberField,
  Segmented,
  Select,
  Tag,
  TextInput,
} from "./ui";

export function AssetModal({
  initial,
  index,
  globalThreshold,
  onSave,
  onClose,
}: {
  initial: AssetDraft;
  index: number | null;
  globalThreshold: number;
  onSave: (draft: AssetDraft) => void | Promise<void>;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState<AssetDraft>({ ...initial });
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<AssetSearchItem[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [searchOpen, setSearchOpen] = useState(false);

  useEffect(() => {
    const keyword = query.trim();
    if (!keyword) {
      setResults([]);
      setSearchError(null);
      setSearching(false);
      setSearchOpen(false);
      return;
    }

    let cancelled = false;
    const timer = window.setTimeout(() => {
      setSearching(true);
      setSearchError(null);
      api
        .searchAssets(keyword)
        .then((items) => {
          if (cancelled) return;
          setResults(items);
          setSearchOpen(true);
        })
        .catch((reason: unknown) => {
          if (cancelled) return;
          setResults([]);
          setSearchError(errorMessage(reason));
          setSearchOpen(true);
        })
        .finally(() => {
          if (!cancelled) setSearching(false);
        });
    }, 400);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [query]);

  function set<K extends keyof AssetDraft>(key: K, value: AssetDraft[K]) {
    setDraft((current) => ({ ...current, [key]: value }));
  }

  function pick(item: AssetSearchItem) {
    setDraft((current) => ({
      ...current,
      kind: item.kind,
      name: item.name,
      symbol: item.kind === "fund" ? "" : item.symbol || item.code,
      code: item.kind === "fund" ? item.code || item.symbol || "" : "",
      secid: item.secid ?? "",
    }));
    setQuery(item.name);
    setResults([]);
    setSearchOpen(false);
    setError(null);
  }

  function changeKind(kind: AssetKind) {
    const code = assetPrimaryCode(draft);
    setDraft((current) => ({
      ...current,
      kind,
      symbol: kind === "fund" ? "" : code,
      code: kind === "fund" ? code : "",
    }));
  }

  function changeSipFrequency(frequency: SipFrequency) {
    setDraft((current) => ({
      ...current,
      sipFrequency: frequency,
      // 每周用 1-7，切换过去时如果原来的日期超出范围就重置为周一。
      sipDay:
        frequency === "weekly" &&
        !["1", "2", "3", "4", "5", "6", "7"].includes(current.sipDay.trim())
          ? "1"
          : current.sipDay,
    }));
  }

  async function submit() {
    const problem = assetDraftError(draft);
    if (problem) {
      setError(problem);
      return;
    }
    setSubmitting(true);
    try {
      await onSave(draft);
    } finally {
      setSubmitting(false);
    }
  }

  const cost = assetDerivedCost(draft);
  const target = assetDerivedTarget(draft, globalThreshold);
  const codeLabel =
    draft.kind === "fund" ? "基金代码" : draft.kind === "gold" ? "黄金代码" : "股票代码";
  const codePlaceholder =
    draft.kind === "fund"
      ? "161725 / 518880"
      : draft.kind === "gold"
        ? "AU9999 / nf_AU0 / hf_XAU"
        : "sh600000 / 00700.HK / AAPL";

  return (
    <Modal
      title={index == null ? "添加资产" : "编辑资产"}
      subtitle="成本价或本金会自动换算成触发线，价格越过触发线时提醒。"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn ghost" onClick={onClose}>
            取消
          </button>
          <button
            type="button"
            className="btn primary"
            onClick={submit}
            disabled={submitting}
          >
            {submitting ? "保存中…" : index == null ? "添加" : "保存"}
          </button>
        </>
      }
    >
      <div className="form-stack">
        <Field label="类型">
          <Segmented
            value={draft.kind}
            options={ASSET_KINDS.map((item) => ({ value: item.value, label: item.label }))}
            onChange={changeKind}
          />
        </Field>

        <div className="field">
          <span className="field-label">搜索资产</span>
          <div className="asset-search">
            <Search size={15} className="asset-search-icon" />
            <input
              className="input asset-search-field"
              value={query}
              placeholder="例如：浦发银行 / 600000 / 161725 / AU9999"
              onChange={(event) => {
                setQuery(event.target.value);
                setSearchOpen(true);
              }}
              onFocus={() => {
                if (results.length > 0 || searchError) setSearchOpen(true);
              }}
              onBlur={() => window.setTimeout(() => setSearchOpen(false), 120)}
              onKeyDown={(event) => {
                if (event.key === "Escape") {
                  setSearchOpen(false);
                } else if (event.key === "Enter" && results.length > 0) {
                  event.preventDefault();
                  pick(results[0]);
                }
              }}
            />
            {searching ? (
              <RefreshCw size={14} className="asset-search-spinner spin" />
            ) : null}
            {searchOpen && query.trim() ? (
              <div className="asset-search-menu">
                {searchError ? (
                  <div className="asset-search-empty error">{searchError}</div>
                ) : results.length === 0 ? (
                  <div className="asset-search-empty">
                    {searching ? "搜索中…" : "没有找到匹配的股票、基金或黄金"}
                  </div>
                ) : (
                  results.map((item, position) => (
                    <button
                      type="button"
                      key={`${item.kind}-${item.secid ?? item.code}-${position}`}
                      className="asset-search-item"
                      onMouseDown={(event) => {
                        event.preventDefault();
                        pick(item);
                      }}
                    >
                      <span className="asset-search-name">
                        <strong>{item.name}</strong>
                        <span>
                          {item.code}
                          {item.market ? ` · ${item.market}` : ""}
                        </span>
                      </span>
                      <Tag>{kindLabel(item.kind)}</Tag>
                    </button>
                  ))
                )}
              </div>
            ) : null}
          </div>
          <span className="field-hint">
            输入名称或代码，选中结果会自动填入类型、名称和代码
          </span>
        </div>

        <div className="form-grid two">
          <Field label="名称" hint="留空时使用行情接口返回的名称">
            <TextInput
              value={draft.name}
              placeholder="例如：浦发银行"
              onChange={(event) => set("name", event.target.value)}
            />
          </Field>
          <Field label={codeLabel}>
            <TextInput
              value={
                draft.kind === "fund"
                  ? draft.code || draft.symbol
                  : draft.symbol || draft.code
              }
              placeholder={codePlaceholder}
              onChange={(event) =>
                set(draft.kind === "fund" ? "code" : "symbol", event.target.value)
              }
            />
          </Field>
        </div>

        <Field label="secid（可选）" hint="东方财富代码，例如 1.600000、116.00700、118.AU9999；填了会优先生效">
          <TextInput
            value={draft.secid}
            placeholder="1.600000"
            onChange={(event) => set("secid", event.target.value)}
          />
        </Field>

        <div className="form-grid two">
          <Field label="成本价" hint="优先使用；留空则用本金 ÷ 数量">
            <NumberField
              value={draft.cost}
              placeholder="10.00"
              onChange={(value) => set("cost", value)}
            />
          </Field>
          <Field label="持有数量" hint={draft.kind === "gold" ? "克数" : "股数 / 份额"}>
            <NumberField
              value={draft.quantity}
              placeholder="1000"
              onChange={(value) => set("quantity", value)}
            />
          </Field>
        </div>

        <Field label="本金（可选）" hint="和持有数量一起填写时会自动计算成本价">
          <NumberField
            value={draft.principal}
            placeholder="10000"
            onChange={(value) => set("principal", value)}
          />
        </Field>

        <div className="form-grid two">
          <Field label="单独阈值 %（可选）" hint="覆盖全局阈值，0 表示回本即提醒">
            <NumberField
              value={draft.threshold}
              placeholder="0"
              onChange={(value) => set("threshold", value)}
            />
          </Field>
          <Field label="触发价（可选）" hint="优先级最高，填了就不看成本价">
            <NumberField
              value={draft.target}
              placeholder="11.00"
              onChange={(value) => set("target", value)}
            />
          </Field>
        </div>

        <div className="sip-block">
          <label className="sip-toggle">
            <input
              type="checkbox"
              checked={draft.sipEnabled}
              onChange={(event) => set("sipEnabled", event.target.checked)}
            />
            <span>
              <strong>定投提醒</strong>
              <em>
                股票、基金、黄金都支持；到定投日会通过通知渠道提醒你，不会自动下单
              </em>
            </span>
          </label>

          {draft.sipEnabled ? (
            <div className="form-grid two sip-fields">
              <Field label="每期金额" hint="按当前价估算可买数量">
                <NumberField
                  value={draft.sipAmount}
                  placeholder="500"
                  onChange={(value) => set("sipAmount", value)}
                />
              </Field>
              <Field label="周期">
                <Segmented
                  value={draft.sipFrequency}
                  options={[
                    { value: "monthly", label: "每月" },
                    { value: "weekly", label: "每周" },
                  ]}
                  onChange={changeSipFrequency}
                />
              </Field>
              <Field
                label={draft.sipFrequency === "weekly" ? "星期" : "每月几号"}
                hint={
                  draft.sipFrequency === "monthly"
                    ? "超过当月天数时按当月最后一天"
                    : undefined
                }
              >
                {draft.sipFrequency === "weekly" ? (
                  <Select
                    value={draft.sipDay}
                    onChange={(event) => set("sipDay", event.target.value)}
                  >
                    {WEEKDAY_LABELS.map((label, index) => (
                      <option key={label} value={String(index + 1)}>
                        {label}
                      </option>
                    ))}
                  </Select>
                ) : (
                  <NumberField
                    value={draft.sipDay}
                    placeholder="1"
                    onChange={(value) => set("sipDay", value)}
                  />
                )}
              </Field>
              <Field label="提醒时间" hint="到点后的下一轮检查会提醒">
                <TextInput
                  type="time"
                  value={draft.sipTime}
                  onChange={(event) => set("sipTime", event.target.value)}
                />
              </Field>
            </div>
          ) : null}

          {draft.sipEnabled ? (
            <div className="sip-summary">
              定投计划：{sipScheduleLabel(draft)}
              {draft.sipAmount.trim()
                ? ` · 每期 ${formatPrice(Number(draft.sipAmount) || 0)}`
                : ""}
            </div>
          ) : null}
        </div>

        {cost != null || target != null ? (
          <div className="derive-box">
            <span>成本价</span>
            <strong className="mono">{formatPrice(cost)}</strong>
            <span>触发价</span>
            <strong className="mono accent">{formatPrice(target)}</strong>
          </div>
        ) : null}

        {error ? <div className="inline-error">{error}</div> : null}
      </div>
    </Modal>
  );
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
