import { Context, Layer } from "effect";

export type DesktopConfig = Readonly<{
  readonly openAgentsBaseUrl: string;
  readonly convexUrl: string;
  readonly khalaSyncEnabled: boolean;
  readonly khalaSyncUrl: string;
  readonly executorTickMs: number;
}>;

export class DesktopConfigService extends Context.Tag("@openagents/desktop/DesktopConfigService")<
  DesktopConfigService,
  DesktopConfig
>() {}

const DEFAULT_OPENAGENTS_BASE_URL = "https://openagents.com";
const DEFAULT_CONVEX_URL = "https://aware-caterpillar-962.convex.cloud";
const DEFAULT_EXECUTOR_TICK_MS = 2_000;

const deriveDefaultKhalaSyncUrl = (openAgentsBaseUrl: string): string => {
  try {
    const url = new URL(openAgentsBaseUrl);
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    url.pathname = "/sync/socket/websocket";
    url.search = "";
    url.hash = "";
    return url.toString();
  } catch {
    return "wss://openagents.com/sync/socket/websocket";
  }
};

const normalizeUrl = (raw: unknown, fallback: string): string => {
  if (typeof raw !== "string" || raw.trim().length === 0) return fallback;
  try {
    const u = new URL(raw);
    return u.toString().replace(/\/$/, "");
  } catch {
    return fallback;
  }
};

const normalizeTickMs = (raw: unknown): number => {
  const n =
    typeof raw === "number"
      ? raw
      : typeof raw === "string" && raw.trim().length > 0
        ? Number(raw)
        : NaN;
  if (!Number.isFinite(n)) return DEFAULT_EXECUTOR_TICK_MS;
  return Math.max(250, Math.min(60_000, Math.floor(n)));
};

const normalizeBoolean = (raw: unknown, fallback = false): boolean => {
  if (typeof raw === "boolean") return raw;
  if (typeof raw === "string") {
    const normalized = raw.trim().toLowerCase();
    if (normalized === "true" || normalized === "1" || normalized === "yes") return true;
    if (normalized === "false" || normalized === "0" || normalized === "no") return false;
  }

  return fallback;
};

const readBridgeConfig = (): Partial<DesktopConfig> => {
  if (typeof window === "undefined") return {};
  return window.openAgentsDesktop?.config ?? {};
};

export const makeDesktopConfig = (override?: Partial<DesktopConfig>): DesktopConfig => {
  const bridge = readBridgeConfig();
  const fromProcess = typeof process !== "undefined" ? process.env : undefined;
  const openAgentsBaseUrl = normalizeUrl(
    override?.openAgentsBaseUrl ??
      bridge.openAgentsBaseUrl ??
      fromProcess?.OA_DESKTOP_OPENAGENTS_BASE_URL,
    DEFAULT_OPENAGENTS_BASE_URL,
  );
  const khalaSyncEnabled = normalizeBoolean(
    override?.khalaSyncEnabled ??
      bridge.khalaSyncEnabled ??
      fromProcess?.OA_DESKTOP_KHALA_SYNC_ENABLED,
  );
  const khalaSyncUrl = normalizeUrl(
    override?.khalaSyncUrl ?? bridge.khalaSyncUrl ?? fromProcess?.OA_DESKTOP_KHALA_SYNC_URL,
    deriveDefaultKhalaSyncUrl(openAgentsBaseUrl),
  );

  return {
    openAgentsBaseUrl,
    convexUrl: normalizeUrl(
      override?.convexUrl ?? bridge.convexUrl ?? fromProcess?.OA_DESKTOP_CONVEX_URL,
      DEFAULT_CONVEX_URL,
    ),
    khalaSyncEnabled,
    khalaSyncUrl,
    executorTickMs: normalizeTickMs(
      override?.executorTickMs ?? bridge.executorTickMs ?? fromProcess?.OA_DESKTOP_EXECUTOR_TICK_MS,
    ),
  };
};

export const DesktopConfigLive = (override?: Partial<DesktopConfig>) =>
  Layer.succeed(DesktopConfigService, makeDesktopConfig(override));
