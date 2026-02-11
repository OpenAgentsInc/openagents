import { Context, Effect, Layer } from "effect";

export type DesktopWalletState = "uninitialized" | "initialized" | "locked" | "unlocked";
export type DesktopWalletRecoveryState =
  | "none"
  | "seed_backup_pending"
  | "seed_backup_acknowledged"
  | "restore_ready"
  | "restored";

export type DesktopWalletStatus = Readonly<{
  readonly walletState: DesktopWalletState;
  readonly recoveryState: DesktopWalletRecoveryState;
  readonly seedBackupAcknowledged: boolean;
  readonly passphraseStored: boolean;
  readonly restorePrepared: boolean;
  readonly lastErrorCode: string | null;
  readonly lastErrorMessage: string | null;
  readonly lastOperation: string | null;
  readonly updatedAtMs: number;
}>;

const fallbackStatus = (): DesktopWalletStatus => ({
  walletState: "uninitialized",
  recoveryState: "none",
  seedBackupAcknowledged: false,
  passphraseStored: false,
  restorePrepared: false,
  lastErrorCode: null,
  lastErrorMessage: null,
  lastOperation: null,
  updatedAtMs: Date.now(),
});

const normalizeWalletState = (value: unknown): DesktopWalletState => {
  if (value === "uninitialized" || value === "initialized" || value === "locked" || value === "unlocked") {
    return value;
  }
  return "uninitialized";
};

const normalizeRecoveryState = (value: unknown): DesktopWalletRecoveryState => {
  if (
    value === "none" ||
    value === "seed_backup_pending" ||
    value === "seed_backup_acknowledged" ||
    value === "restore_ready" ||
    value === "restored"
  ) {
    return value;
  }
  return "none";
};

const normalizeStatus = (input: unknown): DesktopWalletStatus => {
  if (!input || typeof input !== "object") return fallbackStatus();

  const record = input as Record<string, unknown>;
  return {
    walletState: normalizeWalletState(record.walletState),
    recoveryState: normalizeRecoveryState(record.recoveryState),
    seedBackupAcknowledged: record.seedBackupAcknowledged === true,
    passphraseStored: record.passphraseStored === true,
    restorePrepared: record.restorePrepared === true,
    lastErrorCode: typeof record.lastErrorCode === "string" ? record.lastErrorCode : null,
    lastErrorMessage: typeof record.lastErrorMessage === "string" ? record.lastErrorMessage : null,
    lastOperation: typeof record.lastOperation === "string" ? record.lastOperation : null,
    updatedAtMs: typeof record.updatedAtMs === "number" ? record.updatedAtMs : Date.now(),
  };
};

const walletBridge = () => {
  if (typeof window === "undefined") return undefined;
  return window.openAgentsDesktop?.lndWallet;
};

export type LndWalletGatewayApi = Readonly<{
  readonly snapshot: () => Effect.Effect<DesktopWalletStatus>;
  readonly initialize: (input: {
    readonly passphrase: string;
    readonly seedMnemonic?: ReadonlyArray<string>;
  }) => Effect.Effect<void>;
  readonly unlock: (input?: {
    readonly passphrase?: string;
  }) => Effect.Effect<void>;
  readonly lock: () => Effect.Effect<void>;
  readonly acknowledgeSeedBackup: () => Effect.Effect<void>;
  readonly prepareRestore: () => Effect.Effect<void>;
  readonly restore: (input: {
    readonly passphrase: string;
    readonly seedMnemonic: ReadonlyArray<string>;
    readonly recoveryWindowDays?: number;
  }) => Effect.Effect<void>;
}>;

export class LndWalletGatewayService extends Context.Tag("@openagents/desktop/LndWalletGatewayService")<
  LndWalletGatewayService,
  LndWalletGatewayApi
>() {}

export const LndWalletGatewayLive = Layer.succeed(
  LndWalletGatewayService,
  LndWalletGatewayService.of({
    snapshot: () =>
      Effect.promise(async () => {
        const bridge = walletBridge();
        if (!bridge) return fallbackStatus();
        return normalizeStatus(await bridge.snapshot());
      }).pipe(Effect.catchAll(() => Effect.succeed(fallbackStatus()))),

    initialize: (input) =>
      Effect.promise(async () => {
        await walletBridge()?.initialize(input);
      }).pipe(Effect.catchAll(() => Effect.void)),

    unlock: (input) =>
      Effect.promise(async () => {
        await walletBridge()?.unlock(input);
      }).pipe(Effect.catchAll(() => Effect.void)),

    lock: () =>
      Effect.promise(async () => {
        await walletBridge()?.lock();
      }).pipe(Effect.catchAll(() => Effect.void)),

    acknowledgeSeedBackup: () =>
      Effect.promise(async () => {
        await walletBridge()?.acknowledgeSeedBackup();
      }).pipe(Effect.catchAll(() => Effect.void)),

    prepareRestore: () =>
      Effect.promise(async () => {
        await walletBridge()?.prepareRestore();
      }).pipe(Effect.catchAll(() => Effect.void)),

    restore: (input) =>
      Effect.promise(async () => {
        await walletBridge()?.restore(input);
      }).pipe(Effect.catchAll(() => Effect.void)),
  }),
);
