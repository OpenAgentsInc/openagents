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

type LndWalletState = "uninitialized" | "initialized" | "locked" | "unlocked";
type LndWalletRecoveryState =
  | "none"
  | "seed_backup_pending"
  | "seed_backup_acknowledged"
  | "restore_ready"
  | "restored";

type LndWalletSnapshot = Readonly<{
  readonly walletState: LndWalletState;
  readonly recoveryState: LndWalletRecoveryState;
  readonly seedBackupAcknowledged: boolean;
  readonly passphraseStored: boolean;
  readonly restorePrepared: boolean;
  readonly lastErrorCode: string | null;
  readonly lastErrorMessage: string | null;
  readonly lastOperation: string | null;
  readonly updatedAtMs: number;
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
      readonly lndWallet?: {
        readonly snapshot: () => Promise<LndWalletSnapshot>;
        readonly initialize: (input: {
          readonly passphrase: string;
          readonly seedMnemonic?: ReadonlyArray<string>;
        }) => Promise<void>;
        readonly unlock: (input?: {
          readonly passphrase?: string;
        }) => Promise<void>;
        readonly lock: () => Promise<void>;
        readonly acknowledgeSeedBackup: () => Promise<void>;
        readonly prepareRestore: () => Promise<void>;
        readonly restore: (input: {
          readonly passphrase: string;
          readonly seedMnemonic: ReadonlyArray<string>;
          readonly recoveryWindowDays?: number;
        }) => Promise<void>;
      };
    };
  }
}
