import { Context, Effect, Fiber, Layer, Option, Ref, Schedule } from "effect";

import { DesktopConfigService } from "./config";
import { ConnectivityProbeService } from "./connectivity";
import { DesktopStateService } from "./state";
import { TaskProviderService } from "./taskProvider";

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
    const fiberRef = yield* Ref.make<Fiber.RuntimeFiber<unknown, unknown> | null>(null);

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

    const processTask = Effect.fn("ExecutorLoop.processTask")(function* (task: ExecutorTask) {
      yield* tasks.markRunning(task.id);
      yield* setExecutorStatus({
        status: "running_task",
        taskId: task.id,
        incrementTick: true,
      });

      const shouldFail = task.payload.toLowerCase().includes("fail");
      if (shouldFail) {
        const reason = "demo_failure_requested";
        yield* tasks.markFailed(task.id, reason);
        yield* setExecutorStatus({
          status: "failed_task",
          taskId: task.id,
          error: reason,
        });
        return;
      }

      yield* tasks.markCompleted(task.id);
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

      const nextTask = yield* tasks.pollPendingTask(snapshot.auth.userId);
      if (Option.isNone(nextTask)) {
        yield* setExecutorStatus({
          status: "idle",
          incrementTick: true,
          error: null,
        });
        return;
      }

      yield* processTask(nextTask.value);
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

    return ExecutorLoopService.of({
      tick: () => tick(),
      start: () => start(),
      stop: () => stop(),
    });
  }),
);
