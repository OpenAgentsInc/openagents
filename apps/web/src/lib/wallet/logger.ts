export type LogLevel = "DEBUG" | "INFO" | "WARN" | "ERROR";

export interface LogEntry {
  timestamp: string;
  level: LogLevel;
  category: string;
  message: string;
  context?: Record<string, unknown>;
}

export const LogCategory = {
  AUTH: "auth",
  PAYMENT: "payment",
  SDK: "sdk",
  SDK_INTERNAL: "sdk-internal",
  UI: "ui",
  SESSION: "session",
  VALIDATION: "validation",
} as const;

const MAX_LOG_ENTRIES = 10000;
const logBuffer: LogEntry[] = [];
const SENSITIVE_KEYS = ["mnemonic", "seed", "privateKey", "password", "secret", "apiKey", "bolt11", "invoice"];

function sanitize(ctx?: Record<string, unknown>): Record<string, unknown> | undefined {
  if (!ctx) return undefined;
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(ctx)) {
    const lower = k.toLowerCase();
    if (SENSITIVE_KEYS.some((s) => lower.includes(s))) out[k] = "[REDACTED]";
    else if (typeof v === "object" && v !== null) out[k] = sanitize(v as Record<string, unknown>);
    else out[k] = v;
  }
  return out;
}

function log(level: LogLevel, category: string, message: string, context?: Record<string, unknown>): void {
  const entry: LogEntry = {
    timestamp: new Date().toISOString(),
    level,
    category,
    message,
    context: sanitize(context),
  };
  logBuffer.push(entry);
  if (logBuffer.length > MAX_LOG_ENTRIES) logBuffer.shift();
  if (import.meta.env.DEV) {
    const fmt = `[${entry.timestamp}] ${entry.level} [${entry.category}] ${entry.message}`;
    if (level === "ERROR") console.error(fmt, context ?? "");
    else if (level === "WARN") console.warn(fmt, context ?? "");
    else console.log(fmt, context ?? "");
  }
}

export function logSdkMessage(level: string, message: string): void {
  let l: LogLevel = "INFO";
  switch (level.toUpperCase()) {
    case "ERROR": l = "ERROR"; break;
    case "WARN":
    case "WARNING": l = "WARN"; break;
    case "DEBUG":
    case "TRACE": l = "DEBUG"; break;
  }
  log(l, LogCategory.SDK_INTERNAL, message);
}

export const walletLogger = {
  debug: (c: string, m: string, ctx?: Record<string, unknown>) => log("DEBUG", c, m, ctx),
  info: (c: string, m: string, ctx?: Record<string, unknown>) => log("INFO", c, m, ctx),
  warn: (c: string, m: string, ctx?: Record<string, unknown>) => log("WARN", c, m, ctx),
  error: (c: string, m: string, ctx?: Record<string, unknown>) => log("ERROR", c, m, ctx),
  authSuccess: (method: string) => log("INFO", LogCategory.AUTH, "Authentication succeeded", { method }),
  authFailure: (method: string, reason: string) => log("WARN", LogCategory.AUTH, "Authentication failed", { method, reason }),
  sessionEnd: (reason?: string) => log("INFO", LogCategory.SESSION, "Session ended", reason ? { reason } : undefined),
  sdkInitialized: () => log("INFO", LogCategory.SDK, "SDK initialized"),
  sdkError: (op: string, err: string) => log("ERROR", LogCategory.SDK, "SDK error", { operation: op, error: err }),
  getLogsAsString: () =>
    logBuffer.map((e) => `[${e.timestamp}] ${e.level} [${e.category}] ${e.message}${e.context ? " " + JSON.stringify(e.context) : ""}`).join("\n"),
  getLogsByCategory: (cat: string) => logBuffer.filter((e) => e.category === cat),
  initSession: async () => {},
  endSession: async () => {},
};
