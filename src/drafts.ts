import type {
  Asset,
  AssetKind,
  Channel,
  ChannelType,
  Config,
} from "./types";

export interface GlobalDraft {
  interval: string;
  timeout: string;
  threshold: string;
  cooldown: string;
  repeat: string;
}

export interface AssetDraft {
  kind: AssetKind;
  name: string;
  symbol: string;
  secid: string;
  code: string;
  cost: string;
  quantity: string;
  principal: string;
  threshold: string;
  target: string;
}

export interface HeaderDraft {
  name: string;
  value: string;
}

export interface ChannelDraft {
  type: ChannelType;
  key: string;
  server: string;
  group: string;
  sound: string;
  level: string;
  sendkey: string;
  base_url: string;
  version: string;
  token: string;
  topic: string;
  bot_token: string;
  chat_id: string;
  webhook: string;
  secret: string;
  url: string;
  headers: HeaderDraft[];
}

export interface ConfigDraft {
  global: GlobalDraft;
  assets: AssetDraft[];
  channels: ChannelDraft[];
}

export const ASSET_KINDS: { value: AssetKind; label: string; hint: string }[] = [
  { value: "stock", label: "股票", hint: "A 股 / 港股 / 美股 / ETF" },
  { value: "fund", label: "基金", hint: "场外基金，如 161725" },
  { value: "gold", label: "黄金", hint: "AU9999、nf_AU0、hf_XAU" },
];

export const CHANNEL_OPTIONS: {
  value: ChannelType;
  label: string;
  hint: string;
}[] = [
  { value: "desktop", label: "系统通知", hint: "桌面版默认开启，可用于设置提示音" },
  { value: "bark", label: "Bark", hint: "iPhone 推送，免费" },
  { value: "serverchan", label: "Server 酱", hint: "微信推送，免费" },
  { value: "pushplus", label: "PushPlus", hint: "微信推送，免费" },
  { value: "telegram", label: "Telegram", hint: "Bot 消息" },
  { value: "wecom", label: "企业微信", hint: "群机器人" },
  { value: "dingtalk", label: "钉钉", hint: "群机器人" },
  { value: "feishu", label: "飞书", hint: "自定义机器人" },
  { value: "ntfy", label: "ntfy", hint: "开源，可自建" },
  { value: "gotify", label: "Gotify", hint: "开源，可自建" },
  { value: "webhook", label: "Webhook", hint: "POST JSON 到自定义地址" },
];

export function kindLabel(kind: AssetKind): string {
  return ASSET_KINDS.find((item) => item.value === kind)?.label ?? kind;
}

export function channelLabel(type: ChannelType): string {
  return CHANNEL_OPTIONS.find((item) => item.value === type)?.label ?? type;
}

function text(value: string | null | undefined): string {
  return value ?? "";
}

function numberText(value: number | null | undefined): string {
  return value == null ? "" : String(value);
}

export function parseNumber(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

function intOr(value: string, fallback: number): number {
  const parsed = parseNumber(value);
  return parsed == null ? fallback : Math.trunc(parsed);
}

export function assetToDraft(asset: Asset): AssetDraft {
  return {
    kind: asset.kind,
    name: text(asset.name),
    symbol: text(asset.symbol),
    secid: text(asset.secid),
    code: text(asset.code),
    cost: numberText(asset.cost),
    quantity: numberText(asset.quantity),
    principal: numberText(asset.principal),
    threshold: numberText(asset.threshold_percent),
    target: numberText(asset.target_price),
  };
}

export function emptyAssetDraft(kind: AssetKind = "stock"): AssetDraft {
  return {
    kind,
    name: "",
    symbol: "",
    secid: "",
    code: "",
    cost: "",
    quantity: "",
    principal: "",
    threshold: "",
    target: "",
  };
}

export function draftToAsset(draft: AssetDraft): Asset {
  const asset: Asset = { kind: draft.kind };
  const name = draft.name.trim();
  if (name) asset.name = name;

  const secid = draft.secid.trim();
  if (secid) asset.secid = secid;

  if (draft.kind === "fund") {
    const code = draft.code.trim() || draft.symbol.trim();
    if (code) asset.code = code;
  } else {
    const symbol = draft.symbol.trim() || draft.code.trim();
    if (symbol) asset.symbol = symbol;
  }

  const cost = parseNumber(draft.cost);
  if (cost != null) asset.cost = cost;
  const quantity = parseNumber(draft.quantity);
  if (quantity != null) asset.quantity = quantity;
  const principal = parseNumber(draft.principal);
  if (principal != null) asset.principal = principal;
  const threshold = parseNumber(draft.threshold);
  if (threshold != null) asset.threshold_percent = threshold;
  const target = parseNumber(draft.target);
  if (target != null) asset.target_price = target;

  return asset;
}

export function assetDraftError(draft: AssetDraft): string | null {
  if (draft.kind === "fund") {
    if (!draft.code.trim() && !draft.symbol.trim() && !draft.secid.trim()) {
      return "请填写基金代码";
    }
  } else if (!draft.symbol.trim() && !draft.secid.trim()) {
    return draft.kind === "gold" ? "请填写黄金代码" : "请填写股票代码";
  }

  const cost = parseNumber(draft.cost);
  const quantity = parseNumber(draft.quantity);
  const principal = parseNumber(draft.principal);
  const target = parseNumber(draft.target);

  if (cost == null && target == null && !(principal != null && quantity != null)) {
    return "请填写成本价，或者同时填写本金与持有数量";
  }
  if (cost == null && principal != null && quantity == null) {
    return "填写了本金，还需要填写持有数量才能算出成本价";
  }

  const fields: [string, string, boolean][] = [
    ["成本价", draft.cost, true],
    ["持有数量", draft.quantity, true],
    ["本金", draft.principal, true],
    ["触发价", draft.target, true],
    ["单独阈值", draft.threshold, false],
  ];
  for (const [label, raw, positive] of fields) {
    if (!raw.trim()) continue;
    const value = parseNumber(raw);
    if (value == null) return `${label}不是有效数字`;
    if (positive && value <= 0) return `${label}必须大于 0`;
    if (!positive && value < 0) return `${label}不能为负数`;
  }

  return null;
}

export function assetPrimaryCode(draft: AssetDraft): string {
  if (draft.kind === "fund") {
    return draft.code.trim() || draft.symbol.trim() || draft.secid.trim();
  }
  return draft.symbol.trim() || draft.secid.trim() || draft.code.trim();
}

export function assetDerivedCost(draft: AssetDraft): number | null {
  const cost = parseNumber(draft.cost);
  if (cost != null) return cost;
  const principal = parseNumber(draft.principal);
  const quantity = parseNumber(draft.quantity);
  if (principal != null && quantity != null && quantity > 0) {
    return principal / quantity;
  }
  return null;
}

export function assetDerivedTarget(
  draft: AssetDraft,
  globalThreshold: number,
): number | null {
  const target = parseNumber(draft.target);
  if (target != null) return target;
  const cost = assetDerivedCost(draft);
  if (cost == null) return null;
  const threshold = parseNumber(draft.threshold);
  return cost * (1 + (threshold ?? globalThreshold) / 100);
}

export function emptyChannelDraft(type: ChannelType): ChannelDraft {
  return {
    type,
    key: "",
    server:
      type === "bark"
        ? "https://api.day.app"
        : type === "ntfy"
          ? "https://ntfy.sh"
          : "",
    group: "",
    sound: "",
    level: "",
    sendkey: "",
    base_url: "",
    version: "",
    token: "",
    topic: "",
    bot_token: "",
    chat_id: "",
    webhook: "",
    secret: "",
    url: "",
    headers: [],
  };
}

export function channelToDraft(channel: Channel): ChannelDraft {
  return {
    type: channel.type,
    key: text(channel.key),
    server: text(channel.server),
    group: text(channel.group),
    sound: text(channel.sound),
    level: text(channel.level),
    sendkey: text(channel.sendkey),
    base_url: text(channel.base_url),
    version: channel.version == null ? "" : String(channel.version),
    token: text(channel.token),
    topic: text(channel.topic),
    bot_token: text(channel.bot_token),
    chat_id: text(channel.chat_id),
    webhook: text(channel.webhook),
    secret: text(channel.secret),
    url: text(channel.url),
    headers: Object.entries(channel.headers ?? {}).map(([name, value]) => ({
      name,
      value,
    })),
  };
}

function optional(value: string): string | undefined {
  const trimmed = value.trim();
  return trimmed ? trimmed : undefined;
}

export function draftToChannel(draft: ChannelDraft): Channel {
  switch (draft.type) {
    case "bark":
      return {
        type: "bark",
        key: draft.key.trim(),
        server: optional(draft.server),
        group: optional(draft.group),
        sound: optional(draft.sound),
        level: optional(draft.level),
      };
    case "serverchan": {
      const version = parseNumber(draft.version);
      return {
        type: "serverchan",
        sendkey: draft.sendkey.trim(),
        base_url: optional(draft.base_url),
        version: version == null ? undefined : Math.trunc(version),
      };
    }
    case "pushplus":
      return {
        type: "pushplus",
        token: draft.token.trim(),
        topic: optional(draft.topic),
      };
    case "telegram":
      return {
        type: "telegram",
        bot_token: draft.bot_token.trim(),
        chat_id: draft.chat_id.trim(),
      };
    case "wecom":
      return { type: "wecom", webhook: draft.webhook.trim() };
    case "dingtalk":
      return {
        type: "dingtalk",
        webhook: draft.webhook.trim(),
        secret: optional(draft.secret),
      };
    case "feishu":
      return {
        type: "feishu",
        webhook: draft.webhook.trim(),
        secret: optional(draft.secret),
      };
    case "ntfy":
      return {
        type: "ntfy",
        topic: draft.topic.trim(),
        server: optional(draft.server),
        token: optional(draft.token),
      };
    case "gotify":
      return {
        type: "gotify",
        server: draft.server.trim(),
        token: draft.token.trim(),
      };
    case "webhook": {
      const headers: Record<string, string> = {};
      for (const header of draft.headers) {
        const name = header.name.trim();
        if (name) headers[name] = header.value;
      }
      return {
        type: "webhook",
        url: draft.url.trim(),
        headers: Object.keys(headers).length ? headers : undefined,
      };
    }
    case "desktop":
      return { type: "desktop", sound: optional(draft.sound) };
  }
}

export function channelDraftError(draft: ChannelDraft): string | null {
  const require = (value: string, label: string) =>
    value.trim() ? null : `请填写${label}`;

  switch (draft.type) {
    case "bark":
      return require(draft.key, "Bark Key");
    case "serverchan":
      return require(draft.sendkey, "SendKey");
    case "pushplus":
      return require(draft.token, "PushPlus Token");
    case "telegram":
      return require(draft.bot_token, "Bot Token") ?? require(draft.chat_id, "Chat ID");
    case "wecom":
      return require(draft.webhook, "Webhook 地址");
    case "dingtalk":
      return require(draft.webhook, "Webhook 地址");
    case "feishu":
      return require(draft.webhook, "Webhook 地址");
    case "ntfy":
      return require(draft.topic, "Topic");
    case "gotify":
      return require(draft.server, "Server 地址") ?? require(draft.token, "App Token");
    case "webhook":
      return require(draft.url, "Webhook URL");
    case "desktop":
      return null;
  }
}

const MASK = (value: string) =>
  value.length <= 8
    ? `${value.slice(0, 2)}••••`
    : `${value.slice(0, 4)}••••${value.slice(-4)}`;

export function channelSummary(channel: Channel): string {
  switch (channel.type) {
    case "bark":
      return channel.server ? channel.server.replace(/^https?:\/\//, "") : "api.day.app";
    case "serverchan":
      return channel.sendkey ? MASK(channel.sendkey) : "未填写 SendKey";
    case "pushplus":
      return channel.token ? MASK(channel.token) : "未填写 Token";
    case "telegram":
      return channel.chat_id ? `chat ${channel.chat_id}` : "未填写 Chat ID";
    case "wecom":
    case "dingtalk":
    case "feishu":
      return channel.webhook
        ? channel.webhook.replace(/^https?:\/\//, "").split("/")[0]
        : "未填写 Webhook";
    case "ntfy":
      return `${(channel.server ?? "https://ntfy.sh").replace(/^https?:\/\//, "")}/${channel.topic}`;
    case "gotify":
      return channel.server ? channel.server.replace(/^https?:\/\//, "") : "未填写 Server";
    case "webhook":
      return channel.url ? channel.url.replace(/^https?:\/\//, "").split("/")[0] : "未填写 URL";
    case "desktop":
      return "默认开启，可配置提示音";
  }
}

export function configToDraft(config: Config): ConfigDraft {
  return {
    global: {
      interval: String(config.interval_secs),
      timeout: String(config.timeout_secs),
      threshold: String(config.threshold_percent),
      cooldown: String(config.notify.cooldown_secs),
      repeat: String(config.notify.repeat_interval_secs),
    },
    assets: config.assets.map(assetToDraft),
    channels: config.notify.channels.map(channelToDraft),
  };
}

export function draftToConfig(draft: ConfigDraft): Config {
  return {
    interval_secs: intOr(draft.global.interval, 60),
    timeout_secs: intOr(draft.global.timeout, 10),
    threshold_percent: parseNumber(draft.global.threshold) ?? 0,
    notify: {
      cooldown_secs: intOr(draft.global.cooldown, 1800),
      repeat_interval_secs: intOr(draft.global.repeat, 0),
      channels: draft.channels.map(draftToChannel),
    },
    assets: draft.assets.map(draftToAsset),
  };
}
