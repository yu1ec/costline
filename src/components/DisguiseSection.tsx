import {
  BarChart3,
  Code2,
  Eye,
  FileText,
  Keyboard,
  MonitorPlay,
  Table2,
} from "lucide-react";
import {
  DISGUISE_PERSONAS,
  PERSONA_ORDER,
  personaAlias,
  type DisguiseMode,
  type DisguisePersonaId,
} from "../disguise";
import { formatPrice } from "../format";
import type { Row } from "../types";

const PERSONA_ICONS: Record<DisguisePersonaId, typeof Code2> = {
  dev: Code2,
  ops: BarChart3,
  product: FileText,
  finance: Table2,
};

export function DisguiseSection({
  persona,
  auto,
  rows,
  onChangePersona,
  onChangeAuto,
  onEnter,
}: {
  persona: DisguisePersonaId;
  auto: boolean;
  rows: Row[];
  onChangePersona: (persona: DisguisePersonaId) => void;
  onChangeAuto: (auto: boolean) => void;
  onEnter: (mode: Exclude<DisguiseMode, "off">) => void;
}) {
  const isMac =
    typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
  const mod = isMac ? "⌘" : "Ctrl";

  return (
    <section className="panel">
      <div className="panel-head">
        <div>
          <h2>伪装模式</h2>
          <p>
            把盯盘界面换成工作场景。后台盯盘和通知不受影响，只是别人看到的界面不同。
          </p>
        </div>
      </div>

      <div className="persona-grid">
        {PERSONA_ORDER.map((id) => {
          const meta = DISGUISE_PERSONAS[id];
          const Icon = PERSONA_ICONS[id];
          return (
            <button
              key={id}
              type="button"
              className={persona === id ? "persona-card active" : "persona-card"}
              onClick={() => onChangePersona(id)}
            >
              <span className="persona-icon" style={{ color: meta.accent }}>
                <Icon size={17} />
              </span>
              <span className="persona-text">
                <strong>{meta.name}</strong>
                <span>{meta.blurb}</span>
              </span>
            </button>
          );
        })}
      </div>

      <div className="disguise-actions">
        <button
          type="button"
          className="btn"
          onClick={() => onEnter("mask")}
        >
          <Eye size={15} />
          数据换皮
        </button>
        <button
          type="button"
          className="btn primary"
          onClick={() => onEnter("full")}
        >
          <MonitorPlay size={15} />
          全屏伪装
        </button>
      </div>

      <div className="disguise-hints">
        <div className="shortcut-line">
          <Keyboard size={14} />
          <span>全屏伪装</span>
          <kbd>{mod}</kbd>
          <kbd>⇧</kbd>
          <kbd>H</kbd>
        </div>
        <div className="shortcut-line">
          <Keyboard size={14} />
          <span>数据换皮</span>
          <kbd>{mod}</kbd>
          <kbd>⇧</kbd>
          <kbd>M</kbd>
        </div>
        <p>
          伪装状态下连点三次左上角标识，或按 <kbd>Esc</kbd>{" "}
          退出。伪装只改变界面和窗口标题，不会暂停盯盘，也不会拦截系统通知。
        </p>
      </div>

      {rows.length > 0 ? (
        <div className="disguise-map">
          <div className="disguise-map-head">
            <strong>数据对照</strong>
            <span>
              伪装界面里用左侧名称显示，数字都是真实行情。伪装中按住{" "}
              <kbd>{isMac ? "⌥" : "Alt"}</kbd> 可临时显示真实名称。
            </span>
          </div>
          <div className="disguise-map-list">
            {rows.map((row, index) => (
              <div key={row.key}>
                <span>{personaAlias(persona, index)}</span>
                <i>↔</i>
                <strong>{row.name}</strong>
                <em>{formatPrice(row.price)}</em>
              </div>
            ))}
          </div>
        </div>
      ) : null}

      <label className="toggle-row">
        <input
          type="checkbox"
          checked={auto}
          onChange={(event) => onChangeAuto(event.target.checked)}
        />
        <span>
          <strong>启动时自动进入全屏伪装</strong>
          <em>下次打开窗口先显示伪装界面，用快捷键再切回盯盘。</em>
        </span>
      </label>
    </section>
  );
}
