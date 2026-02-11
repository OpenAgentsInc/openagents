import { Clock, Context, Effect, Layer, Ref } from "effect";

import {
  LndWalletOperationError,
  LndWalletService,
  type LndWalletState,
} from "@openagentsinc/lnd-effect";

import {
  DesktopSecureStorageError,
  DesktopSecureStorageService,
} from "./desktopSecureStorage";
import { LndRuntimeManagerService } from "./lndRuntimeManager";

export type LndWalletRecoveryState =
  | "none"
  | "seed_backup_pending"
  | "seed_backup_acknowledged"
  | "restore_ready"
  | "restored";

export type LndWalletManagerErrorCode =
  | "runtime_not_running"
  | "wallet_not_initialized"
  | "wallet_already_initialized"
  | "passphrase_missing"
  | "invalid_seed_phrase"
  | "restore_not_prepared"
  | "secure_storage_error"
  | "wallet_service_error";

export class LndWalletManagerError extends Error {
  readonly code: LndWalletManagerErrorCode;

  constructor(code: LndWalletManagerErrorCode, message: string) {
    super(message);
    this.name = "LndWalletManagerError";
    this.code = code;
  }
}

export type LndWalletManagerStatus = Readonly<{
  readonly walletState: LndWalletState;
  readonly recoveryState: LndWalletRecoveryState;
  readonly seedBackupAcknowledged: boolean;
  readonly passphraseStored: boolean;
  readonly restorePrepared: boolean;
  readonly lastErrorCode: LndWalletManagerErrorCode | null;
  readonly lastErrorMessage: string | null;
  readonly lastOperation: string | null;
  readonly updatedAtMs: number;
}>;

export type LndWalletManagerConfig = Readonly<{
  readonly passphraseStorageKey: string;
  readonly seedBackupAckStorageKey: string;
  readonly restorePreparedStorageKey: string;
  readonly restoredStorageKey: string;
}>;

export const defaultLndWalletManagerConfig = (): LndWalletManagerConfig => ({
  passphraseStorageKey: "lnd.wallet.passphrase",
  seedBackupAckStorageKey: "lnd.wallet.seed_backup_ack",
  restorePreparedStorageKey: "lnd.wallet.restore_prepared",
  restoredStorageKey: "lnd.wallet.restored",
});

export class LndWalletManagerConfigService extends Context.Tag("@openagents/desktop/LndWalletManagerConfigService")<
  LndWalletManagerConfigService,
  LndWalletManagerConfig
>() {}

export const LndWalletManagerConfigLive = (config: LndWalletManagerConfig) =>
  Layer.succeed(LndWalletManagerConfigService, config);

const initialWalletManagerStatus = (): LndWalletManagerStatus => ({
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

const toMessage = (value: unknown): string => {
  if (value instanceof Error) return value.message;
  return String(value);
};

const normalizeSecretFlag = (value: string | null): boolean => value === "1";

const computeRecoveryState = (input: {
  readonly walletState: LndWalletState;
  readonly seedBackupAcknowledged: boolean;
  readonly restorePrepared: boolean;
  readonly restored: boolean;
}): LndWalletRecoveryState => {
  if (input.restorePrepared) return "restore_ready";
  if (input.restored) return "restored";
  if (input.walletState === "uninitialized") return "none";
  return input.seedBackupAcknowledged ? "seed_backup_acknowledged" : "seed_backup_pending";
};

export type LndWalletManagerApi = Readonly<{
  readonly bootstrap: () => Effect.Effect<void, LndWalletManagerError>;
  readonly snapshot: () => Effect.Effect<LndWalletManagerStatus>;
  readonly initializeWallet: (input: {
    readonly passphrase: string;
    readonly seedMnemonic?: ReadonlyArray<string>;
  }) => Effect.Effect<void, LndWalletManagerError>;
  readonly unlockWallet: (input?: {
    readonly passphrase?: string;
  }) => Effect.Effect<void, LndWalletManagerError>;
  readonly lockWallet: () => Effect.Effect<void, LndWalletManagerError>;
  readonly acknowledgeSeedBackup: () => Effect.Effect<void, LndWalletManagerError>;
  readonly prepareRestore: () => Effect.Effect<void, LndWalletManagerError>;
  readonly restoreWallet: (input: {
    readonly passphrase: string;
    readonly seedMnemonic: ReadonlyArray<string>;
    readonly recoveryWindowDays?: number;
  }) => Effect.Effect<void, LndWalletManagerError>;
}>;

export class LndWalletManagerService extends Context.Tag("@openagents/desktop/LndWalletManagerService")<
  LndWalletManagerService,
  LndWalletManagerApi
>() {}

export const LndWalletManagerLive = Layer.effect(
  LndWalletManagerService,
  Effect.gen(function* () {
    const config = yield* LndWalletManagerConfigService;
    const secureStorage = yield* DesktopSecureStorageService;
    const runtimeManager = yield* LndRuntimeManagerService;
    const walletService = yield* LndWalletService;

    const statusRef = yield* Ref.make<LndWalletManagerStatus>(initialWalletManagerStatus());

    const setStatus = (f: (current: LndWalletManagerStatus, now: number) => LndWalletManagerStatus) =>
      Effect.gen(function* () {
        const now = yield* Clock.currentTimeMillis;
        yield* Ref.update(statusRef, (current) => f(current, now));
      });

    const failWithStatus = (input: {
      readonly code: LndWalletManagerErrorCode;
      readonly message: string;
      readonly operation: string;
    }): Effect.Effect<never, LndWalletManagerError> =>
      setStatus((current, now) => ({
        ...current,
        lastErrorCode: input.code,
        lastErrorMessage: input.message,
        lastOperation: input.operation,
        updatedAtMs: now,
      })).pipe(Effect.zipRight(Effect.fail(new LndWalletManagerError(input.code, input.message))));

    const ensureRuntimeRunning = (operation: string): Effect.Effect<void, LndWalletManagerError> =>
      Effect.gen(function* () {
        const runtime = yield* runtimeManager.snapshot();
        if (runtime.lifecycle !== "running") {
          return yield* failWithStatus({
            code: "runtime_not_running",
            message: `Wallet operation '${operation}' requires running LND runtime`,
            operation,
          });
        }
      });

    const refreshState = (operation: string): Effect.Effect<void, LndWalletManagerError> =>
      Effect.gen(function* () {
        const walletState = yield* walletService.getWalletState().pipe(
          Effect.mapError((error) =>
            new LndWalletManagerError("wallet_service_error", `Wallet state probe failed: ${toMessage(error)}`),
          ),
        );

        const passphraseStored = Boolean(yield* secureStorage.getSecret(config.passphraseStorageKey).pipe(
          Effect.mapError((error) =>
            new LndWalletManagerError(
              "secure_storage_error",
              `Secret storage read failed (${error.code}): ${error.message}`,
            ),
          ),
        ));

        const seedBackupAck = normalizeSecretFlag(
          yield* secureStorage.getSecret(config.seedBackupAckStorageKey).pipe(
            Effect.mapError((error) =>
              new LndWalletManagerError(
                "secure_storage_error",
                `Secret storage read failed (${error.code}): ${error.message}`,
              ),
            ),
          ),
        );

        const restorePrepared = normalizeSecretFlag(
          yield* secureStorage.getSecret(config.restorePreparedStorageKey).pipe(
            Effect.mapError((error) =>
              new LndWalletManagerError(
                "secure_storage_error",
                `Secret storage read failed (${error.code}): ${error.message}`,
              ),
            ),
          ),
        );

        const restored = normalizeSecretFlag(
          yield* secureStorage.getSecret(config.restoredStorageKey).pipe(
            Effect.mapError((error) =>
              new LndWalletManagerError(
                "secure_storage_error",
                `Secret storage read failed (${error.code}): ${error.message}`,
              ),
            ),
          ),
        );

        yield* setStatus((current, now) => ({
          ...current,
          walletState,
          seedBackupAcknowledged: seedBackupAck,
          passphraseStored,
          restorePrepared,
          recoveryState: computeRecoveryState({
            walletState,
            seedBackupAcknowledged: seedBackupAck,
            restorePrepared,
            restored,
          }),
          lastErrorCode: null,
          lastErrorMessage: null,
          lastOperation: operation,
          updatedAtMs: now,
        }));
      }).pipe(
        Effect.catchAll((error) =>
          failWithStatus({
            code: error instanceof LndWalletManagerError ? error.code : "wallet_service_error",
            message: error instanceof LndWalletManagerError ? error.message : toMessage(error),
            operation,
          }),
        ),
      );

    const resolvePassphrase = (operation: string, passphrase?: string): Effect.Effect<string, LndWalletManagerError> =>
      Effect.gen(function* () {
        const trimmed = passphrase?.trim();
        if (trimmed && trimmed.length > 0) {
          return trimmed;
        }
        const stored = yield* secureStorage.getSecret(config.passphraseStorageKey).pipe(
          Effect.mapError((error) =>
            new LndWalletManagerError(
              "secure_storage_error",
              `Secret storage read failed (${error.code}): ${error.message}`,
            ),
          ),
        );

        if (!stored || stored.trim().length === 0) {
          return yield* failWithStatus({
            code: "passphrase_missing",
            message: `Wallet operation '${operation}' requires a passphrase`,
            operation,
          });
        }
        return stored;
      });

    const mapWalletError = (operation: string, error: unknown): LndWalletManagerError => {
      if (error instanceof LndWalletManagerError) return error;
      if (error instanceof DesktopSecureStorageError) {
        return new LndWalletManagerError(
          "secure_storage_error",
          `Secret storage failure (${error.code}): ${error.message}`,
        );
      }
      if (error instanceof LndWalletOperationError) {
        return new LndWalletManagerError(
          "wallet_service_error",
          `${operation} failed: ${error.reason}`,
        );
      }
      return new LndWalletManagerError("wallet_service_error", `${operation} failed: ${toMessage(error)}`);
    };

    const initializeWallet = (input: {
      readonly passphrase: string;
      readonly seedMnemonic?: ReadonlyArray<string>;
    }): Effect.Effect<void, LndWalletManagerError> =>
      Effect.gen(function* () {
        yield* ensureRuntimeRunning("initialize_wallet");

        const current = yield* Ref.get(statusRef);
        if (current.walletState !== "uninitialized") {
          return yield* failWithStatus({
            code: "wallet_already_initialized",
            message: `Wallet is already ${current.walletState}`,
            operation: "initialize_wallet",
          });
        }

        const passphrase = input.passphrase.trim();
        if (passphrase.length === 0) {
          return yield* failWithStatus({
            code: "passphrase_missing",
            message: "Wallet initialization requires a non-empty passphrase",
            operation: "initialize_wallet",
          });
        }

        yield* walletService.initializeWallet({
          passphrase,
          ...(input.seedMnemonic ? { seedMnemonic: [...input.seedMnemonic] } : {}),
        }).pipe(Effect.mapError((error) => mapWalletError("initialize_wallet", error)));

        yield* secureStorage.setSecret(config.passphraseStorageKey, passphrase).pipe(
          Effect.mapError((error) => mapWalletError("initialize_wallet", error)),
        );
        yield* secureStorage.deleteSecret(config.seedBackupAckStorageKey).pipe(
          Effect.mapError((error) => mapWalletError("initialize_wallet", error)),
        );
        yield* secureStorage.deleteSecret(config.restoredStorageKey).pipe(
          Effect.mapError((error) => mapWalletError("initialize_wallet", error)),
        );
        yield* secureStorage.deleteSecret(config.restorePreparedStorageKey).pipe(
          Effect.mapError((error) => mapWalletError("initialize_wallet", error)),
        );

        yield* refreshState("initialize_wallet");
      }).pipe(
        Effect.catchAll((error) =>
          failWithStatus({
            code: error instanceof LndWalletManagerError ? error.code : "wallet_service_error",
            message: error instanceof LndWalletManagerError ? error.message : toMessage(error),
            operation: "initialize_wallet",
          }),
        ),
      );

    const unlockWallet = (input?: {
      readonly passphrase?: string;
    }): Effect.Effect<void, LndWalletManagerError> =>
      Effect.gen(function* () {
        yield* ensureRuntimeRunning("unlock_wallet");

        const current = yield* Ref.get(statusRef);
        if (current.walletState === "uninitialized") {
          return yield* failWithStatus({
            code: "wallet_not_initialized",
            message: "Wallet has not been initialized yet",
            operation: "unlock_wallet",
          });
        }

        const passphrase = yield* resolvePassphrase("unlock_wallet", input?.passphrase);
        yield* walletService.unlockWallet({ passphrase }).pipe(
          Effect.mapError((error) => mapWalletError("unlock_wallet", error)),
        );

        yield* secureStorage.setSecret(config.passphraseStorageKey, passphrase).pipe(
          Effect.mapError((error) => mapWalletError("unlock_wallet", error)),
        );

        yield* refreshState("unlock_wallet");
      }).pipe(
        Effect.catchAll((error) =>
          failWithStatus({
            code: error instanceof LndWalletManagerError ? error.code : "wallet_service_error",
            message: error instanceof LndWalletManagerError ? error.message : toMessage(error),
            operation: "unlock_wallet",
          }),
        ),
      );

    const lockWallet = (): Effect.Effect<void, LndWalletManagerError> =>
      Effect.gen(function* () {
        yield* walletService.lockWallet().pipe(Effect.mapError((error) => mapWalletError("lock_wallet", error)));
        yield* refreshState("lock_wallet");
      }).pipe(
        Effect.catchAll((error) =>
          failWithStatus({
            code: error instanceof LndWalletManagerError ? error.code : "wallet_service_error",
            message: error instanceof LndWalletManagerError ? error.message : toMessage(error),
            operation: "lock_wallet",
          }),
        ),
      );

    const acknowledgeSeedBackup = (): Effect.Effect<void, LndWalletManagerError> =>
      Effect.gen(function* () {
        const current = yield* Ref.get(statusRef);
        if (current.walletState === "uninitialized") {
          return yield* failWithStatus({
            code: "wallet_not_initialized",
            message: "Cannot acknowledge seed backup before wallet initialization",
            operation: "ack_seed_backup",
          });
        }

        yield* secureStorage.setSecret(config.seedBackupAckStorageKey, "1").pipe(
          Effect.mapError((error) => mapWalletError("ack_seed_backup", error)),
        );
        yield* refreshState("ack_seed_backup");
      }).pipe(
        Effect.catchAll((error) =>
          failWithStatus({
            code: error instanceof LndWalletManagerError ? error.code : "wallet_service_error",
            message: error instanceof LndWalletManagerError ? error.message : toMessage(error),
            operation: "ack_seed_backup",
          }),
        ),
      );

    const prepareRestore = (): Effect.Effect<void, LndWalletManagerError> =>
      Effect.gen(function* () {
        yield* secureStorage.setSecret(config.restorePreparedStorageKey, "1").pipe(
          Effect.mapError((error) => mapWalletError("prepare_restore", error)),
        );
        yield* refreshState("prepare_restore");
      }).pipe(
        Effect.catchAll((error) =>
          failWithStatus({
            code: error instanceof LndWalletManagerError ? error.code : "wallet_service_error",
            message: error instanceof LndWalletManagerError ? error.message : toMessage(error),
            operation: "prepare_restore",
          }),
        ),
      );

    const restoreWallet = (input: {
      readonly passphrase: string;
      readonly seedMnemonic: ReadonlyArray<string>;
      readonly recoveryWindowDays?: number;
    }): Effect.Effect<void, LndWalletManagerError> =>
      Effect.gen(function* () {
        yield* ensureRuntimeRunning("restore_wallet");

        const restorePrepared = normalizeSecretFlag(
          yield* secureStorage.getSecret(config.restorePreparedStorageKey).pipe(
            Effect.mapError((error) => mapWalletError("restore_wallet", error)),
          ),
        );

        if (!restorePrepared) {
          return yield* failWithStatus({
            code: "restore_not_prepared",
            message: "Restore flow must be prepared before restoring wallet",
            operation: "restore_wallet",
          });
        }

        const normalizedSeed = input.seedMnemonic
          .map((word) => word.trim())
          .filter((word) => word.length > 0);

        if (normalizedSeed.length < 12 || normalizedSeed.length > 24) {
          return yield* failWithStatus({
            code: "invalid_seed_phrase",
            message: "Restore requires a seed phrase with 12-24 words",
            operation: "restore_wallet",
          });
        }

        const passphrase = input.passphrase.trim();
        if (passphrase.length === 0) {
          return yield* failWithStatus({
            code: "passphrase_missing",
            message: "Restore requires a non-empty passphrase",
            operation: "restore_wallet",
          });
        }

        yield* walletService
          .restoreWallet({
            passphrase,
            seedMnemonic: normalizedSeed,
            ...(input.recoveryWindowDays !== undefined
              ? { recoveryWindowDays: input.recoveryWindowDays }
              : {}),
          })
          .pipe(Effect.mapError((error) => mapWalletError("restore_wallet", error)));

        yield* secureStorage.setSecret(config.passphraseStorageKey, passphrase).pipe(
          Effect.mapError((error) => mapWalletError("restore_wallet", error)),
        );
        yield* secureStorage.setSecret(config.restoredStorageKey, "1").pipe(
          Effect.mapError((error) => mapWalletError("restore_wallet", error)),
        );
        yield* secureStorage.deleteSecret(config.restorePreparedStorageKey).pipe(
          Effect.mapError((error) => mapWalletError("restore_wallet", error)),
        );
        yield* secureStorage.setSecret(config.seedBackupAckStorageKey, "1").pipe(
          Effect.mapError((error) => mapWalletError("restore_wallet", error)),
        );

        yield* refreshState("restore_wallet");
      }).pipe(
        Effect.catchAll((error) =>
          failWithStatus({
            code: error instanceof LndWalletManagerError ? error.code : "wallet_service_error",
            message: error instanceof LndWalletManagerError ? error.message : toMessage(error),
            operation: "restore_wallet",
          }),
        ),
      );

    return LndWalletManagerService.of({
      bootstrap: () => refreshState("wallet_bootstrap"),
      snapshot: () => Ref.get(statusRef),
      initializeWallet,
      unlockWallet,
      lockWallet,
      acknowledgeSeedBackup,
      prepareRestore,
      restoreWallet,
    });
  }),
);

export type LndWalletSnapshotForRenderer = Readonly<{
  readonly walletState: LndWalletState;
  readonly recoveryState: LndWalletRecoveryState;
  readonly seedBackupAcknowledged: boolean;
  readonly passphraseStored: boolean;
  readonly restorePrepared: boolean;
  readonly lastErrorCode: LndWalletManagerErrorCode | null;
  readonly lastErrorMessage: string | null;
  readonly lastOperation: string | null;
  readonly updatedAtMs: number;
}>;

export const projectLndWalletSnapshotForRenderer = (
  snapshot: LndWalletManagerStatus,
): LndWalletSnapshotForRenderer => ({
  walletState: snapshot.walletState,
  recoveryState: snapshot.recoveryState,
  seedBackupAcknowledged: snapshot.seedBackupAcknowledged,
  passphraseStored: snapshot.passphraseStored,
  restorePrepared: snapshot.restorePrepared,
  lastErrorCode: snapshot.lastErrorCode,
  lastErrorMessage: snapshot.lastErrorMessage,
  lastOperation: snapshot.lastOperation,
  updatedAtMs: snapshot.updatedAtMs,
});
