export function formatPrice(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "-";
  const abs = Math.abs(value);
  const decimals = abs < 1 ? 4 : abs < 10 ? 3 : 2;
  return value.toFixed(decimals);
}

/** 带正负号的涨跌额，例如 +0.12 / -1.05 */
export function formatSignedPrice(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "-";
  const abs = Math.abs(value);
  const decimals = abs < 1 ? 4 : abs < 10 ? 3 : 2;
  return `${value >= 0 ? "+" : ""}${value.toFixed(decimals)}`;
}

export function formatPercent(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return "-";
  return `${value >= 0 ? "+" : ""}${value.toFixed(2)}%`;
}

export function currencySymbol(currency: string): string {
  switch (currency) {
    case "CNY":
      return "¥";
    case "HKD":
      return "HK$";
    case "USD":
      return "$";
    default:
      return "";
  }
}

export function formatMoney(value: number, currency: string): string {
  const sign = value >= 0 ? "+" : "-";
  return `${sign}${currencySymbol(currency)}${Math.abs(value).toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

export function formatAmount(value: number, currency: string): string {
  return `${currencySymbol(currency)}${value.toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

/** 把 2026-12-01 09:30 显示成 12-01 09:30 */
export function formatSipDate(value: string): string {
  return value.length >= 16 ? value.slice(5) : value;
}

export function formatTime(timestamp: number | null): string {
  if (!timestamp) return "尚未检查";
  const date = new Date(timestamp * 1000);
  return date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function formatClock(): string {
  return new Date().toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  });
}
