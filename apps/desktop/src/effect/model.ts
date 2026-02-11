export type DesktopAuthStatus = "signed_out" | "code_requested" | "signed_in";

export type ExecutorLoopStatus = "stopped" | "running";

export type ExecutorRunStatus =
  | "waiting_auth"
  | "idle"
  | "running_task"
  | "completed_task"
  | "failed_task";

export type ExecutorTaskStatus =
  | "queued"
  | "approved"
  | "running"
  | "paid"
  | "cached"
  | "blocked"
  | "failed"
  | "completed";

export type ExecutorTaskRequest = Readonly<{
  readonly url: string;
  readonly method?: "GET" | "POST" | "PUT" | "PATCH" | "DELETE";
  readonly headers?: Readonly<Record<string, string>>;
  readonly body?: string;
  readonly maxSpendMsats: number;
  readonly challengeHeader?: string;
  readonly forceRefresh?: boolean;
  readonly scope?: string;
  readonly cacheTtlMs?: number;
}>;

export type ExecutorTask = Readonly<{
  readonly id: string;
  readonly ownerId: string;
  readonly status: ExecutorTaskStatus;
  readonly request: ExecutorTaskRequest;
  readonly attemptCount: number;
  readonly source?: string;
  readonly idempotencyKey?: string;
  readonly requestId?: string;
  readonly metadata?: unknown;
  readonly lastErrorCode?: string;
  readonly lastErrorMessage?: string;
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
  readonly wallet: {
    readonly walletState: "uninitialized" | "initialized" | "locked" | "unlocked";
    readonly recoveryState:
      | "none"
      | "seed_backup_pending"
      | "seed_backup_acknowledged"
      | "restore_ready"
      | "restored";
    readonly seedBackupAcknowledged: boolean;
    readonly passphraseStored: boolean;
    readonly restorePrepared: boolean;
    readonly lastErrorCode: string | null;
    readonly lastErrorMessage: string | null;
    readonly lastOperation: string | null;
    readonly updatedAtMs: number | null;
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
  wallet: {
    walletState: "uninitialized",
    recoveryState: "none",
    seedBackupAcknowledged: false,
    passphraseStored: false,
    restorePrepared: false,
    lastErrorCode: null,
    lastErrorMessage: null,
    lastOperation: null,
    updatedAtMs: null,
  },
});
