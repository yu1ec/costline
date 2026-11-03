import { AlertCircle, CheckCircle2, Download, RefreshCw, Rocket } from "lucide-react";
import type { UpdateController } from "../updater";

export function UpdateSection({ updates }: { updates: UpdateController }) {
  const { currentVersion, status, update, progress, error, check, install } = updates;
  const percent =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;
  const busy = status === "checking" || status === "downloading" || status === "ready";

  return (
    <section className="panel" id="update-panel">
      <div className="panel-head">
        <div>
          <h2>软件更新</h2>
          <p>更新包通过 GitHub Releases 分发，安装前会自动校验签名。</p>
        </div>
        <div className="panel-actions">
          <button
            type="button"
            className="btn"
            onClick={() => void check()}
            disabled={busy}
          >
            <RefreshCw size={15} className={status === "checking" ? "spin" : ""} />
            检查更新
          </button>
          {status === "available" ? (
            <button type="button" className="btn primary" onClick={() => void install()}>
              <Download size={15} />
              下载并安装
            </button>
          ) : null}
        </div>
      </div>

      <div className={`update-status ${status === "error" ? "error" : ""}`}>
        <StatusIcon status={status} />
        <div className="update-status-text">
          <span>{statusText(status, currentVersion, update?.version, percent)}</span>
          {update?.date ? (
            <span className="update-date">
              发布于 {new Date(update.date).toLocaleString("zh-CN")}
            </span>
          ) : null}
          {error ? <span className="update-error">{error}</span> : null}
        </div>
      </div>

      {status === "downloading" && percent != null ? (
        <div className="update-progress" aria-label={`下载进度 ${percent}%`}>
          <span style={{ width: `${percent}%` }} />
        </div>
      ) : null}

      {status === "available" && update?.body ? (
        <pre className="update-notes">{update.body}</pre>
      ) : null}
    </section>
  );
}

function StatusIcon({ status }: { status: UpdateController["status"] }) {
  if (status === "error") return <AlertCircle size={16} />;
  if (status === "current") return <CheckCircle2 size={16} />;
  if (status === "ready") return <Rocket size={16} />;
  return <Download size={16} />;
}

function statusText(
  status: UpdateController["status"],
  currentVersion: string,
  nextVersion: string | undefined,
  percent: number | null,
): string {
  const current = currentVersion ? `v${currentVersion}` : "当前版本";
  switch (status) {
    case "checking":
      return "正在检查更新…";
    case "current":
      return `已是最新版本 ${current}`;
    case "available":
      return `发现新版本 v${nextVersion ?? "?"}（当前 ${current}）`;
    case "downloading":
      return percent == null ? "正在下载更新…" : `正在下载更新 ${percent}%`;
    case "ready":
      return "安装完成，正在重启…";
    case "error":
      return "检查或安装更新失败";
    default:
      return `${current}，点击右上角检查更新。`;
  }
}
