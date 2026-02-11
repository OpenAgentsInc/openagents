import { Context, Effect, Layer } from "effect";
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";

import {
  LndWalletOperationError,
  LndWalletService,
  type LndWalletState,
  type LndWalletInitializeRequest,
  type LndWalletRestoreRequest,
  type LndWalletUnlockRequest,
} from "@openagentsinc/lnd-effect";

export type LndWalletLocalConfig = Readonly<{
  readonly userDataPath: string;
  readonly defaultWalletState: LndWalletState;
}>;

export const defaultLndWalletLocalConfig = (input: {
  readonly userDataPath: string;
}): LndWalletLocalConfig => ({
  userDataPath: input.userDataPath,
  defaultWalletState: "uninitialized",
});

export class LndWalletLocalConfigService extends Context.Tag("@openagents/desktop/LndWalletLocalConfigService")<
  LndWalletLocalConfigService,
  LndWalletLocalConfig
>() {}

export const LndWalletLocalConfigLive = (config: LndWalletLocalConfig) =>
  Layer.succeed(LndWalletLocalConfigService, config);

type PersistedWalletState = Readonly<{
  readonly version: 1;
  readonly walletState: LndWalletState;
  readonly passphraseHash: string | null;
  readonly restored: boolean;
  readonly updatedAtMs: number;
}>;

const walletStatePath = (userDataPath: string): string =>
  path.join(userDataPath, "lnd", "wallet", "wallet-state.json");

const hashPassphrase = (passphrase: string): string =>
  crypto.createHash("sha256").update(passphrase).digest("hex");

const emptyState = (defaultWalletState: LndWalletState): PersistedWalletState => ({
  version: 1,
  walletState: defaultWalletState,
  passphraseHash: null,
  restored: false,
  updatedAtMs: Date.now(),
});

const parseWalletState = (raw: string): PersistedWalletState => {
  const parsed = JSON.parse(raw) as Partial<PersistedWalletState>;
  const validState: ReadonlyArray<LndWalletState> = ["uninitialized", "initialized", "locked", "unlocked"];
  if (
    !parsed ||
    parsed.version !== 1 ||
    !validState.includes(parsed.walletState as LndWalletState) ||
    typeof parsed.restored !== "boolean" ||
    typeof parsed.updatedAtMs !== "number"
  ) {
    throw new Error("invalid_wallet_state_shape");
  }
  return {
    version: 1,
    walletState: parsed.walletState as LndWalletState,
    passphraseHash: typeof parsed.passphraseHash === "string" ? parsed.passphraseHash : null,
    restored: parsed.restored,
    updatedAtMs: parsed.updatedAtMs,
  };
};

const readWalletState = (statePath: string, defaultWalletState: LndWalletState): PersistedWalletState => {
  if (!fs.existsSync(statePath)) return emptyState(defaultWalletState);
  return parseWalletState(fs.readFileSync(statePath, "utf8"));
};

const writeWalletState = (statePath: string, nextState: PersistedWalletState): void => {
  fs.mkdirSync(path.dirname(statePath), { recursive: true });
  fs.writeFileSync(statePath, `${JSON.stringify(nextState, null, 2)}\n`, "utf8");
};

export const LndWalletLocalServiceLive = Layer.effect(
  LndWalletService,
  Effect.gen(function* () {
    const config = yield* LndWalletLocalConfigService;
    const statePath = walletStatePath(config.userDataPath);

    const withState = <A>(
      f: (state: PersistedWalletState) => A,
    ): Effect.Effect<A, LndWalletOperationError> =>
      Effect.try({
        try: () => f(readWalletState(statePath, config.defaultWalletState)),
        catch: (error) => {
          if (error instanceof LndWalletOperationError) return error;
          return LndWalletOperationError.make({
            operation: "wallet_state_read",
            reason: String(error),
          });
        },
      });

    const persistState = (
      f: (state: PersistedWalletState) => PersistedWalletState,
      operation: string,
    ): Effect.Effect<PersistedWalletState, LndWalletOperationError> =>
      withState(f).pipe(
        Effect.flatMap((nextState) =>
          Effect.try({
            try: () => {
              writeWalletState(statePath, nextState);
              return nextState;
            },
            catch: (error) =>
              LndWalletOperationError.make({
                operation,
                reason: String(error),
              }),
          }),
        ),
      );

    const initializeWallet = (request: LndWalletInitializeRequest) =>
      persistState((current) => {
        if (current.walletState !== "uninitialized") {
          throw LndWalletOperationError.make({
            operation: "initializeWallet",
            reason: `wallet_state_${current.walletState}`,
          });
        }
        return {
          ...current,
          walletState: "locked",
          passphraseHash: hashPassphrase(request.passphrase),
          restored: false,
          updatedAtMs: Date.now(),
        };
      }, "initializeWallet").pipe(Effect.map((next) => next.walletState));

    const unlockWallet = (request: LndWalletUnlockRequest) =>
      persistState((current) => {
        if (current.walletState === "uninitialized") {
          throw LndWalletOperationError.make({
            operation: "unlockWallet",
            reason: "wallet_uninitialized",
          });
        }

        const expectedHash = current.passphraseHash;
        if (!expectedHash || expectedHash !== hashPassphrase(request.passphrase)) {
          throw LndWalletOperationError.make({
            operation: "unlockWallet",
            reason: "invalid_passphrase",
          });
        }

        return {
          ...current,
          walletState: "unlocked",
          updatedAtMs: Date.now(),
        };
      }, "unlockWallet").pipe(Effect.map((next) => next.walletState));

    const restoreWallet = (request: LndWalletRestoreRequest) =>
      persistState((current) => {
        if (request.seedMnemonic.length < 12 || request.seedMnemonic.length > 24) {
          throw LndWalletOperationError.make({
            operation: "restoreWallet",
            reason: "invalid_seed_length",
          });
        }

        return {
          ...current,
          walletState: "locked",
          passphraseHash: hashPassphrase(request.passphrase),
          restored: true,
          updatedAtMs: Date.now(),
        };
      }, "restoreWallet").pipe(Effect.map((next) => next.walletState));

    const lockWallet = () =>
      persistState((current) => {
        if (current.walletState === "uninitialized") {
          return current;
        }

        return {
          ...current,
          walletState: "locked",
          updatedAtMs: Date.now(),
        };
      }, "lockWallet").pipe(Effect.map((next) => next.walletState));

    return LndWalletService.of({
      getWalletState: () => withState((state) => state.walletState),
      initializeWallet,
      unlockWallet,
      restoreWallet,
      lockWallet,
    });
  }),
);
