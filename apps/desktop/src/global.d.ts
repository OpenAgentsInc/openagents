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

type SparkWalletLifecycle = "disconnected" | "connecting" | "connected" | "error";
type SparkWalletNetwork = "mainnet" | "regtest";

type SparkWalletSnapshot = Readonly<{
  readonly lifecycle: SparkWalletLifecycle;
  readonly network: SparkWalletNetwork;
  readonly apiKeyConfigured: boolean;
  readonly mnemonicStored: boolean;
  readonly identityPubkey: string | null;
  readonly balanceSats: number | null;
  readonly tokenBalanceCount: number;
  readonly lastSyncedAtMs: number | null;
  readonly lastPaymentId: string | null;
  readonly lastPaymentAtMs: number | null;
  readonly lastErrorCode: string | null;
  readonly lastErrorMessage: string | null;
}>;

type SparkInvoicePaymentResult = Readonly<{
  readonly paymentId: string;
  readonly amountMsats: number;
  readonly preimageHex: string;
  readonly paidAtMs: number;
}>;

type L402Credential = Readonly<{
  readonly host: string;
  readonly scope?: string | undefined;
  readonly macaroon: string;
  readonly preimageHex: string;
  readonly amountMsats: number;
  readonly issuedAtMs: number;
}>;

type L402CredentialCacheLookup =
  | Readonly<{ readonly _tag: "miss" }>
  | Readonly<{ readonly _tag: "hit"; readonly credential: L402Credential }>
  | Readonly<{ readonly _tag: "stale"; readonly credential: L402Credential }>;

declare global {
  interface Window {
    openAgentsDesktop?: {
      readonly config?: {
        readonly openAgentsBaseUrl?: string;
        readonly convexUrl?: string;
        readonly khalaSyncEnabled?: boolean;
        readonly khalaSyncUrl?: string;
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
      readonly sparkWallet?: {
        readonly snapshot: () => Promise<SparkWalletSnapshot>;
        readonly bootstrap: () => Promise<void>;
        readonly refresh: () => Promise<SparkWalletSnapshot>;
        readonly payInvoice: (input: {
          readonly invoice: string;
          readonly host: string;
          readonly maxAmountMsats: number;
        }) => Promise<SparkInvoicePaymentResult>;
        readonly disconnect: () => Promise<void>;
      };
      readonly l402CredentialCache?: {
        readonly getByHost: (input: {
          readonly host: string;
          readonly scope: string;
          readonly nowMs: number;
        }) => Promise<L402CredentialCacheLookup>;
        readonly putByHost: (input: {
          readonly host: string;
          readonly scope: string;
          readonly credential: L402Credential;
          readonly options?: { readonly ttlMs?: number };
        }) => Promise<void>;
        readonly markInvalid: (input: { readonly host: string; readonly scope: string }) => Promise<void>;
        readonly clearHost: (input: { readonly host: string; readonly scope: string }) => Promise<void>;
      };
    };
  }
}
