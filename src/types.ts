export type AssetKind = "stock" | "fund" | "gold";

export type SipFrequency = "weekly" | "monthly";

export interface SipPlan {
  enabled: boolean;
  amount: number;
  frequency: SipFrequency;
  day: number;
  time: string;
}

export interface SipTradeInfo {
  status: string;
  blocked: boolean;
  max_amount: number | null;
  reason: string | null;
  checked_at: number;
  error: string | null;
}

export interface SipInfo {
  enabled: boolean;
  amount: number;
  schedule: string;
  next_at: string | null;
  due_today: boolean;
  trade: SipTradeInfo | null;
}

export type TradeSide = "buy" | "sell";

export interface Trade {
  id?: string | null;
  side: TradeSide;
  quantity: number;
  price: number;
  fee?: number;
  date?: string | null;
  note?: string | null;
}

export interface ImportConfig {
  dir?: string | null;
}

export interface Asset {
  kind: AssetKind;
  name?: string | null;
  symbol?: string | null;
  secid?: string | null;
  code?: string | null;
  cost?: number | null;
  quantity?: number | null;
  principal?: number | null;
  as_of?: string | null;
  trades?: Trade[];
  threshold_percent?: number | null;
  target_price?: number | null;
  sip?: SipPlan | null;
}

export type ChannelType =
  | "bark"
  | "serverchan"
  | "pushplus"
  | "telegram"
  | "wecom"
  | "dingtalk"
  | "feishu"
  | "ntfy"
  | "gotify"
  | "webhook"
  | "desktop";

export interface Channel {
  type: ChannelType;
  key?: string;
  server?: string;
  group?: string;
  sound?: string;
  level?: string;
  sendkey?: string;
  base_url?: string;
  version?: number;
  token?: string;
  topic?: string;
  bot_token?: string;
  chat_id?: string;
  webhook?: string;
  secret?: string;
  url?: string;
  headers?: Record<string, string>;
}

export interface NotifyConfig {
  cooldown_secs: number;
  repeat_interval_secs: number;
  channels: Channel[];
}

export interface Config {
  interval_secs: number;
  timeout_secs: number;
  threshold_percent: number;
  realtime: boolean;
  notify: NotifyConfig;
  import?: ImportConfig | null;
  assets: Asset[];
}

export interface GoldReference {
  price: number;
  change_percent: number | null;
  note: string;
}

export interface Row {
  key: string;
  name: string;
  kind: string;
  id: string;
  price: number;
  cost: number | null;
  target: number | null;
  change_percent: number | null;
  pnl: number | null;
  principal: number | null;
  quantity: number | null;
  currency: string;
  above: boolean;
  triggered: boolean;
  recovered: boolean;
  closed: boolean;
  note: string;
  gold_price: GoldReference | null;
  sip: SipInfo | null;
}

export interface SipReminder {
  key: string;
  name: string;
  kind: string;
  id: string;
  amount: number;
  currency: string;
  price: number | null;
  estimate_quantity: number | null;
  schedule: string;
  next_at: string | null;
  status: "ready" | "blocked" | "unknown";
  reason: string | null;
  trade_status: string | null;
  max_amount: number | null;
}

export interface SipPayload {
  title: string;
  body: string;
  reminders: SipReminder[];
}

export interface Snapshot {
  watching: boolean;
  realtime: boolean;
  interval_secs: number;
  assets_count: number;
  rows: Row[];
  last_check: number | null;
  last_error: string | null;
  config_path: string;
}

export interface AssetSearchItem {
  kind: AssetKind;
  name: string;
  code: string;
  symbol: string | null;
  secid: string | null;
  market: string | null;
}

export interface AlertPayload {
  title: string;
  body: string;
  rows: Row[];
}

export interface ChannelResult {
  label: string;
  ok: boolean;
  error: string | null;
}
