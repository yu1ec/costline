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
};
