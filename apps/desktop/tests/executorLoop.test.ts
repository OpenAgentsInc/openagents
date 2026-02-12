import { Effect, Layer, Option, Ref } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { AuthGatewayService } from "../src/effect/authGateway";
import { ConnectivityProbeService } from "../src/effect/connectivity";
import { DesktopAppService } from "../src/effect/app";
import { makeDesktopLayer } from "../src/effect/layer";
import { L402ExecutorService } from "../src/effect/l402Executor";
import { TaskProviderService } from "../src/effect/taskProvider";
import type { ExecutorTask, ExecutorTaskStatus } from "../src/effect/model";

const authGatewayTestLayer = Layer.succeed(
  AuthGatewayService,
  AuthGatewayService.of({
    startMagicCode: () => Effect.void,
    verifyMagicCode: ({ email }) =>
      Effect.succeed({
        userId: "user_desktop_test",
        token: "token_test",
        user: {
          id: "user_desktop_test",
          email,
          firstName: "Desktop",
          lastName: "Tester",
        },
      }),
    getSession: () =>
      Effect.succeed({
        userId: null,
        token: null,
        user: null,
      }),
  }),
);

const connectivityTestLayer = Layer.succeed(
  ConnectivityProbeService,
  ConnectivityProbeService.of({
    probe: () =>
      Effect.succeed({
        openAgentsReachable: true,
        convexReachable: true,
        checkedAtMs: Date.now(),
      }),
  }),
);

const makeTaskProviderTestLayer = () =>
  Layer.effect(
    TaskProviderService,
    Effect.gen(function* () {
      const ref = yield* Ref.make<ReadonlyArray<ExecutorTask>>([]);

      const transitionTask = (input: {
        readonly taskId: string;
        readonly toStatus: ExecutorTaskStatus;
        readonly errorCode?: string;
        readonly errorMessage?: string;
      }) =>
        Effect.gen(function* () {
          let updated: ExecutorTask | null = null;
          yield* Ref.update(ref, (rows) =>
            rows.map((row) => {
              if (row.id !== input.taskId) return row;
              const next: ExecutorTask = {
                ...row,
                status: input.toStatus,
                updatedAtMs: Date.now(),
                ...(input.errorCode ? { lastErrorCode: input.errorCode } : {}),
                ...(input.errorMessage ? { lastErrorMessage: input.errorMessage, failureReason: input.errorMessage } : {}),
              };
              updated = next;
              return next;
            }),
          );
          if (!updated) {
            return yield* Effect.dieMessage("task_not_found");
          }
          return updated;
        });

      return TaskProviderService.of({
        enqueueDemoTask: ({ payload, token }) =>
          Effect.gen(function* () {
            void token;
            const now = Date.now();
            const task: ExecutorTask = {
              id: crypto.randomUUID(),
              ownerId: payload.includes("foreign") ? "other_user" : "user_desktop_test",
              status: "queued",
              request: {
                url: payload.trim(),
                method: "GET",
                maxSpendMsats: 2_500,
                scope: "test",
              },
              attemptCount: 0,
              createdAtMs: now,
              updatedAtMs: now,
            };
            yield* Ref.update(ref, (rows) => [...rows, task]);
            return task;
          }),
        pollPendingTask: ({ userId, token }) =>
          Effect.gen(function* () {
            void token;
            const rows = yield* Ref.get(ref);
            const pending = rows
              .filter((row) => row.status === "queued" || row.status === "approved")
              .sort((a, b) => a.createdAtMs - b.createdAtMs);
            for (const row of pending) {
              if (row.ownerId === userId || row.request.url.includes("foreign")) return Option.some(row);
            }
            return Option.none<ExecutorTask>();
          }),
        transitionTask: ({ taskId, toStatus, errorCode, errorMessage }) => {
          const input: {
            readonly taskId: string;
            readonly toStatus: ExecutorTaskStatus;
            readonly errorCode?: string;
            readonly errorMessage?: string;
          } = {
            taskId,
            toStatus,
            ...(errorCode ? { errorCode } : {}),
            ...(errorMessage ? { errorMessage } : {}),
          };
          return transitionTask(input);
        },
        listTasks: ({ token }) =>
          Effect.gen(function* () {
            void token;
            const rows = yield* Ref.get(ref);
            return rows;
          }),
      });
    }),
  );

const l402ExecutorTestLayer = Layer.succeed(
  L402ExecutorService,
  L402ExecutorService.of({
    execute: (task) =>
      Effect.sync(() => {
        if (task.request.url.includes("blocked")) {
          return {
            status: "blocked",
            errorCode: "DomainNotAllowedError",
            denyReason: "host_blocked",
            paymentBackend: "spark",
          } as const;
        }
        if (task.request.url.includes("fail")) {
          return {
            status: "failed",
            errorCode: "PaymentFailedError",
            denyReason: "invoice_expired",
            paymentBackend: "spark",
          } as const;
        }
        if (task.request.url.includes("cached")) {
          return {
            status: "cached",
            amountMsats: 2_500,
            paymentId: null,
            proofReference: "preimage:cached",
            responseStatusCode: 200,
            cacheStatus: "hit",
            paymentBackend: "spark",
          } as const;
        }
        return {
          status: "paid",
          amountMsats: 2_500,
          paymentId: "pay_test_1",
          proofReference: "preimage:paid",
          responseStatusCode: 200,
          cacheStatus: "miss",
          paymentBackend: "spark",
        } as const;
      }),
  }),
);

const makeTestLayer = () =>
  makeDesktopLayer(
    {
      openAgentsBaseUrl: "https://openagents.example",
      convexUrl: "https://convex.example",
      executorTickMs: 500,
    },
    {
      authGateway: authGatewayTestLayer,
      connectivity: connectivityTestLayer,
      taskProvider: makeTaskProviderTestLayer(),
      l402Executor: l402ExecutorTestLayer,
    },
  );

describe("desktop executor loop", () => {
  it.effect("stays in waiting_auth when user is not signed in", () =>
    Effect.gen(function* () {
      const app = yield* DesktopAppService;
      yield* app.bootstrap();
      yield* app.tickExecutor();
      const snapshot = yield* app.snapshot();

      expect(snapshot.auth.status).toBe("signed_out");
      expect(snapshot.executor.status).toBe("waiting_auth");
    }).pipe(Effect.provide(makeTestLayer())),
  );

  it.effect("runs queued task to completion after auth", () =>
    Effect.gen(function* () {
      const app = yield* DesktopAppService;
      yield* app.bootstrap();
      yield* app.requestMagicCode("chris@openagents.com");
      yield* app.verifyMagicCode({ email: "chris@openagents.com", code: "123456" });
      yield* app.enqueueDemoTask("https://api.example.com/paid");
      yield* app.tickExecutor();

      const tasks = yield* app.listTasks();
      const snapshot = yield* app.snapshot();

      expect(tasks[0]?.status).toBe("completed");
      expect(snapshot.auth.userId).toBe("user_desktop_test");
      expect(snapshot.executor.status).toBe("completed_task");
    }).pipe(Effect.provide(makeTestLayer())),
  );

  it.effect("marks task failed for payment failures", () =>
    Effect.gen(function* () {
      const app = yield* DesktopAppService;
      yield* app.bootstrap();
      yield* app.verifyMagicCode({ email: "chris@openagents.com", code: "123456" });
      yield* app.enqueueDemoTask("https://api.example.com/fail");
      yield* app.tickExecutor();

      const tasks = yield* app.listTasks();
      const snapshot = yield* app.snapshot();
      expect(tasks[0]?.status).toBe("failed");
      expect(tasks[0]?.failureReason).toBe("invoice_expired");
      expect(snapshot.executor.status).toBe("failed_task");
      expect(snapshot.executor.lastError).toBe("invoice_expired");
    }).pipe(Effect.provide(makeTestLayer())),
  );

  it.effect("maps blocked outcomes and records deny reason", () =>
    Effect.gen(function* () {
      const app = yield* DesktopAppService;
      yield* app.bootstrap();
      yield* app.verifyMagicCode({ email: "chris@openagents.com", code: "123456" });
      yield* app.enqueueDemoTask("https://api.example.com/blocked");
      yield* app.tickExecutor();

      const tasks = yield* app.listTasks();
      const snapshot = yield* app.snapshot();
      expect(tasks[0]?.status).toBe("blocked");
      expect(tasks[0]?.failureReason).toBe("host_blocked");
      expect(snapshot.executor.status).toBe("failed_task");
      expect(snapshot.executor.lastError).toBe("host_blocked");
    }).pipe(Effect.provide(makeTestLayer())),
  );

  it.effect("prevents cross-user task execution when owner mismatches signed-in user", () =>
    Effect.gen(function* () {
      const app = yield* DesktopAppService;
      yield* app.bootstrap();
      yield* app.verifyMagicCode({ email: "chris@openagents.com", code: "123456" });
      yield* app.enqueueDemoTask("https://api.example.com/foreign");
      yield* app.tickExecutor();

      const tasks = yield* app.listTasks();
      const snapshot = yield* app.snapshot();
      expect(tasks[0]?.status).toBe("queued");
      expect(snapshot.executor.status).toBe("failed_task");
      expect(snapshot.executor.lastError).toBe("owner_mismatch");
    }).pipe(Effect.provide(makeTestLayer())),
  );
});
