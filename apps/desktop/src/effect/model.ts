export type DesktopAuthStatus = "signed_out" | "code_requested" | "signed_in";

export type ExecutorLoopStatus = "stopped" | "running";

export type ExecutorRunStatus =
  | "waiting_auth"
  | "idle"
  | "running_task"
  | "completed_task"
  | "failed_task";

export type ExecutorTaskStatus = "queued" | "running" | "completed" | "failed";

export type ExecutorTask = Readonly<{
  readonly id: string;
  readonly payload: string;
  readonly status: ExecutorTaskStatus;
  readonly createdAtMs: number;
  readonly updatedAtMs: number;
  readonly failureReason?: string;
}>;

export type DesktopRuntimeState = Readonly<{
  readonly auth: {
    readonly status: DesktopAuthStatus;
    readonly userId: string | null;
    readonly email: string | null;
    readonly tokenPresent: boolean;
    readonly lastError: string | null;
  };
  readonly connectivity: {
    readonly openAgentsReachable: boolean;
    readonly convexReachable: boolean;
    readonly lastCheckedAtMs: number | null;
  };
  readonly executor: {
    readonly loop: ExecutorLoopStatus;
    readonly status: ExecutorRunStatus;
    readonly ticks: number;
    readonly lastTransitionAtMs: number | null;
    readonly lastTaskId: string | null;
    readonly lastError: string | null;
  };
  readonly lnd: {
    readonly lifecycle: "unavailable" | "stopped" | "starting" | "running" | "stopping" | "backoff" | "failed";
    readonly health: "unknown" | "starting" | "healthy" | "unhealthy";
    readonly target: string | null;
    readonly pid: number | null;
    readonly restartCount: number;
    readonly crashCount: number;
    readonly nextRestartAtMs: number | null;
    readonly lastHealthCheckAtMs: number | null;
    readonly lastError: string | null;
  };
}>;

export const initialDesktopRuntimeState = (): DesktopRuntimeState => ({
  auth: {
    status: "signed_out",
    userId: null,
    email: null,
    tokenPresent: false,
    lastError: null,
  },
  connectivity: {
    openAgentsReachable: false,
    convexReachable: false,
    lastCheckedAtMs: null,
  },
  executor: {
    loop: "stopped",
    status: "waiting_auth",
    ticks: 0,
    lastTransitionAtMs: null,
    lastTaskId: null,
    lastError: null,
  },
  lnd: {
    lifecycle: "unavailable",
    health: "unknown",
    target: null,
    pid: null,
    restartCount: 0,
    crashCount: 0,
    nextRestartAtMs: null,
    lastHealthCheckAtMs: null,
    lastError: null,
  },
});
