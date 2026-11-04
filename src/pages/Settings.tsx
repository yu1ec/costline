import { useEffect, useState } from "react";
import {
  AlertCircle,
  Bell,
  CheckCircle2,
  FolderOpen,
  Pencil,
  Plus,
  RotateCcw,
  Send,
  Trash2,
  Wallet,
} from "lucide-react";
import type { Config, Snapshot } from "../types";
import type { DisguiseMode, DisguisePersonaId } from "../disguise";
import type { AssetDraft, ChannelDraft, GlobalDraft } from "../drafts";
import {
  assetDraftError,
  assetDraftKey,
  assetPrimaryCode,
  assetToDraft,
  assetDerivedCost,
  assetDerivedTarget,
  channelDraftError,
  channelLabel,
  channelSummary,
  channelToDraft,
  configToDraft,
  draftToChannel,
  draftToConfig,
  emptyAssetDraft,
  emptyChannelDraft,
  kindLabel,
  parseNumber,
  sipScheduleLabel,
} from "../drafts";
import { formatAmount, formatPrice } from "../format";
import { api } from "../api";
import type { UpdateController } from "../updater";
import { AssetModal } from "../components/AssetModal";
import { ChannelModal } from "../components/ChannelModal";
import { DisguiseSection } from "../components/DisguiseSection";
import { UpdateSection } from "../components/UpdateSection";
import { Field, NumberField, Tag, TextInput } from "../components/ui";

type Toast = (text: string, tone?: "ok" | "error") => void;

export function Settings({
  config,
  snapshot,
  updates,
  disguisePersona,
  disguiseAuto,
  onDisguisePersona,
  onDisguiseAuto,
  onEnterDisguise,
  onSaved,
  onSnapshot,
  onToast,
}: {
  config: Config;
  snapshot: Snapshot | null;
  updates: UpdateController;
  disguisePersona: DisguisePersonaId;
  disguiseAuto: boolean;
  onDisguisePersona: (persona: DisguisePersonaId) => void;
  onDisguiseAuto: (auto: boolean) => void;
  onEnterDisguise: (mode: Exclude<DisguiseMode, "off">) => void;
  onSaved: (config: Config, snapshot: Snapshot) => void;
  onSnapshot: (snapshot: Snapshot) => void;
  onToast: Toast;
}) {
  const [global, setGlobal] = useState<GlobalDraft>(
    () => configToDraft(config).global,
  );
  const [assets, setAssets] = useState<AssetDraft[]>(() =>
    config.assets.map(assetToDraft),
  );
  const [channels, setChannels] = useState<ChannelDraft[]>(() =>
    config.notify.channels.map(channelToDraft),
  );
  const [dirty, setDirty] = useState(false);
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResults, setTestResults] = useState<
    { label: string; ok: boolean; error: string | null }[] | null
  >(null);
  const [assetModal, setAssetModal] = useState<{
    index: number | null;
    draft: AssetDraft;
  } | null>(null);
  const [channelModal, setChannelModal] = useState<{
    index: number | null;
    draft: ChannelDraft;
  } | null>(null);

  useEffect(() => {
    const next = configToDraft(config);
    // 资产和渠道会自动保存；基础参数没保存时不要被配置刷新覆盖。
    setGlobal((current) => (dirty ? current : next.global));
    setAssets(next.assets);
    setChannels(next.channels);
  }, [config, dirty]);

  function updateGlobal<K extends keyof GlobalDraft>(key: K, value: GlobalDraft[K]) {
    setGlobal((current) => ({ ...current, [key]: value }));
    setDirty(true);
  }

  function buildConfig(): Config {
    return draftToConfig({ global, assets, channels });
  }

  function validateAll(): string | null {
    for (let index = 0; index < assets.length; index += 1) {
      const problem = assetDraftError(assets[index]);
      if (problem) {
        const name = assets[index].name.trim() || `第 ${index + 1} 个资产`;
        return `${name}：${problem}`;
      }
    }
    for (let index = 0; index < channels.length; index += 1) {
      const problem = channelDraftError(channels[index]);
      if (problem) {
        return `${channelLabel(channels[index].type)}：${problem}`;
      }
    }
    return null;
  }

  async function save() {
    const problem = validateAll();
    if (problem) {
      onToast(problem, "error");
      return;
    }
    setSaving(true);
    try {
      const payload = buildConfig();
      const nextSnapshot = await api.saveConfig(payload);
      onSaved(payload, nextSnapshot);
      setDirty(false);
      onToast("设置已保存", "ok");
    } catch (error) {
      onToast(errorMessage(error), "error");
    } finally {
      setSaving(false);
    }
  }

  async function runTest() {
    const problem = validateAll();
    if (problem) {
      onToast(problem, "error");
      return;
    }
    setTesting(true);
    setTestResults(null);
    try {
      const results = await api.testNotify(buildConfig());
      setTestResults(results);
      const success = results.filter((result) => result.ok).length;
      onToast(
        success === results.length
          ? `测试通知全部成功（${success} 个渠道）`
          : `测试通知：${success}/${results.length} 个渠道成功`,
        success > 0 ? "ok" : "error",
      );
    } catch (error) {
      onToast(errorMessage(error), "error");
    } finally {
      setTesting(false);
    }
  }

  async function resetState() {
    if (
      !window.confirm(
        "确定重置提醒状态吗？重置后，已经提醒过的资产在再次越过触发线时会重新提醒。",
      )
    ) {
      return;
    }
    try {
      const nextSnapshot = await api.resetState();
      onSnapshot(nextSnapshot);
      onToast("提醒状态已重置", "ok");
    } catch (error) {
      onToast(errorMessage(error), "error");
    }
  }

  /// 资产 / 渠道改动后立即写盘；基础参数仍走底部「保存」按钮。
  async function autoSave(
    nextAssets: AssetDraft[],
    nextChannels: ChannelDraft[],
    message: string,
  ): Promise<boolean> {
    const payload = draftToConfig({
      global: configToDraft(config).global,
      assets: nextAssets,
      channels: nextChannels,
    });
    try {
      const nextSnapshot = await api.saveConfig(payload);
      setAssets(nextAssets);
      setChannels(nextChannels);
      onSaved(payload, nextSnapshot);
      onToast(message, "ok");
      return true;
    } catch (error) {
      onToast(errorMessage(error), "error");
      return false;
    }
  }

  async function saveAsset(draft: AssetDraft) {
    const index = assetModal?.index ?? null;
    const next =
      index == null
        ? [...assets, draft]
        : assets.map((item, position) => (position === index ? draft : item));
    const ok = await autoSave(
      next,
      channels,
      index == null ? "资产已添加并生效" : "资产已保存并生效",
    );
    if (ok) setAssetModal(null);
  }

  async function removeAsset(index: number) {
    const name =
      assets[index].name.trim() || assetPrimaryCode(assets[index]) || "这个资产";
    if (!window.confirm(`确定删除「${name}」吗？`)) return;
    await autoSave(
      assets.filter((_, position) => position !== index),
      channels,
      "资产已删除",
    );
  }

  async function saveChannel(draft: ChannelDraft) {
    const index = channelModal?.index ?? null;
    const next =
      index == null
        ? [...channels, draft]
        : channels.map((item, position) => (position === index ? draft : item));
    const ok = await autoSave(
      assets,
      next,
      index == null ? "通知渠道已添加并生效" : "通知渠道已保存并生效",
    );
    if (ok) setChannelModal(null);
  }

  async function removeChannel(index: number) {
    if (!window.confirm(`确定删除「${channelLabel(channels[index].type)}」渠道吗？`)) {
      return;
    }
    await autoSave(
      assets,
      channels.filter((_, position) => position !== index),
      "通知渠道已删除",
    );
  }

  const globalThreshold = Number(global.threshold) || 0;
  const configPath = snapshot?.config_path ?? "";
  const currencyByKey = new Map<string, string>();
  for (const row of snapshot?.rows ?? []) {
    currencyByKey.set(row.key, row.currency);
  }

  return (
    <div className="page settings-page">
      <section className="panel">
        <div className="panel-head">
          <div>
            <h2>基础参数</h2>
            <p>控制轮询频率、回本阈值与通知节奏。</p>
          </div>
        </div>
        <div className="form-grid three">
          <Field label="轮询间隔（秒）" hint="免费行情接口建议 ≥ 30 秒">
            <NumberField
              value={global.interval}
              placeholder="60"
              onChange={(value) => updateGlobal("interval", value)}
            />
          </Field>
          <Field label="请求超时（秒）">
            <NumberField
              value={global.timeout}
              placeholder="10"
              onChange={(value) => updateGlobal("timeout", value)}
            />
          </Field>
          <Field label="全局回本阈值 %" hint="0 = 现价超过成本价即提醒">
            <NumberField
              value={global.threshold}
              placeholder="0"
              onChange={(value) => updateGlobal("threshold", value)}
            />
          </Field>
          <Field label="通知冷却（秒）" hint="避免反复穿越成本线时刷屏">
            <NumberField
              value={global.cooldown}
              placeholder="1800"
              onChange={(value) => updateGlobal("cooldown", value)}
            />
          </Field>
          <Field label="重复提醒（秒）" hint="0 = 只在穿越时提醒一次">
            <NumberField
              value={global.repeat}
              placeholder="0"
              onChange={(value) => updateGlobal("repeat", value)}
            />
          </Field>
          <Field
            label="交割单目录"
            hint="券商导出的 CSV / TXT 交割单放这里，卖出后自动按摊薄成本扣减本金"
            className="span-2"
          >
            <TextInput
              value={global.importDir}
              placeholder="~/Documents/交割单"
              onChange={(event) => updateGlobal("importDir", event.target.value)}
            />
          </Field>
        </div>
      </section>

      <section className="panel">
        <div className="panel-head">
          <div>
            <h2>持仓资产</h2>
            <p>支持股票、场外基金、黄金；成本价也可以用「本金 ÷ 数量」自动算。</p>
          </div>
          <button
            type="button"
            className="btn"
            onClick={() =>
              setAssetModal({ index: null, draft: emptyAssetDraft() })
            }
          >
            <Plus size={15} /> 添加资产
          </button>
        </div>

        {assets.length === 0 ? (
          <div className="list-empty">
            <Wallet size={20} />
            <span>还没有资产，点击右上角添加。</span>
          </div>
        ) : (
          <div className="item-list">
            {assets.map((asset, index) => {
              const code = assetPrimaryCode(asset);
              const cost = assetDerivedCost(asset);
              const target = assetDerivedTarget(asset, globalThreshold);
              return (
                <div className="item-row" key={`${asset.kind}-${index}`}>
                  <div className="item-main">
                    <div className="item-title">
                      <span className="item-name">
                        {asset.name.trim() || code || "未命名资产"}
                      </span>
                      <Tag>{kindLabel(asset.kind)}</Tag>
                    </div>
                    <span className="item-meta">
                      {code || "未填写代码"}
                      <span className="dot">·</span>
                      成本 <span className="mono">{formatPrice(cost)}</span>
                      <span className="dot">·</span>
                      触发{" "}
                      <span className="mono accent">{formatPrice(target)}</span>
                      {asset.sipEnabled ? (
                        <>
                          <span className="dot">·</span>
                          <span className="sip-meta">
                            定投{" "}
                            <span className="mono">
                              {formatAmount(
                                parseNumber(asset.sipAmount) ?? 0,
                                currencyByKey.get(assetDraftKey(asset)) ?? "CNY",
                              )}
                            </span>
                            <span className="dot">·</span>
                            {sipScheduleLabel(asset)}
                          </span>
                        </>
                      ) : null}
                    </span>
                  </div>
                  <div className="item-actions">
                    <button
                      type="button"
                      className="icon-btn"
                      aria-label="编辑"
                      onClick={() => setAssetModal({ index, draft: { ...asset } })}
                    >
                      <Pencil size={15} />
                    </button>
                    <button
                      type="button"
                      className="icon-btn danger"
                      aria-label="删除"
                      onClick={() => removeAsset(index)}
                    >
                      <Trash2 size={15} />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      <section className="panel">
        <div className="panel-head">
          <div>
            <h2>通知渠道</h2>
            <p>
              触发时始终发送系统通知，并在窗口内提示；这里可以再添加远程渠道。
            </p>
          </div>
          <div className="panel-actions">
            <button
              type="button"
              className="btn"
              onClick={runTest}
              disabled={testing}
            >
              <Send size={15} />
              {testing ? "发送中" : "测试通知"}
            </button>
            <button
              type="button"
              className="btn"
              onClick={() =>
                setChannelModal({
                  index: null,
                  draft: emptyChannelDraft("bark"),
                })
              }
            >
              <Plus size={15} /> 添加渠道
            </button>
          </div>
        </div>

        {channels.length === 0 ? (
          <div className="list-empty">
            <Bell size={20} />
            <span>系统通知已默认开启，还可以添加 Bark、Telegram 等远程渠道。</span>
          </div>
        ) : (
          <div className="item-list">
            {channels.map((channel, index) => (
              <div className="item-row" key={`${channel.type}-${index}`}>
                <div className="item-main">
                  <div className="item-title">
                    <span className="item-name">{channelLabel(channel.type)}</span>
                    <Tag tone="accent">已启用</Tag>
                  </div>
                  <span className="item-meta">
                    {channelSummary(draftToChannel(channel))}
                  </span>
                </div>
                <div className="item-actions">
                  <button
                    type="button"
                    className="icon-btn"
                    aria-label="编辑"
                    onClick={() =>
                      setChannelModal({
                        index,
                        draft: {
                          ...channel,
                          headers: channel.headers.map((header) => ({
                            ...header,
                          })),
                        },
                      })
                    }
                  >
                    <Pencil size={15} />
                  </button>
                  <button
                    type="button"
                    className="icon-btn danger"
                    aria-label="删除"
                    onClick={() => removeChannel(index)}
                  >
                    <Trash2 size={15} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}

        {testResults ? (
          <div className="test-results">
            {testResults.map((result, index) => (
              <div
                className={result.ok ? "test-result ok" : "test-result error"}
                key={`${result.label}-${index}`}
              >
                {result.ok ? <CheckCircle2 size={15} /> : <AlertCircle size={15} />}
                <span>
                  {result.label}
                  {result.ok ? " 发送成功" : ` 失败：${result.error}`}
                </span>
              </div>
            ))}
          </div>
        ) : null}
      </section>

      <DisguiseSection
        persona={disguisePersona}
        auto={disguiseAuto}
        rows={snapshot?.rows ?? []}
        onChangePersona={onDisguisePersona}
        onChangeAuto={onDisguiseAuto}
        onEnter={onEnterDisguise}
      />

      <UpdateSection updates={updates} />

      <div className="settings-footer">
        <div className="settings-footer-info">
          <span className={dirty ? "dirty-dot active" : "dirty-dot"} />
          <span>
            {dirty ? "基础参数有未保存的修改" : "基础参数已保存（资产与渠道自动保存）"}
          </span>
          {configPath ? <span className="config-path">{configPath}</span> : null}
        </div>
        <div className="settings-footer-actions">
          <button type="button" className="btn ghost" onClick={() => api.openConfigDir()}>
            <FolderOpen size={15} /> 配置目录
          </button>
          <button type="button" className="btn ghost" onClick={resetState}>
            <RotateCcw size={15} /> 重置提醒状态
          </button>
          <button
            type="button"
            className="btn primary"
            onClick={save}
            disabled={saving}
          >
            {saving ? "保存中…" : "保存设置"}
          </button>
        </div>
      </div>

      {assetModal ? (
        <AssetModal
          initial={assetModal.draft}
          index={assetModal.index}
          globalThreshold={globalThreshold}
          onSave={saveAsset}
          onClose={() => setAssetModal(null)}
        />
      ) : null}

      {channelModal ? (
        <ChannelModal
          initial={channelModal.draft}
          index={channelModal.index}
          onSave={saveChannel}
          onClose={() => setChannelModal(null)}
        />
      ) : null}
    </div>
  );
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
