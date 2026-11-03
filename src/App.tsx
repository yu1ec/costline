import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  AlertCircle,
  ArrowLeft,
  Bell,
  Download,
  Eye,
  EyeOff,
  LineChart,
  Moon,
  Pause,
  Play,
  RefreshCw,
  Settings as SettingsIcon,
  Sun,
  X,
} from "lucide-react";
import { api } from "./api";
import type { AlertPayload, Config, SipPayload, Snapshot } from "./types";
import { draftToAsset, type AssetDraft } from "./drafts";
import {
  DISGUISE_PERSONAS,
  loadDisguisePrefs,
  saveDisguisePrefs,
  type DisguiseMode,
  type DisguisePersonaId,
} from "./disguise";
import { Dashboard } from "./pages/Dashboard";
import { Settings } from "./pages/Settings";
import { DisguiseScreen, MaskedBoard } from "./components/Disguise";
import { formatAmount, formatPrice, formatTime } from "./format";
import { useUpdateController } from "./updater";

type Tab = "dashboard" | "settings";
type Theme = "dark" | "light";
type ToastState = { id: number; text: string; tone: "ok" | "error" };
type DisguiseState = {
  persona: DisguisePersonaId;
  auto: boolean;
  mode: DisguiseMode;
};

export default function App() {
  const [config, setConfig] = useState<Config | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [tab, setTab] = useState<Tab>("dashboard");
  const [busy, setBusy] = useState(false);
  const [booting, setBooting] = useState(true);
  const [fatal, setFatal] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastState | null>(null);
  const [alert, setAlert] = useState<AlertPayload | null>(null);
  const [sipAlert, setSipAlert] = useState<SipPayload | null>(null);
  const [theme, setTheme] = useState<Theme>(() =>
    document.documentElement.dataset.theme === "light" ? "light" : "dark",
  );
  const [hideAmounts, setHideAmounts] = useState(
    () => localStorage.getItem("costline-hide-amounts") === "1",
  );
  const [disguise, setDisguise] = useState<DisguiseState>(() => {
    const prefs = loadDisguisePrefs();
    return {
      persona: prefs.persona,
      auto: prefs.auto,
      mode: prefs.auto ? "full" : "off",
    };
  });
  const updates = useUpdateController();

  const load = useCallback(async () => {
    setFatal(null);
    try {
      const [nextConfig, nextSnapshot] = await Promise.all([
        api.getConfig(),
        api.getSnapshot(),
      ]);
      setConfig(nextConfig);
      setSnapshot(nextSnapshot);
    } catch (error) {
      setFatal(errorMessage(error));
    } finally {
      setBooting(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<Snapshot>("snapshot", (event) => setSnapshot(event.payload)).then(
      (stop) => {
        if (disposed) stop();
        else unlisten = stop;
      },
    );
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<AlertPayload>("alert", (event) => setAlert(event.payload)).then(
      (stop) => {
        if (disposed) stop();
        else unlisten = stop;
      },
    );
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen<SipPayload>("sip", (event) => setSipAlert(event.payload)).then(
      (stop) => {
        if (disposed) stop();
        else unlisten = stop;
      },
    );
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const showToast = useCallback(
    (text: string, tone: "ok" | "error" = "ok") => {
      setToast({ id: Date.now(), text, tone });
    },
    [],
  );

  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), 4200);
    return () => clearTimeout(timer);
  }, [toast]);

  useEffect(() => {
    const timer = setTimeout(() => {
      void updates.check(true);
    }, 4000);
    return () => clearTimeout(timer);
  }, [updates.check]);

  useEffect(() => {
    if (updates.status === "available" && updates.update) {
      showToast(`发现新版本 v${updates.update.version}，可在设置中更新`);
    }
  }, [updates.status, updates.update, showToast]);

  const personaMeta = DISGUISE_PERSONAS[disguise.persona];

  useEffect(() => {
    saveDisguisePrefs({ persona: disguise.persona, auto: disguise.auto });
  }, [disguise.persona, disguise.auto]);

  useEffect(() => {
    localStorage.setItem("costline-hide-amounts", hideAmounts ? "1" : "0");
  }, [hideAmounts]);

  useEffect(() => {
    const title =
      disguise.mode === "off" ? "Costline 盯盘" : personaMeta.windowTitle;
    document.title = title;
    try {
      void getCurrentWindow().setTitle(title).catch(() => {});
    } catch {
      // 浏览器预览下没有 Tauri 窗口，忽略即可。
    }
  }, [disguise.mode, personaMeta.windowTitle]);

  useEffect(() => {
    function handleKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setDisguise((current) =>
          current.mode === "off" ? current : { ...current, mode: "off" },
        );
        return;
      }
      if (!(event.metaKey || event.ctrlKey) || !event.shiftKey) return;
      const key = event.key.toLowerCase();
      if (key !== "h" && key !== "m") return;
      event.preventDefault();
      setAlert(null);
      setSipAlert(null);
      setToast(null);
      setDisguise((current) => {
        const target: Exclude<DisguiseMode, "off"> = key === "h" ? "full" : "mask";
        return { ...current, mode: current.mode === target ? "off" : target };
      });
    }
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, []);

  const enterDisguise = useCallback((mode: Exclude<DisguiseMode, "off">) => {
    setAlert(null);
    setSipAlert(null);
    setToast(null);
    setDisguise((current) => ({ ...current, mode }));
  }, []);

  const exitDisguise = useCallback(() => {
    setDisguise((current) => ({ ...current, mode: "off" }));
  }, []);

  const changePersona = useCallback((persona: DisguisePersonaId) => {
    setDisguise((current) => ({ ...current, persona }));
  }, []);

  const changeAuto = useCallback((auto: boolean) => {
    setDisguise((current) => ({ ...current, auto }));
  }, []);

  function openUpdate() {
    setTab("settings");
    setTimeout(() => {
      document
        .getElementById("update-panel")
        ?.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 60);
  }

  function toggleTheme() {
    const next: Theme = theme === "dark" ? "light" : "dark";
    setTheme(next);
    document.documentElement.dataset.theme = next;
    document.documentElement.style.colorScheme = next;
    localStorage.setItem("costline-theme", next);
  }

  async function checkNow() {
    setBusy(true);
    try {
      const next = await api.checkNow();
      setSnapshot(next);
    } catch (error) {
      showToast(errorMessage(error), "error");
    } finally {
      setBusy(false);
    }
  }

  async function addAsset(draft: AssetDraft): Promise<boolean> {
    if (!config) return false;
    const payload: Config = {
      ...config,
      assets: [...config.assets, draftToAsset(draft)],
    };
    try {
      const nextSnapshot = await api.saveConfig(payload);
      setConfig(payload);
      setSnapshot(nextSnapshot);
      showToast("资产已添加并生效");
      return true;
    } catch (error) {
      showToast(errorMessage(error), "error");
      return false;
    }
  }

  async function toggleWatch() {
    if (!snapshot) return;
    setBusy(true);
    try {
      const next = await api.setWatching(!snapshot.watching);
      setSnapshot(next);
      showToast(next.watching ? "已恢复自动盯盘" : "已暂停自动盯盘");
    } catch (error) {
      showToast(errorMessage(error), "error");
    } finally {
      setBusy(false);
    }
  }

  if (disguise.mode === "full") {
    return (
      <DisguiseScreen
        persona={disguise.persona}
        snapshot={snapshot}
        onExit={exitDisguise}
      />
    );
  }

  if (disguise.mode === "mask") {
    return (
      <MaskedBoard
        persona={disguise.persona}
        snapshot={snapshot}
        onExit={exitDisguise}
      />
    );
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <div className="brand-mark">
            <LineChart size={18} />
          </div>
          <div className="brand-text">
            <h1>costline</h1>
            <span>回本盯盘</span>
          </div>
        </div>

        <div className="header-center">
          <span className={snapshot?.watching ? "status-pill live" : "status-pill"}>
            <span className="status-dot" />
            {snapshot?.watching ? "自动盯盘中" : "已暂停"}
          </span>
          <span className="header-meta">
            {snapshot?.last_check
              ? `最近检查 ${formatTime(snapshot.last_check)}`
              : "等待首次检查"}
          </span>
        </div>

        <div className="header-actions">
          <button
            type="button"
            className="btn ghost"
            onClick={() => enterDisguise("full")}
            title="全屏伪装"
          >
            <EyeOff size={15} />
            伪装
          </button>
          {tab === "dashboard" ? (
            <>
              <button
                type="button"
                className="btn ghost"
                onClick={() => setHideAmounts((current) => !current)}
                title={hideAmounts ? "显示投资数据" : "隐藏投资数据"}
                aria-pressed={hideAmounts}
              >
                {hideAmounts ? <Eye size={15} /> : <EyeOff size={15} />}
                {hideAmounts ? "显示" : "隐藏"}
              </button>
              <button type="button" className="btn ghost" onClick={toggleWatch} disabled={busy}>
                {snapshot?.watching ? <Pause size={15} /> : <Play size={15} />}
                {snapshot?.watching ? "暂停" : "继续"}
              </button>
              <button type="button" className="btn" onClick={checkNow} disabled={busy}>
                <RefreshCw size={15} className={busy ? "spin" : ""} />
                检查
              </button>
              <button type="button" className="btn" onClick={() => setTab("settings")}>
                <SettingsIcon size={15} />
                设置
              </button>
            </>
          ) : (
            <button type="button" className="btn ghost" onClick={() => setTab("dashboard")}>
              <ArrowLeft size={15} />
              返回盯盘
            </button>
          )}
          {updates.status === "available" ? (
            <button type="button" className="btn update-badge" onClick={openUpdate}>
              <Download size={15} />
              更新 v{updates.update?.version}
            </button>
          ) : null}
          <button
            type="button"
            className="icon-btn"
            onClick={toggleTheme}
            aria-label="切换主题"
          >
            {theme === "dark" ? <Sun size={17} /> : <Moon size={17} />}
          </button>
        </div>
      </header>

      {alert ? (
        <div className="alert-toast" role="alert">
          <div className="alert-toast-icon">
            <Bell size={17} />
          </div>
          <div className="alert-toast-content">
            <strong>{alert.title}</strong>
            <span>
              {alert.rows
                .slice(0, 3)
                .map((row) => `${row.name} ${formatPrice(row.price)}`)
                .join(" · ")}
              {alert.rows.length > 3 ? ` · 等 ${alert.rows.length} 项` : ""}
            </span>
          </div>
          <button
            type="button"
            className="btn ghost"
            onClick={() => {
              setTab("dashboard");
              setAlert(null);
            }}
          >
            查看
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label="关闭提醒"
            onClick={() => setAlert(null)}
          >
            <X size={15} />
          </button>
        </div>
      ) : null}

      {sipAlert ? (
        <div
          className={alert ? "alert-toast sip stacked" : "alert-toast sip"}
          role="alert"
        >
          <div className="alert-toast-icon">
            <Bell size={17} />
          </div>
          <div className="alert-toast-content">
            <strong>{sipAlert.title}</strong>
            <span>
              {sipAlert.reminders
                .slice(0, 3)
                .map(
                  (item) =>
                    `${item.name} ${formatAmount(item.amount, item.currency)}`,
                )
                .join(" · ")}
              {sipAlert.reminders.length > 3
                ? ` · 等 ${sipAlert.reminders.length} 项`
                : ""}
            </span>
          </div>
          <button
            type="button"
            className="btn ghost"
            onClick={() => {
              setTab("dashboard");
              setSipAlert(null);
            }}
          >
            查看
          </button>
          <button
            type="button"
            className="icon-btn"
            aria-label="关闭定投提醒"
            onClick={() => setSipAlert(null)}
          >
            <X size={15} />
          </button>
        </div>
      ) : null}

      <main className="app-main">
        {booting ? (
          <div className="boot-state">
            <RefreshCw size={20} className="spin" />
            <span>正在加载配置…</span>
          </div>
        ) : fatal || !config ? (
          <div className="boot-state error">
            <AlertCircle size={22} />
            <strong>启动失败</strong>
            <p>{fatal ?? "无法读取配置"}</p>
            <button type="button" className="btn" onClick={() => void load()}>
              重试
            </button>
          </div>
        ) : tab === "dashboard" ? (
          <Dashboard
            config={config}
            snapshot={snapshot}
            busy={busy}
            hideAmounts={hideAmounts}
            onCheck={checkNow}
            onAddAsset={addAsset}
            onOpenSettings={() => setTab("settings")}
          />
        ) : (
          <Settings
            config={config}
            snapshot={snapshot}
            updates={updates}
            disguisePersona={disguise.persona}
            disguiseAuto={disguise.auto}
            onDisguisePersona={changePersona}
            onDisguiseAuto={changeAuto}
            onEnterDisguise={enterDisguise}
            onSaved={(nextConfig, nextSnapshot) => {
              setConfig(nextConfig);
              setSnapshot(nextSnapshot);
            }}
            onSnapshot={setSnapshot}
            onToast={showToast}
          />
        )}
      </main>

      {toast ? (
        <div className={`toast ${toast.tone}`} key={toast.id}>
          {toast.tone === "ok" ? <CheckIcon /> : <AlertCircle size={16} />}
          <span>{toast.text}</span>
        </div>
      ) : null}
    </div>
  );
}

function CheckIcon() {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M20 6 9 17l-5-5" />
    </svg>
  );
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
