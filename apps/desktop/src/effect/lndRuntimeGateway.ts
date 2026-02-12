import { Context, Effect, Layer } from "effect";

export type DesktopLndRuntimeLifecycle =
  | "unavailable"
  | "stopped"
  | "starting"
  | "running"
  | "stopping"
  | "backoff"
  | "failed";

export type DesktopLndRuntimeHealth = "unknown" | "starting" | "healthy" | "unhealthy";

export type DesktopLndRuntimeStatus = Readonly<{
  readonly lifecycle: DesktopLndRuntimeLifecycle;
  readonly health: DesktopLndRuntimeHealth;
  readonly target: string | null;
  readonly pid: number | null;
  readonly restartCount: number;
  readonly crashCount: number;
  readonly nextRestartAtMs: number | null;
  readonly lastHealthCheckAtMs: number | null;
  readonly lastError: string | null;
  readonly sync: {
    readonly source: "none" | "rest_getinfo";
    readonly blockHeight: number | null;
    readonly numPeers: number | null;
    readonly bestHeaderTimestamp: number | null;
    readonly syncedToChain: boolean | null;
    readonly syncedToGraph: boolean | null;
    readonly walletSynced: boolean | null;
    readonly lastUpdatedAtMs: number | null;
    readonly lastError: string | null;
  };
}>;

const unavailableStatus = (): DesktopLndRuntimeStatus => ({
  lifecycle: "unavailable",
  health: "unknown",
  target: null,
  pid: null,
  restartCount: 0,
  crashCount: 0,
  nextRestartAtMs: null,
  lastHealthCheckAtMs: null,
  lastError: null,
  sync: {
    source: "none",
    blockHeight: null,
    numPeers: null,
    bestHeaderTimestamp: null,
    syncedToChain: null,
    syncedToGraph: null,
    walletSynced: null,
    lastUpdatedAtMs: null,
    lastError: null,
  },
});

const getRuntimeBridge = () => {
  if (typeof window === "undefined") return undefined;
  return window.openAgentsDesktop?.lndRuntime;
};

const normalizeSnapshot = (input: unknown): DesktopLndRuntimeStatus => {
  if (!input || typeof input !== "object") return unavailableStatus();

  const record = input as Record<string, unknown>;
  const lifecycle =
    typeof record.lifecycle === "string"
      ? (record.lifecycle as DesktopLndRuntimeLifecycle)
      : unavailableStatus().lifecycle;
  const health =
    typeof record.health === "string"
      ? (record.health as DesktopLndRuntimeHealth)
      : unavailableStatus().health;
  const syncRecord =
    record.sync && typeof record.sync === "object" ? (record.sync as Record<string, unknown>) : null;

  return {
    lifecycle,
    health,
    target: typeof record.target === "string" ? record.target : null,
    pid: typeof record.pid === "number" ? record.pid : null,
    restartCount: typeof record.restartCount === "number" ? record.restartCount : 0,
    crashCount: typeof record.crashCount === "number" ? record.crashCount : 0,
    nextRestartAtMs: typeof record.nextRestartAtMs === "number" ? record.nextRestartAtMs : null,
    lastHealthCheckAtMs:
      typeof record.lastHealthCheckAtMs === "number" ? record.lastHealthCheckAtMs : null,
    lastError: typeof record.lastError === "string" ? record.lastError : null,
    sync: {
      source:
        syncRecord?.source === "rest_getinfo" || syncRecord?.source === "none"
          ? syncRecord.source
          : "none",
      blockHeight: typeof syncRecord?.blockHeight === "number" ? syncRecord.blockHeight : null,
      numPeers: typeof syncRecord?.numPeers === "number" ? syncRecord.numPeers : null,
      bestHeaderTimestamp:
        typeof syncRecord?.bestHeaderTimestamp === "number" ? syncRecord.bestHeaderTimestamp : null,
      syncedToChain:
        typeof syncRecord?.syncedToChain === "boolean" ? syncRecord.syncedToChain : null,
      syncedToGraph:
        typeof syncRecord?.syncedToGraph === "boolean" ? syncRecord.syncedToGraph : null,
      walletSynced:
        typeof syncRecord?.walletSynced === "boolean" ? syncRecord.walletSynced : null,
      lastUpdatedAtMs:
        typeof syncRecord?.lastUpdatedAtMs === "number" ? syncRecord.lastUpdatedAtMs : null,
      lastError: typeof syncRecord?.lastError === "string" ? syncRecord.lastError : null,
    },
  };
};

export type LndRuntimeGatewayApi = Readonly<{
  readonly snapshot: () => Effect.Effect<DesktopLndRuntimeStatus>;
  readonly start: () => Effect.Effect<void>;
  readonly stop: () => Effect.Effect<void>;
  readonly restart: () => Effect.Effect<void>;
}>;

export class LndRuntimeGatewayService extends Context.Tag("@openagents/desktop/LndRuntimeGatewayService")<
  LndRuntimeGatewayService,
  LndRuntimeGatewayApi
>() {}

export const LndRuntimeGatewayLive = Layer.succeed(
  LndRuntimeGatewayService,
  LndRuntimeGatewayService.of({
    snapshot: () =>
      Effect.promise(async () => {
        const runtime = getRuntimeBridge();
        if (!runtime) return unavailableStatus();
        return normalizeSnapshot(await runtime.snapshot());
      }).pipe(Effect.catchAll(() => Effect.succeed(unavailableStatus()))),
    start: () =>
      Effect.promise(async () => {
        await getRuntimeBridge()?.start();
      }).pipe(Effect.catchAll(() => Effect.void)),
    stop: () =>
      Effect.promise(async () => {
        await getRuntimeBridge()?.stop();
      }).pipe(Effect.catchAll(() => Effect.void)),
    restart: () =>
      Effect.promise(async () => {
        await getRuntimeBridge()?.restart();
      }).pipe(Effect.catchAll(() => Effect.void)),
  }),
);
