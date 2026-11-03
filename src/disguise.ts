export type DisguisePersonaId = "dev" | "ops" | "product" | "finance";

export type DisguiseMode = "off" | "mask" | "full";

export interface DisguisePersona {
  id: DisguisePersonaId;
  name: string;
  blurb: string;
  windowTitle: string;
  brand: string;
  brandSub: string;
  accent: string;
  owner: string;
  avatar: string;
  nav: string[];
  detailTitle: string;
  codePrefix: string;
  totals: {
    monitors: string;
    ok: string;
    alerts: string;
    delta: string;
  };
  columns: {
    name: string;
    value: string;
    baseline: string;
    change: string;
    delta: string;
    status: string;
  };
  status: {
    triggered: string;
    above: string;
    below: string;
  };
  kindLabel: string;
  footer: string;
  aliases: string[];
}

export const PERSONA_ORDER: DisguisePersonaId[] = [
  "dev",
  "ops",
  "product",
  "finance",
];

export const DISGUISE_PERSONAS: Record<DisguisePersonaId, DisguisePersona> = {
  dev: {
    id: "dev",
    name: "开发",
    blurb: "IDE 工作区 + 代码审查 + 测试输出",
    windowTitle: "web-console — Orbit",
    brand: "orbit",
    brandSub: "web-console",
    accent: "#6ea8fe",
    owner: "yuki",
    avatar: "YK",
    nav: ["工作区", "代码审查", "流水线", "监控"],
    detailTitle: "服务指标",
    codePrefix: "svc",
    totals: {
      monitors: "服务",
      ok: "达标",
      alerts: "告警",
      delta: "差值",
    },
    columns: {
      name: "服务",
      value: "耗时",
      baseline: "基线 / 阈值",
      change: "抖动",
      delta: "差值",
      status: "健康度",
    },
    status: {
      triggered: "告警",
      above: "达标",
      below: "观察",
    },
    kindLabel: "服务",
    footer: "指标来自内部构建监控，30 秒同步一次。",
    aliases: [
      "api-gateway",
      "billing-worker",
      "order-sync",
      "search-index",
      "push-service",
      "media-transcode",
      "auth-proxy",
      "report-job",
    ],
  },
  ops: {
    id: "ops",
    name: "运营",
    blurb: "增长看板 + 渠道转化 + 活动数据",
    windowTitle: "增长看板 · 渠道转化",
    brand: "增长罗盘",
    brandSub: "Growth Console",
    accent: "#f59e0b",
    owner: "林雨",
    avatar: "林",
    nav: ["总览", "渠道", "活动", "用户"],
    detailTitle: "渠道表现",
    codePrefix: "ch",
    totals: {
      monitors: "渠道",
      ok: "达标",
      alerts: "异动",
      delta: "偏差",
    },
    columns: {
      name: "渠道",
      value: "转化率",
      baseline: "目标 / 预警",
      change: "环比",
      delta: "偏差",
      status: "状态",
    },
    status: {
      triggered: "预警",
      above: "正常",
      below: "偏低",
    },
    kindLabel: "渠道",
    footer: "数据来自增长数据仓，口径与周报一致。",
    aliases: [
      "信息流",
      "搜索推广",
      "私域社群",
      "联盟渠道",
      "直播带货",
      "内容种草",
      "短信触达",
      "地推",
    ],
  },
  product: {
    id: "product",
    name: "产品",
    blurb: "需求池 + PRD 文档 + 迭代进度",
    windowTitle: "需求池 · Q4 增长实验",
    brand: "需求雷达",
    brandSub: "Product Hub",
    accent: "#a78bfa",
    owner: "陈默",
    avatar: "陈",
    nav: ["需求池", "迭代", "文档", "评审"],
    detailTitle: "需求进度",
    codePrefix: "req",
    totals: {
      monitors: "需求",
      ok: "已完成",
      alerts: "风险",
      delta: "偏差",
    },
    columns: {
      name: "需求",
      value: "工时",
      baseline: "计划 / 阈值",
      change: "环比",
      delta: "偏差",
      status: "状态",
    },
    status: {
      triggered: "关注",
      above: "已完成",
      below: "进行中",
    },
    kindLabel: "需求",
    footer: "需求状态由项目空间自动同步。",
    aliases: [
      "结算页改版",
      "新手引导优化",
      "分享链路重构",
      "会员权益升级",
      "搜索排序实验",
      "消息中心改版",
      "数据看板二期",
      "帮助中心迁移",
    ],
  },
  finance: {
    id: "finance",
    name: "财务",
    blurb: "预算执行表 + 费用科目 + 超支预警",
    windowTitle: "预算执行表 · 2026 Q4",
    brand: "费用中心",
    brandSub: "Finance Workspace",
    accent: "#34d399",
    owner: "周岚",
    avatar: "周",
    nav: ["预算总表", "人力成本", "市场费用", "差旅"],
    detailTitle: "科目执行",
    codePrefix: "acc",
    totals: {
      monitors: "科目",
      ok: "执行正常",
      alerts: "超支预警",
      delta: "偏差",
    },
    columns: {
      name: "科目",
      value: "实际",
      baseline: "预算 / 阈值",
      change: "环比",
      delta: "偏差",
      status: "状态",
    },
    status: {
      triggered: "超支",
      above: "正常",
      below: "预算内",
    },
    kindLabel: "科目",
    footer: "数据来自财务共享中心，仅供内部核对。",
    aliases: [
      "市场推广",
      "云服务",
      "差旅费",
      "办公租赁",
      "外包服务",
      "招聘费用",
      "培训预算",
      "软件订阅",
    ],
  },
};

export interface DisguisePrefs {
  persona: DisguisePersonaId;
  auto: boolean;
}

const STORAGE_KEY = "costline-disguise";

export function loadDisguisePrefs(): DisguisePrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<DisguisePrefs>;
      if (parsed.persona && parsed.persona in DISGUISE_PERSONAS) {
        return { persona: parsed.persona, auto: Boolean(parsed.auto) };
      }
    }
  } catch {
    // 忽略损坏的本地配置，回退到默认值。
  }
  return { persona: "dev", auto: false };
}

export function saveDisguisePrefs(prefs: DisguisePrefs): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
  } catch {
    // localStorage 不可用时只影响持久化，不影响当次使用。
  }
}

export function personaAlias(
  persona: DisguisePersonaId,
  index: number,
): string {
  const list = DISGUISE_PERSONAS[persona].aliases;
  const alias = list[index % list.length];
  const round = Math.floor(index / list.length);
  return round === 0 ? alias : `${alias} ${round + 1}`;
}

export function personaCode(
  persona: DisguisePersonaId,
  index: number,
): string {
  return `${DISGUISE_PERSONAS[persona].codePrefix}-${String(index + 1).padStart(2, "0")}`;
}

export function formatSigned(value: number): string {
  const sign = value >= 0 ? "+" : "-";
  return `${sign}${Math.abs(value).toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}
