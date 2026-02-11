import { Context, Effect, Layer, Ref } from "effect";

import { AuthGatewayService } from "./authGateway";
import { ConnectivityProbeService } from "./connectivity";
import { DesktopStateService } from "./state";
import { ExecutorLoopService } from "./executorLoop";
import { TaskProviderService } from "./taskProvider";

import type { DesktopRuntimeState, ExecutorTask } from "./model";

export type DesktopAppApi = Readonly<{
  readonly bootstrap: () => Effect.Effect<void, unknown>;
  readonly requestMagicCode: (email: string) => Effect.Effect<void, unknown>;
  readonly verifyMagicCode: (input: { readonly email: string; readonly code: string }) => Effect.Effect<void, unknown>;
  readonly signOut: () => Effect.Effect<void, unknown>;
  readonly startExecutor: () => Effect.Effect<void, unknown>;
  readonly stopExecutor: () => Effect.Effect<void, unknown>;
  readonly tickExecutor: () => Effect.Effect<void, unknown>;
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
    const tokenRef = yield* Ref.make<string | null>(null);

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

    const bootstrap = Effect.fn("DesktopApp.bootstrap")(function* () {
      yield* refreshConnectivity();
      const token = yield* Ref.get(tokenRef);
      const session = yield* auth.getSession(token).pipe(
        Effect.catchAll(() =>
          Effect.succeed({
            userId: null,
            token: token,
            user: null,
          }),
        ),
      );

      const nextToken = session.token ?? token;
      yield* Ref.set(tokenRef, nextToken);
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
      yield* Ref.set(tokenRef, verified.token);
      yield* refreshConnectivity();
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
      yield* Ref.set(tokenRef, null);
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

    return DesktopAppService.of({
      bootstrap: () => guarded("bootstrap", bootstrap()),
      requestMagicCode: (email) => guarded("requestMagicCode", requestMagicCode(email)),
      verifyMagicCode: (input) => guarded("verifyMagicCode", verifyMagicCode(input)),
      signOut: () => guarded("signOut", signOut()),
      startExecutor: () => executor.start(),
      stopExecutor: () => executor.stop(),
      tickExecutor: () => executor.tick(),
      enqueueDemoTask: tasks.enqueueDemoTask,
      listTasks: () => tasks.listTasks(),
      snapshot: () => state.get(),
    });
  }),
);
