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
