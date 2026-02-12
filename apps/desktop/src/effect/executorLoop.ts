import { Context, Effect, Fiber, Layer, Option, Ref, Schedule } from "effect";

import { DesktopConfigService } from "./config";
import { ConnectivityProbeService } from "./connectivity";
import { DesktopStateService } from "./state";
import { TaskProviderService } from "./taskProvider";
import { DesktopSessionService } from "./session";
import { L402ExecutorService } from "./l402Executor";
import { getOrCreateDesktopDeviceId } from "./deviceId";

import type { ExecutorTask } from "./model";

export type ExecutorLoopApi = Readonly<{
  readonly tick: () => Effect.Effect<void>;
  readonly start: () => Effect.Effect<void>;
  readonly stop: () => Effect.Effect<void>;
}>;

export class ExecutorLoopService extends Context.Tag("@openagents/desktop/ExecutorLoopService")<
  ExecutorLoopService,
  ExecutorLoopApi
>() {}

export const ExecutorLoopLive = Layer.effect(
  ExecutorLoopService,
  Effect.gen(function* () {
    const cfg = yield* DesktopConfigService;
    const connectivity = yield* ConnectivityProbeService;
    const state = yield* DesktopStateService;
    const tasks = yield* TaskProviderService;
    const sessionStore = yield* DesktopSessionService;
    const l402Executor = yield* L402ExecutorService;
    const fiberRef = yield* Ref.make<Fiber.RuntimeFiber<unknown, unknown> | null>(null);
    const deviceId = yield* Effect.sync(() => getOrCreateDesktopDeviceId());

    const setExecutorStatus = Effect.fn("ExecutorLoop.setExecutorStatus")(function* (input: {
      readonly status: "waiting_auth" | "idle" | "running_task" | "completed_task" | "failed_task";
      readonly loop?: "stopped" | "running";
      readonly taskId?: string | null;
      readonly error?: string | null;
      readonly incrementTick?: boolean;
    }) {
      const now = Date.now();
      yield* state.update((current) => ({
        ...current,
        executor: {
          ...current.executor,
          ...(input.loop ? { loop: input.loop } : {}),
          status: input.status,
          lastTaskId: input.taskId ?? current.executor.lastTaskId,
          lastError: input.error ?? null,
          ticks: input.incrementTick ? current.executor.ticks + 1 : current.executor.ticks,
          lastTransitionAtMs: now,
        },
      }));
    });

    const processTask = Effect.fn("ExecutorLoop.processTask")(function* (input: {
      readonly task: ExecutorTask;
      readonly userId: string;
      readonly token: string;
    }) {
      const task = input.task;
      if (task.ownerId !== input.userId) {
        yield* setExecutorStatus({
          status: "failed_task",
          taskId: task.id,
          error: "owner_mismatch",
          incrementTick: true,
        });
        return;
      }

      yield* tasks.transitionTask({
        taskId: task.id,
        token: input.token,
        toStatus: "running",
        reason: "desktop_executor_started",
      });
      yield* setExecutorStatus({
        status: "running_task",
        taskId: task.id,
        incrementTick: true,
      });

      const outcome = yield* l402Executor.execute(task);

      if (outcome.status === "blocked") {
        const reason = outcome.denyReason;
        yield* tasks.transitionTask({
          taskId: task.id,
          token: input.token,
          toStatus: "blocked",
          reason: "policy_blocked",
          errorCode: outcome.errorCode,
          errorMessage: reason,
          metadata: {
            denyReason: reason,
            denyReasonCode: outcome.denyReasonCode,
            host: outcome.host,
            maxSpendMsats: outcome.maxSpendMsats,
            quotedAmountMsats: outcome.quotedAmountMsats,
            paymentBackend: outcome.paymentBackend,
            paid: false,
            cacheHit: false,
          },
        });
        yield* setExecutorStatus({
          status: "failed_task",
          taskId: task.id,
          error: reason,
        });
        return;
      }

      if (outcome.status === "failed") {
        const reason = outcome.denyReason;
        yield* tasks.transitionTask({
          taskId: task.id,
          token: input.token,
          toStatus: "failed",
          reason: "payment_failed",
          errorCode: outcome.errorCode,
          errorMessage: reason,
          metadata: {
            denyReason: reason,
            paymentBackend: outcome.paymentBackend,
            paid: false,
            cacheHit: false,
          },
        });
        yield* setExecutorStatus({
          status: "failed_task",
          taskId: task.id,
          error: reason,
        });
        return;
      }

      if (outcome.status !== "paid" && outcome.status !== "cached") {
        yield* setExecutorStatus({
          status: "failed_task",
          taskId: task.id,
          error: "unexpected_executor_outcome",
        });
        return;
      }

      const metadata = {
        amountMsats: outcome.amountMsats,
        paymentId: outcome.paymentId,
        proofReference: outcome.proofReference,
        responseStatusCode: outcome.responseStatusCode,
        responseContentType: outcome.responseContentType,
        responseBytes: outcome.responseBytes,
        responseBodyTextPreview: outcome.responseBodyTextPreview,
        responseBodySha256: outcome.responseBodySha256,
        cacheHit: outcome.cacheHit,
        paid: outcome.paid,
        cacheStatus: outcome.cacheStatus,
        paymentBackend: outcome.paymentBackend,
      };
      yield* tasks.transitionTask({
        taskId: task.id,
        token: input.token,
        toStatus: outcome.status,
        reason: outcome.status === "paid" ? "payment_success" : "payment_cache_hit",
        metadata,
      });
      yield* tasks.transitionTask({
        taskId: task.id,
        token: input.token,
        toStatus: "completed",
        reason: "desktop_executor_completed",
        metadata,
      });
      yield* setExecutorStatus({
        status: "completed_task",
        taskId: task.id,
        error: null,
      });
    });

    const tick = Effect.fn("ExecutorLoop.tick")(function* () {
      const connectivityStatus = yield* connectivity.probe();
      yield* state.update((current) => ({
        ...current,
        connectivity: {
          openAgentsReachable: connectivityStatus.openAgentsReachable,
          convexReachable: connectivityStatus.convexReachable,
          lastCheckedAtMs: connectivityStatus.checkedAtMs,
        },
      }));

      const snapshot = yield* state.get();
      if (snapshot.auth.status !== "signed_in" || !snapshot.auth.userId) {
        yield* setExecutorStatus({
          status: "waiting_auth",
          incrementTick: true,
          error: null,
        });
        return;
      }

      const session = yield* sessionStore.get();
      if (!session.token || session.userId !== snapshot.auth.userId) {
        yield* setExecutorStatus({
          status: "waiting_auth",
          incrementTick: true,
          error: "session_token_missing",
        });
        return;
      }

      yield* tasks
        .heartbeatExecutorPresence({
          token: session.token,
          deviceId,
          capabilities: ["l402_executor", "spark_payer"],
        })
        .pipe(Effect.catchAll(() => Effect.void));

      const nextTask = yield* tasks.pollPendingTask({
        userId: snapshot.auth.userId,
        token: session.token,
      });
      if (Option.isNone(nextTask)) {
        yield* setExecutorStatus({
          status: "idle",
          incrementTick: true,
          error: null,
        });
        return;
      }

      yield* processTask({
        task: nextTask.value,
        userId: snapshot.auth.userId,
        token: session.token,
      });
    });

    const start = Effect.fn("ExecutorLoop.start")(function* () {
      const existing = yield* Ref.get(fiberRef);
      if (existing) return;

      yield* setExecutorStatus({ status: "idle", loop: "running", error: null });
      const runner = Effect.repeat(
        tick().pipe(
          Effect.catchAll((err) =>
            setExecutorStatus({
              status: "failed_task",
              error: String(err),
              incrementTick: true,
            }),
          ),
        ),
        Schedule.spaced(`${cfg.executorTickMs} millis`),
      );
      const fiber = yield* Effect.forkDaemon(runner);
      yield* Ref.set(fiberRef, fiber);
    });

    const stop = Effect.fn("ExecutorLoop.stop")(function* () {
      const existing = yield* Ref.get(fiberRef);
      if (existing) {
        yield* Fiber.interrupt(existing);
        yield* Ref.set(fiberRef, null);
      }
      yield* setExecutorStatus({
        status: "waiting_auth",
        loop: "stopped",
        error: null,
      });
    });

    const safeTick = () =>
      tick().pipe(
        Effect.catchAll((err) =>
          setExecutorStatus({
            status: "failed_task",
            error: String(err),
            incrementTick: true,
          }),
        ),
      );

    return ExecutorLoopService.of({
      tick: () => safeTick(),
      start: () => start(),
      stop: () => stop(),
    });
  }),
);
