export {};

type LndRendererLifecycle = "stopped" | "starting" | "running" | "stopping" | "backoff" | "failed";
type LndRendererHealth = "unknown" | "starting" | "healthy" | "unhealthy";

type LndRuntimeSnapshot = Readonly<{
  readonly lifecycle: LndRendererLifecycle;
  readonly health: LndRendererHealth;
  readonly target: string | null;
  readonly pid: number | null;
  readonly restartCount: number;
  readonly crashCount: number;
  readonly nextRestartAtMs: number | null;
  readonly lastHealthCheckAtMs: number | null;
  readonly lastError: string | null;
}>;

declare global {
  interface Window {
    openAgentsDesktop?: {
      readonly config?: {
        readonly openAgentsBaseUrl?: string;
        readonly convexUrl?: string;
        readonly executorTickMs?: number;
      };
      readonly lndRuntime?: {
        readonly snapshot: () => Promise<LndRuntimeSnapshot>;
        readonly start: () => Promise<void>;
        readonly stop: () => Promise<void>;
        readonly restart: () => Promise<void>;
      };
    };
  }
}
