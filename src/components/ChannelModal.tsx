import { useState } from "react";
import { Plus, Trash2 } from "lucide-react";
import type { ChannelDraft, HeaderDraft } from "../drafts";
import {
  CHANNEL_OPTIONS,
  channelDraftError,
  channelLabel,
  emptyChannelDraft,
} from "../drafts";
import type { ChannelType } from "../types";
import { Field, Modal, NumberField, TextInput } from "./ui";

export function ChannelModal({
  initial,
  index,
  onSave,
  onClose,
}: {
  initial: ChannelDraft;
  index: number | null;
  onSave: (draft: ChannelDraft) => void;
  onClose: () => void;
}) {
  const [draft, setDraft] = useState<ChannelDraft>({ ...initial });
  const [error, setError] = useState<string | null>(null);

  function set<K extends keyof ChannelDraft>(key: K, value: ChannelDraft[K]) {
    setDraft((current) => ({ ...current, [key]: value }));
  }

  function changeType(type: ChannelType) {
    setDraft(emptyChannelDraft(type));
    setError(null);
  }

  function submit() {
    const problem = channelDraftError(draft);
    if (problem) {
      setError(problem);
      return;
    }
    onSave(draft);
  }

  function updateHeader(index: number, patch: Partial<HeaderDraft>) {
    setDraft((current) => ({
      ...current,
      headers: current.headers.map((header, position) =>
        position === index ? { ...header, ...patch } : header,
      ),
    }));
  }

  return (
    <Modal
      title={index == null ? "添加通知渠道" : "编辑通知渠道"}
      subtitle="可以同时启用多个渠道，触发时会逐一发送。"
      onClose={onClose}
      wide
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
        <div className="channel-grid">
          {CHANNEL_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              className={
                draft.type === option.value ? "channel-choice active" : "channel-choice"
              }
              onClick={() => changeType(option.value)}
            >
              <strong>{option.label}</strong>
              <span>{option.hint}</span>
            </button>
          ))}
        </div>

        <div className="divider" />

        <div className="form-grid two">
          {renderFields(draft, set, updateHeader)}
        </div>

        {error ? <div className="inline-error">{error}</div> : null}
      </div>
    </Modal>
  );
}

function renderFields(
  draft: ChannelDraft,
  set: <K extends keyof ChannelDraft>(key: K, value: ChannelDraft[K]) => void,
  updateHeader: (index: number, patch: Partial<HeaderDraft>) => void,
) {
  const label = channelLabel(draft.type);
  const title = <div className="form-section-title">{label} 参数</div>;

  switch (draft.type) {
    case "bark":
      return (
        <>
          {title}
          <Field label="Bark Key">
            <TextInput
              value={draft.key}
              placeholder="你的 Bark Key"
              onChange={(event) => set("key", event.target.value)}
            />
          </Field>
          <Field label="Server" hint="自建 Bark 时修改">
            <TextInput
              value={draft.server}
              placeholder="https://api.day.app"
              onChange={(event) => set("server", event.target.value)}
            />
          </Field>
          <Field label="分组">
            <TextInput
              value={draft.group}
              placeholder="costline"
              onChange={(event) => set("group", event.target.value)}
            />
          </Field>
          <Field label="提示音">
            <TextInput
              value={draft.sound}
              placeholder="default"
              onChange={(event) => set("sound", event.target.value)}
            />
          </Field>
          <Field label="级别">
            <TextInput
              value={draft.level}
              placeholder="active"
              onChange={(event) => set("level", event.target.value)}
            />
          </Field>
        </>
      );
    case "serverchan":
      return (
        <>
          {title}
          <Field label="SendKey" hint="sctp... 或 SCT...">
            <TextInput
              value={draft.sendkey}
              placeholder="sctp..."
              onChange={(event) => set("sendkey", event.target.value)}
            />
          </Field>
          <Field label="接口版本（可选）" hint="旧版 SCT 开头的 Key 填 2">
            <NumberField
              value={draft.version}
              placeholder="2"
              onChange={(value) => set("version", value)}
            />
          </Field>
          <Field label="Base URL（可选）" hint="自建 / 反向代理时使用">
            <TextInput
              value={draft.base_url}
              placeholder="https://..."
              onChange={(event) => set("base_url", event.target.value)}
            />
          </Field>
        </>
      );
    case "pushplus":
      return (
        <>
          {title}
          <Field label="Token">
            <TextInput
              value={draft.token}
              placeholder="PushPlus Token"
              onChange={(event) => set("token", event.target.value)}
            />
          </Field>
          <Field label="群组编码（可选）">
            <TextInput
              value={draft.topic}
              placeholder="topic"
              onChange={(event) => set("topic", event.target.value)}
            />
          </Field>
        </>
      );
    case "telegram":
      return (
        <>
          {title}
          <Field label="Bot Token">
            <TextInput
              value={draft.bot_token}
              placeholder="123456:ABC..."
              onChange={(event) => set("bot_token", event.target.value)}
            />
          </Field>
          <Field label="Chat ID">
            <TextInput
              value={draft.chat_id}
              placeholder="123456789"
              onChange={(event) => set("chat_id", event.target.value)}
            />
          </Field>
        </>
      );
    case "wecom":
      return (
        <>
          {title}
          <Field label="Webhook 地址" className="span-2">
            <TextInput
              value={draft.webhook}
              placeholder="https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=..."
              onChange={(event) => set("webhook", event.target.value)}
            />
          </Field>
        </>
      );
    case "dingtalk":
      return (
        <>
          {title}
          <Field label="Webhook 地址" className="span-2">
            <TextInput
              value={draft.webhook}
              placeholder="https://oapi.dingtalk.com/robot/send?access_token=..."
              onChange={(event) => set("webhook", event.target.value)}
            />
          </Field>
          <Field label="加签 Secret（可选）" className="span-2">
            <TextInput
              value={draft.secret}
              placeholder="SEC..."
              onChange={(event) => set("secret", event.target.value)}
            />
          </Field>
        </>
      );
    case "feishu":
      return (
        <>
          {title}
          <Field label="Webhook 地址" className="span-2">
            <TextInput
              value={draft.webhook}
              placeholder="https://open.feishu.cn/open-apis/bot/v2/hook/..."
              onChange={(event) => set("webhook", event.target.value)}
            />
          </Field>
          <Field label="签名 Secret（可选）" className="span-2">
            <TextInput
              value={draft.secret}
              placeholder="机器人开启签名校验时填写"
              onChange={(event) => set("secret", event.target.value)}
            />
          </Field>
        </>
      );
    case "ntfy":
      return (
        <>
          {title}
          <Field label="Topic">
            <TextInput
              value={draft.topic}
              placeholder="costline-请换成随机主题"
              onChange={(event) => set("topic", event.target.value)}
            />
          </Field>
          <Field label="Server">
            <TextInput
              value={draft.server}
              placeholder="https://ntfy.sh"
              onChange={(event) => set("server", event.target.value)}
            />
          </Field>
          <Field label="访问 Token（可选）" className="span-2">
            <TextInput
              value={draft.token}
              placeholder="tk_..."
              onChange={(event) => set("token", event.target.value)}
            />
          </Field>
        </>
      );
    case "gotify":
      return (
        <>
          {title}
          <Field label="Server 地址">
            <TextInput
              value={draft.server}
              placeholder="https://gotify.example.com"
              onChange={(event) => set("server", event.target.value)}
            />
          </Field>
          <Field label="App Token">
            <TextInput
              value={draft.token}
              placeholder="App Token"
              onChange={(event) => set("token", event.target.value)}
            />
          </Field>
        </>
      );
    case "webhook":
      return (
        <>
          {title}
          <Field label="Webhook URL" className="span-2">
            <TextInput
              value={draft.url}
              placeholder="https://example.com/hook"
              onChange={(event) => set("url", event.target.value)}
            />
          </Field>
          <div className="span-2 header-editor">
            <div className="header-editor-head">
              <span>请求头（可选）</span>
              <button
                type="button"
                className="btn mini"
                onClick={() =>
                  set("headers", [...draft.headers, { name: "", value: "" }])
                }
              >
                <Plus size={14} /> 添加
              </button>
            </div>
            {draft.headers.length === 0 ? (
              <p className="field-hint">例如 Authorization: Bearer xxx</p>
            ) : null}
            {draft.headers.map((header, position) => (
              <div className="header-row" key={position}>
                <TextInput
                  value={header.name}
                  placeholder="Header 名称"
                  onChange={(event) =>
                    updateHeader(position, { name: event.target.value })
                  }
                />
                <TextInput
                  value={header.value}
                  placeholder="值"
                  onChange={(event) =>
                    updateHeader(position, { value: event.target.value })
                  }
                />
                <button
                  type="button"
                  className="icon-btn danger"
                  aria-label="删除请求头"
                  onClick={() =>
                    set(
                      "headers",
                      draft.headers.filter((_, index) => index !== position),
                    )
                  }
                >
                  <Trash2 size={16} />
                </button>
              </div>
            ))}
          </div>
        </>
      );
    case "desktop":
      return (
        <>
          {title}
          <Field label="提示音（可选）" className="span-2">
            <TextInput
              value={draft.sound}
              placeholder="例如 Glass；留空使用系统默认"
              onChange={(event) => set("sound", event.target.value)}
            />
          </Field>
        </>
      );
  }
}
