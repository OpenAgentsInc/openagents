import { Context, Effect, Layer, Option, Ref } from "effect";

import type { ExecutorTask } from "./model";

type MutableTask = {
  id: string;
  payload: string;
  status: ExecutorTask["status"];
  createdAtMs: number;
  updatedAtMs: number;
  failureReason?: string | undefined;
};

export type TaskProviderApi = Readonly<{
  readonly enqueueDemoTask: (payload: string) => Effect.Effect<ExecutorTask>;
  readonly pollPendingTask: (userId: string) => Effect.Effect<Option.Option<ExecutorTask>>;
  readonly markRunning: (taskId: string) => Effect.Effect<void>;
  readonly markCompleted: (taskId: string) => Effect.Effect<void>;
  readonly markFailed: (taskId: string, reason: string) => Effect.Effect<void>;
  readonly listTasks: () => Effect.Effect<ReadonlyArray<ExecutorTask>>;
}>;

export class TaskProviderService extends Context.Tag("@openagents/desktop/TaskProviderService")<
  TaskProviderService,
  TaskProviderApi
>() {}

const toReadonlyTask = (task: MutableTask): ExecutorTask => ({
  id: task.id,
  payload: task.payload,
  status: task.status,
  createdAtMs: task.createdAtMs,
  updatedAtMs: task.updatedAtMs,
  ...(task.failureReason ? { failureReason: task.failureReason } : {}),
});

const updateTask = (tasks: ReadonlyArray<MutableTask>, taskId: string, f: (task: MutableTask) => MutableTask) =>
  tasks.map((task) => (task.id === taskId ? f(task) : task));

export const TaskProviderLive = Layer.effect(
  TaskProviderService,
  Effect.gen(function* () {
    const queueRef = yield* Ref.make<ReadonlyArray<MutableTask>>([]);

    const enqueueDemoTask = Effect.fn("TaskProvider.enqueueDemoTask")(function* (payload: string) {
      const now = Date.now();
      const task: MutableTask = {
        id: crypto.randomUUID(),
        payload: payload.trim().length > 0 ? payload.trim() : "demo-task",
        status: "queued",
        createdAtMs: now,
        updatedAtMs: now,
      };
      yield* Ref.update(queueRef, (tasks) => [...tasks, task]);
      return toReadonlyTask(task);
    });

    const pollPendingTask = Effect.fn("TaskProvider.pollPendingTask")(function* (userId: string) {
      void userId;
      const tasks = yield* Ref.get(queueRef);
      for (const task of tasks) {
        if (task.status === "queued") return Option.some(toReadonlyTask(task));
      }
      return Option.none<ExecutorTask>();
    });

    const markRunning = Effect.fn("TaskProvider.markRunning")(function* (taskId: string) {
      yield* Ref.update(queueRef, (tasks) =>
        updateTask(tasks, taskId, (task) => ({
          ...task,
          status: "running",
          updatedAtMs: Date.now(),
          failureReason: undefined,
        })),
      );
    });

    const markCompleted = Effect.fn("TaskProvider.markCompleted")(function* (taskId: string) {
      yield* Ref.update(queueRef, (tasks) =>
        updateTask(tasks, taskId, (task) => ({
          ...task,
          status: "completed",
          updatedAtMs: Date.now(),
        })),
      );
    });

    const markFailed = Effect.fn("TaskProvider.markFailed")(function* (taskId: string, reason: string) {
      yield* Ref.update(queueRef, (tasks) =>
        updateTask(tasks, taskId, (task) => ({
          ...task,
          status: "failed",
          updatedAtMs: Date.now(),
          failureReason: reason,
        })),
      );
    });

    const listTasks = Effect.fn("TaskProvider.listTasks")(function* () {
      const tasks = yield* Ref.get(queueRef);
      return tasks.map(toReadonlyTask);
    });

    return TaskProviderService.of({
      enqueueDemoTask,
      pollPendingTask,
      markRunning,
      markCompleted,
      markFailed,
      listTasks: () => listTasks(),
    });
  }),
);
