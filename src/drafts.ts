import type {
  Asset,
  AssetKind,
  Channel,
  ChannelType,
  Config,
  SipFrequency,
  Trade,
  TradeSide,
} from "./types";

export interface GlobalDraft {
  interval: string;
  timeout: string;
  threshold: string;
  cooldown: string;
  repeat: string;
  importDir: string;
  realtime: boolean;
}

/** 可编辑的成交记录：数字字段保留字符串，方便输入过程中清空或只输入一半。 */
export interface TradeDraft {
  id?: string | null;
  note?: string | null;
  side: TradeSide;
  quantity: string;
  price: string;
  fee: string;
  date: string;
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
  asOf: string;
  trades: TradeDraft[];
  threshold: string;
  target: string;
  sipEnabled: boolean;
  sipAmount: string;
  sipFrequency: SipFrequency;
  sipDay: string;
  sipTime: string;
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

function tradeToDraft(trade: Trade): TradeDraft {
  return {
    id: trade.id ?? null,
    note: trade.note ?? null,
    side: trade.side,
    quantity: numberText(trade.quantity),
    price: numberText(trade.price),
    fee: numberText(trade.fee),
    date: text(trade.date),
  };
}

function draftToTrade(draft: TradeDraft): Trade {
  const trade: Trade = {
    side: draft.side,
    quantity: parseNumber(draft.quantity) ?? 0,
    price: parseNumber(draft.price) ?? 0,
  };
  const fee = parseNumber(draft.fee);
  if (fee != null) trade.fee = fee;
  const date = draft.date.trim();
  if (date) trade.date = date;
  if (draft.id) trade.id = draft.id;
  if (draft.note) trade.note = draft.note;
  return trade;
}

function todayText(): string {
  const now = new Date();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${now.getFullYear()}-${month}-${day}`;
}

export function newTradeDraft(): TradeDraft {
  return { side: "buy", quantity: "", price: "", fee: "", date: todayText() };
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
    asOf: text(asset.as_of),
    trades: (asset.trades ?? []).map(tradeToDraft),
    threshold: numberText(asset.threshold_percent),
    target: numberText(asset.target_price),
    sipEnabled: asset.sip?.enabled ?? false,
    sipAmount: numberText(asset.sip?.amount),
    sipFrequency: asset.sip?.frequency ?? "monthly",
    sipDay: asset.sip ? String(asset.sip.day) : "1",
    sipTime: asset.sip?.time ?? "09:30",
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
    asOf: "",
    trades: [],
    threshold: "",
    target: "",
    sipEnabled: false,
    sipAmount: "",
    sipFrequency: "monthly",
    sipDay: "1",
    sipTime: "09:30",
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
  const asOf = draft.asOf.trim();
  if (asOf) asset.as_of = asOf;
  if (draft.trades.length > 0) asset.trades = draft.trades.map(draftToTrade);
  const threshold = parseNumber(draft.threshold);
  if (threshold != null) asset.threshold_percent = threshold;
  const target = parseNumber(draft.target);
  if (target != null) asset.target_price = target;

  const sipAmount = parseNumber(draft.sipAmount);
  if (draft.sipEnabled || sipAmount != null) {
    asset.sip = {
      enabled: draft.sipEnabled,
      amount: sipAmount ?? 0,
      frequency: draft.sipFrequency,
      day: Math.trunc(parseNumber(draft.sipDay) ?? 1),
      time: draft.sipTime.trim() || "09:30",
    };
  }

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

  // 有成交记录时，持仓与成本价可以由流水推导，期初持仓可以留空。
  if (draft.trades.length === 0) {
    if (
      cost == null &&
      target == null &&
      !(principal != null && quantity != null)
    ) {
      return "请填写成本价，或者同时填写本金与持有数量";
    }
    if (cost == null && principal != null && quantity == null) {
      return "填写了本金，还需要填写持有数量才能算出成本价";
    }
  }

  const asOf = draft.asOf.trim();
  if (asOf && !/^\d{4}-\d{2}-\d{2}$/.test(asOf)) {
    return "期初持仓日期格式应为 YYYY-MM-DD";
  }

  for (const [index, trade] of draft.trades.entries()) {
    const tradeQuantity = parseNumber(trade.quantity);
    const tradePrice = parseNumber(trade.price);
    const fee = parseNumber(trade.fee);
    if (tradeQuantity == null || tradeQuantity <= 0) {
      return `第 ${index + 1} 笔成交数量必须大于 0`;
    }
    if (tradePrice == null || tradePrice <= 0) {
      return `第 ${index + 1} 笔成交价格必须大于 0`;
    }
    if (fee != null && fee < 0) {
      return `第 ${index + 1} 笔手续费不能为负数`;
    }
    const date = trade.date.trim();
    if (date && !/^\d{4}-\d{2}-\d{2}$/.test(date)) {
      return `第 ${index + 1} 笔成交日期格式应为 YYYY-MM-DD`;
    }
  }

  const position = assetDerivedPosition(draft);
  if (position.error) return position.error;

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

  const sipAmount = parseNumber(draft.sipAmount);
  if (sipAmount != null && sipAmount <= 0) {
    return "定投金额必须大于 0";
  }
  if (draft.sipEnabled) {
    if (sipAmount == null) {
      return "请填写定投金额";
    }
    const day = parseNumber(draft.sipDay);
    if (day == null || !Number.isInteger(day)) {
      return "定投日不是有效数字";
    }
    if (draft.sipFrequency === "weekly" && (day < 1 || day > 7)) {
      return "每周定投日请选择周一到周日";
    }
    if (draft.sipFrequency === "monthly" && (day < 1 || day > 31)) {
      return "每月定投日必须在 1-31 之间";
    }
    if (!/^([01]\d|2[0-3]):[0-5]\d$/.test(draft.sipTime.trim())) {
      return "定投提醒时间格式应为 HH:MM";
    }
  }

  return null;
}

export const WEEKDAY_LABELS = [
  "周一",
  "周二",
  "周三",
  "周四",
  "周五",
  "周六",
  "周日",
];

export function sipScheduleLabel(draft: AssetDraft): string {
  const day = Math.trunc(parseNumber(draft.sipDay) ?? 1);
  const dayText =
    draft.sipFrequency === "weekly"
      ? WEEKDAY_LABELS[Math.min(Math.max(day, 1), 7) - 1]
      : `每月 ${day} 日`;
  return `${dayText} ${draft.sipTime.trim() || "09:30"}`;
}

export function assetDraftKey(draft: AssetDraft): string {
  const id =
    draft.secid.trim() || draft.symbol.trim() || draft.code.trim() || "?";
  return `${draft.kind}:${id.toLowerCase()}`;
}

export function assetPrimaryCode(draft: AssetDraft): string {
  if (draft.kind === "fund") {
    return draft.code.trim() || draft.symbol.trim() || draft.secid.trim();
  }
  return draft.symbol.trim() || draft.secid.trim() || draft.code.trim();
}

const POSITION_EPSILON = 1e-9;

export interface PositionPreview {
  quantity: number;
  principal: number;
  cost: number | null;
  hasPosition: boolean;
  recovered: boolean;
  closed: boolean;
  /** 成交流水无法推导时的错误，例如卖出超过持有数量 */
  error: string | null;
}

/**
 * 前端版的 Position::derive：期初持仓 + 成交流水。
 * 规则与 crates/costline-core/src/position.rs 保持一致：
 * 买入累加数量与本金，卖出净额从本金中扣除，成本价 = 剩余本金 ÷ 剩余数量。
 */
export function assetDerivedPosition(draft: AssetDraft): PositionPreview {
  const quantity = parseNumber(draft.quantity);
  const cost = parseNumber(draft.cost);
  const principal = parseNumber(draft.principal);

  let positionQuantity = quantity ?? 0;
  let positionPrincipal =
    principal ?? (cost != null ? cost * (quantity ?? 0) : 0);
  const hasPosition =
    quantity != null || principal != null || draft.trades.length > 0;

  const asOf = draft.asOf.trim();
  const ordered = draft.trades
    .map((trade, index) => ({ trade, index }))
    // 期初持仓已经包含 as_of 当天及以前的成交，不重复应用。
    .filter(({ trade }) => {
      const date = trade.date.trim();
      return !date || !asOf || date > asOf;
    })
    .sort((left, right) => {
      const leftDate = left.trade.date.trim();
      const rightDate = right.trade.date.trim();
      if (leftDate === rightDate) return left.index - right.index;
      return leftDate < rightDate ? -1 : 1;
    })
    .map(({ trade }) => trade);

  let error: string | null = null;
  for (const trade of ordered) {
    const tradeQuantity = parseNumber(trade.quantity);
    const tradePrice = parseNumber(trade.price);
    const fee = parseNumber(trade.fee) ?? 0;
    if (tradeQuantity == null || tradeQuantity <= 0) {
      error = "成交数量必须大于 0";
      break;
    }
    if (tradePrice == null || tradePrice <= 0) {
      error = "成交价格必须大于 0";
      break;
    }
    if (!Number.isFinite(fee) || fee < 0) {
      error = "手续费不能为负数";
      break;
    }

    if (trade.side === "buy") {
      positionQuantity += tradeQuantity;
      positionPrincipal += tradeQuantity * tradePrice + fee;
    } else {
      if (tradeQuantity > positionQuantity + POSITION_EPSILON) {
        error = `卖出 ${trimNumber(tradeQuantity)} 超过持有数量 ${trimNumber(positionQuantity)}`;
        break;
      }
      positionQuantity = Math.max(0, positionQuantity - tradeQuantity);
      positionPrincipal -= tradeQuantity * tradePrice - fee;
    }
  }

  if (Math.abs(positionPrincipal) < POSITION_EPSILON) positionPrincipal = 0;
  const closed = hasPosition && positionQuantity <= POSITION_EPSILON;
  const recovered = hasPosition && !closed && positionPrincipal <= 0;
  return {
    quantity: positionQuantity,
    principal: positionPrincipal,
    cost:
      positionQuantity > POSITION_EPSILON
        ? positionPrincipal / positionQuantity
        : null,
    hasPosition,
    recovered,
    closed,
    error,
  };
}

/** 成交日期不晚于期初持仓日期、因此不会计入的笔数。 */
export function skippedTradeCount(draft: AssetDraft): number {
  const asOf = draft.asOf.trim();
  if (!asOf) return 0;
  return draft.trades.filter((trade) => {
    const date = trade.date.trim();
    return Boolean(date) && date <= asOf;
  }).length;
}

export function assetDerivedCost(draft: AssetDraft): number | null {
  // 有成交流水时，成本价由流水推导；否则沿用 cost / 本金 ÷ 数量。
  if (draft.trades.length > 0) {
    return assetDerivedPosition(draft).cost;
  }
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
  if (draft.trades.length > 0) {
    // 本金已收回或已清仓时不再做回本提醒，和 Rust 侧保持一致。
    const position = assetDerivedPosition(draft);
    if (position.recovered || position.closed || position.cost == null) {
      return null;
    }
    const threshold = parseNumber(draft.threshold);
    return position.cost * (1 + (threshold ?? globalThreshold) / 100);
  }
  const cost = assetDerivedCost(draft);
  if (cost == null) return null;
  const threshold = parseNumber(draft.threshold);
  return cost * (1 + (threshold ?? globalThreshold) / 100);
}

function trimNumber(value: number): string {
  return String(Number(value.toFixed(6)));
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
      importDir: config.import?.dir ?? "",
      realtime: config.realtime ?? false,
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
    realtime: draft.global.realtime,
    notify: {
      cooldown_secs: intOr(draft.global.cooldown, 1800),
      repeat_interval_secs: intOr(draft.global.repeat, 0),
      channels: draft.channels.map(draftToChannel),
    },
    import: draft.global.importDir.trim()
      ? { dir: draft.global.importDir.trim() }
      : undefined,
    assets: draft.assets.map(draftToAsset),
  };
}
