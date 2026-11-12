import { invoke } from "@tauri-apps/api/core";
import type {
  AssetSearchItem,
  ChannelResult,
  Config,
  NoticeRecord,
  Snapshot,
} from "./types";

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  getSnapshot: () => invoke<Snapshot>("get_snapshot"),
  saveConfig: (config: Config) => invoke<Snapshot>("save_config", { config }),
  checkNow: () => invoke<Snapshot>("check_now"),
  setWatching: (watching: boolean) => invoke<Snapshot>("set_watching", { watching }),
  setRealtime: (realtime: boolean) =>
    invoke<Snapshot>("set_realtime", { realtime }),
  searchAssets: (keyword: string) =>
    invoke<AssetSearchItem[]>("search_assets", { keyword }),
  testNotify: (config: Config) => invoke<ChannelResult[]>("test_notify", { config }),
  getNoticeHistory: () => invoke<NoticeRecord[]>("get_notice_history"),
  clearNoticeHistory: () => invoke<void>("clear_notice_history"),
  resetState: () => invoke<Snapshot>("reset_state"),
  openConfigDir: () => invoke<void>("open_config_dir"),
  quit: () => invoke<void>("quit_app"),
  /** 收起托盘下拉面板。 */
  hideTrayPanel: () => invoke<void>("hide_tray_panel"),
  /** 打开完整面板（可指定落在哪个 tab）。 */
  openDashboard: (tab?: "dashboard" | "history" | "settings") =>
    invoke<void>("open_dashboard", { tab: tab ?? null }),
  /** 让下拉面板窗口高度贴合内容。 */
  resizeTrayPanel: (height: number) =>
    invoke<void>("resize_tray_panel", { height }),
};
