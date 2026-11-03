import { useState } from "react";
import type { AssetDraft } from "../drafts";
import {
  ASSET_KINDS,
  assetDerivedCost,
  assetDerivedTarget,
  assetDraftError,
  assetPrimaryCode,
} from "../drafts";
import { formatPrice } from "../format";
import type { AssetKind } from "../types";
import { Field, Modal, NumberField, Segmented, TextInput } from "./ui";

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
  onSave: (draft: AssetDraft) => void;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState<AssetDraft>({ ...initial });
  const [error, setError] = useState<string | null>(null);

  function set<K extends keyof AssetDraft>(key: K, value: AssetDraft[K]) {
    setDraft((current) => ({ ...current, [key]: value }));
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

  function submit() {
    const problem = assetDraftError(draft);
    if (problem) {
      setError(problem);
      return;
    }
    onSave(draft);
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
          <button type="button" className="btn primary" onClick={submit}>
            {index == null ? "添加" : "保存"}
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
