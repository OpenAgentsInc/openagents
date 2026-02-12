import { Context, Effect, Layer } from "effect";

import { AuthGatewayService } from "./authGateway";
import { ConnectivityProbeService } from "./connectivity";
import { DesktopStateService } from "./state";
import { ExecutorLoopService } from "./executorLoop";
import { TaskProviderService } from "./taskProvider";
import { LndRuntimeGatewayService } from "./lndRuntimeGateway";
import { LndWalletGatewayService } from "./lndWalletGateway";
import { DesktopSessionService } from "./session";

import type { DesktopRuntimeState, ExecutorTask } from "./model";

export type DesktopAppApi = Readonly<{
  readonly bootstrap: () => Effect.Effect<void, unknown>;
  readonly requestMagicCode: (email: string) => Effect.Effect<void, unknown>;
  readonly verifyMagicCode: (input: { readonly email: string; readonly code: string }) => Effect.Effect<void, unknown>;
  readonly signOut: () => Effect.Effect<void, unknown>;
  readonly startExecutor: () => Effect.Effect<void, unknown>;
  readonly stopExecutor: () => Effect.Effect<void, unknown>;
  readonly tickExecutor: () => Effect.Effect<void, unknown>;
  readonly startLndRuntime: () => Effect.Effect<void, unknown>;
  readonly stopLndRuntime: () => Effect.Effect<void, unknown>;
  readonly restartLndRuntime: () => Effect.Effect<void, unknown>;
  readonly initializeWallet: (input: {
    readonly passphrase: string;
    readonly seedMnemonic?: ReadonlyArray<string>;
  }) => Effect.Effect<void, unknown>;
  readonly unlockWallet: (input?: { readonly passphrase?: string }) => Effect.Effect<void, unknown>;
  readonly lockWallet: () => Effect.Effect<void, unknown>;
  readonly acknowledgeSeedBackup: () => Effect.Effect<void, unknown>;
  readonly prepareWalletRestore: () => Effect.Effect<void, unknown>;
  readonly restoreWallet: (input: {
    readonly passphrase: string;
    readonly seedMnemonic: ReadonlyArray<string>;
    readonly recoveryWindowDays?: number;
  }) => Effect.Effect<void, unknown>;
  readonly enqueueDemoTask: (payload: string) => Effect.Effect<ExecutorTask, unknown>;
  readonly listTasks: () => Effect.Effect<ReadonlyArray<ExecutorTask>, unknown>;
  readonly snapshot: () => Effect.Effect<DesktopRuntimeState, unknown>;
}>;

export class DesktopAppService extends Context.Tag("@openagents/desktop/DesktopAppService")<
  DesktopAppService,
  DesktopAppApi
>() {}

const asMessage = (value: unknown): string => {
  if (value instanceof Error) return value.message;
  return String(value);
};

const normalizeEmail = (input: string): string => input.trim().toLowerCase();

export const DesktopAppLive = Layer.effect(
  DesktopAppService,
  Effect.gen(function* () {
    const auth = yield* AuthGatewayService;
    const connectivity = yield* ConnectivityProbeService;
    const state = yield* DesktopStateService;
    const executor = yield* ExecutorLoopService;
    const tasks = yield* TaskProviderService;
    const lndRuntime = yield* LndRuntimeGatewayService;
    const lndWallet = yield* LndWalletGatewayService;
    const sessionStore = yield* DesktopSessionService;

    const refreshConnectivity = Effect.fn("DesktopApp.refreshConnectivity")(function* () {
      const result = yield* connectivity.probe();
      yield* state.update((current) => ({
        ...current,
        connectivity: {
          openAgentsReachable: result.openAgentsReachable,
          convexReachable: result.convexReachable,
          lastCheckedAtMs: result.checkedAtMs,
        },
      }));
    });

    const refreshLndRuntime = Effect.fn("DesktopApp.refreshLndRuntime")(function* () {
      const status = yield* lndRuntime.snapshot();
      yield* state.update((current) => ({
        ...current,
        lnd: {
          lifecycle: status.lifecycle,
          health: status.health,
          target: status.target,
          pid: status.pid,
          restartCount: status.restartCount,
          crashCount: status.crashCount,
          nextRestartAtMs: status.nextRestartAtMs,
          lastHealthCheckAtMs: status.lastHealthCheckAtMs,
          lastError: status.lastError,
          sync: {
            source: status.sync.source,
            blockHeight: status.sync.blockHeight,
            numPeers: status.sync.numPeers,
            bestHeaderTimestamp: status.sync.bestHeaderTimestamp,
            syncedToChain: status.sync.syncedToChain,
            syncedToGraph: status.sync.syncedToGraph,
            walletSynced: status.sync.walletSynced,
            lastUpdatedAtMs: status.sync.lastUpdatedAtMs,
            lastError: status.sync.lastError,
          },
        },
      }));
    });

    const refreshWallet = Effect.fn("DesktopApp.refreshWallet")(function* () {
      const status = yield* lndWallet.snapshot();
      yield* state.update((current) => ({
        ...current,
        wallet: {
          walletState: status.walletState,
          recoveryState: status.recoveryState,
          seedBackupAcknowledged: status.seedBackupAcknowledged,
          passphraseStored: status.passphraseStored,
          restorePrepared: status.restorePrepared,
          lastErrorCode: status.lastErrorCode,
          lastErrorMessage: status.lastErrorMessage,
          lastOperation: status.lastOperation,
          updatedAtMs: status.updatedAtMs,
        },
      }));
    });

    const bootstrap = Effect.fn("DesktopApp.bootstrap")(function* () {
      yield* refreshConnectivity();
      yield* refreshLndRuntime();
      yield* refreshWallet();
      const existingSession = yield* sessionStore.get();
      const session = yield* auth.getSession(existingSession.token).pipe(
        Effect.catchAll(() =>
          Effect.succeed({
            userId: null,
            token: existingSession.token,
            user: null,
          }),
        ),
      );

      const nextToken = session.token ?? existingSession.token;
      yield* sessionStore.set({
        userId: session.userId,
        token: nextToken,
      });
      yield* state.update((current) => ({
        ...current,
        auth: {
          status: session.userId ? "signed_in" : "signed_out",
          userId: session.userId,
          email: session.user?.email ?? null,
          tokenPresent: Boolean(nextToken),
          lastError: null,
        },
      }));
    });

    const requestMagicCode = Effect.fn("DesktopApp.requestMagicCode")(function* (email: string) {
      const normalized = normalizeEmail(email);
      yield* auth.startMagicCode(normalized);
      yield* state.update((current) => ({
        ...current,
        auth: {
          ...current.auth,
          status: "code_requested",
          email: normalized,
          lastError: null,
        },
      }));
    });

    const verifyMagicCode = Effect.fn("DesktopApp.verifyMagicCode")(function* (input: {
      readonly email: string;
      readonly code: string;
    }) {
      const normalizedEmail = normalizeEmail(input.email);
      const verified = yield* auth.verifyMagicCode({
        email: normalizedEmail,
        code: input.code.trim(),
      });
      yield* sessionStore.set({
        userId: verified.userId,
        token: verified.token,
      });
      yield* refreshConnectivity();
      yield* refreshLndRuntime();
      yield* refreshWallet();
      yield* state.update((current) => ({
        ...current,
        auth: {
          status: "signed_in",
          userId: verified.userId,
          email: verified.user?.email ?? normalizedEmail,
          tokenPresent: Boolean(verified.token),
          lastError: null,
        },
      }));
    });

    const signOut = Effect.fn("DesktopApp.signOut")(function* () {
      yield* sessionStore.clear();
      yield* state.update((current) => ({
        ...current,
        auth: {
          status: "signed_out",
          userId: null,
          email: null,
          tokenPresent: false,
          lastError: null,
        },
      }));
    });

    const guarded = <A>(label: string, effect: Effect.Effect<A, unknown>) =>
      effect.pipe(
        Effect.catchAll((error) =>
          state
            .update((current) => ({
              ...current,
              auth: {
                ...current.auth,
                lastError: `${label}: ${asMessage(error)}`,
              },
            }))
            .pipe(Effect.zipRight(Effect.fail(error))),
        ),
      );

    const walletAction = (label: string, effect: Effect.Effect<void, unknown>) =>
      effect.pipe(
        Effect.catchAll((error) =>
          refreshWallet().pipe(
            Effect.zipRight(
              state.update((current) => ({
                ...current,
                auth: {
                  ...current.auth,
                  lastError: `${label}: ${asMessage(error)}`,
                },
              })),
            ),
            Effect.zipRight(Effect.fail(error)),
          ),
        ),
        Effect.zipRight(refreshWallet()),
      );

    return DesktopAppService.of({
      bootstrap: () => guarded("bootstrap", bootstrap()),
      requestMagicCode: (email) => guarded("requestMagicCode", requestMagicCode(email)),
      verifyMagicCode: (input) => guarded("verifyMagicCode", verifyMagicCode(input)),
      signOut: () => guarded("signOut", signOut()),
      startExecutor: () => executor.start(),
      stopExecutor: () => executor.stop(),
      tickExecutor: () =>
        executor.tick().pipe(
          Effect.zipRight(refreshLndRuntime()),
        ),
      startLndRuntime: () =>
        guarded(
          "startLndRuntime",
          lndRuntime.start().pipe(
            Effect.zipRight(refreshLndRuntime()),
          ),
        ),
      stopLndRuntime: () =>
        guarded(
          "stopLndRuntime",
          lndRuntime.stop().pipe(
            Effect.zipRight(refreshLndRuntime()),
          ),
        ),
      restartLndRuntime: () =>
        guarded(
          "restartLndRuntime",
          lndRuntime.restart().pipe(
            Effect.zipRight(refreshLndRuntime()),
          ),
        ),
      initializeWallet: (input) =>
        walletAction(
          "initializeWallet",
          lndWallet.initialize(input),
        ),
      unlockWallet: (input) =>
        walletAction(
          "unlockWallet",
          lndWallet.unlock(input),
        ),
      lockWallet: () =>
        walletAction(
          "lockWallet",
          lndWallet.lock(),
        ),
      acknowledgeSeedBackup: () =>
        walletAction(
          "acknowledgeSeedBackup",
          lndWallet.acknowledgeSeedBackup(),
        ),
      prepareWalletRestore: () =>
        walletAction(
          "prepareWalletRestore",
          lndWallet.prepareRestore(),
        ),
      restoreWallet: (input) =>
        walletAction(
          "restoreWallet",
          lndWallet.restore(input),
        ),
      enqueueDemoTask: (payload) =>
        Effect.gen(function* () {
          const session = yield* sessionStore.get();
          if (!session.token) return yield* Effect.fail(new Error("missing_auth_token"));
          return yield* tasks.enqueueDemoTask({
            payload,
            token: session.token,
          });
        }),
      listTasks: () =>
        Effect.gen(function* () {
          const session = yield* sessionStore.get();
          if (!session.token) return [] as const;
          return yield* tasks.listTasks({
            token: session.token,
            limit: 100,
          });
        }),
      snapshot: () => state.get(),
    });
  }),
);
