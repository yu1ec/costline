export type AssetKind = "stock" | "fund" | "gold";

export type SipFrequency = "weekly" | "monthly";

export interface SipPlan {
  enabled: boolean;
  amount: number;
  frequency: SipFrequency;
  day: number;
  time: string;
}

export interface SipInfo {
  enabled: boolean;
  amount: number;
  schedule: string;
  next_at: string | null;
  due_today: boolean;
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
  notify: NotifyConfig;
  assets: Asset[];
}

export interface Row {
  key: string;
  name: string;
  kind: string;
  id: string;
  price: number;
  cost: number | null;
  target: number;
  change_percent: number | null;
  pnl: number | null;
  currency: string;
  above: boolean;
  triggered: boolean;
  note: string;
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
}

export interface SipPayload {
  title: string;
  body: string;
  reminders: SipReminder[];
}

export interface Snapshot {
  watching: boolean;
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
