import { Effect, Layer } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { AuthGatewayService } from "../src/effect/authGateway";
import { ConnectivityProbeService } from "../src/effect/connectivity";
import { DesktopAppService } from "../src/effect/app";
import { makeDesktopLayer } from "../src/effect/layer";

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

const testLayer = makeDesktopLayer(
  {
    openAgentsBaseUrl: "https://openagents.example",
    convexUrl: "https://convex.example",
    executorTickMs: 500,
  },
  {
    authGateway: authGatewayTestLayer,
    connectivity: connectivityTestLayer,
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
    }).pipe(Effect.provide(testLayer)),
  );

  it.effect("runs queued task to completion after auth", () =>
    Effect.gen(function* () {
      const app = yield* DesktopAppService;
      yield* app.bootstrap();
      yield* app.requestMagicCode("chris@openagents.com");
      yield* app.verifyMagicCode({ email: "chris@openagents.com", code: "123456" });
      yield* app.enqueueDemoTask("demo-success");
      yield* app.tickExecutor();

      const tasks = yield* app.listTasks();
      const snapshot = yield* app.snapshot();

      expect(tasks[0]?.status).toBe("completed");
      expect(snapshot.auth.userId).toBe("user_desktop_test");
      expect(snapshot.executor.status).toBe("completed_task");
    }).pipe(Effect.provide(testLayer)),
  );

  it.effect("marks task failed when payload requests failure", () =>
    Effect.gen(function* () {
      const app = yield* DesktopAppService;
      yield* app.bootstrap();
      yield* app.verifyMagicCode({ email: "chris@openagents.com", code: "123456" });
      yield* app.enqueueDemoTask("please-fail-demo");
      yield* app.tickExecutor();

      const tasks = yield* app.listTasks();
      const snapshot = yield* app.snapshot();
      expect(tasks[0]?.status).toBe("failed");
      expect(snapshot.executor.status).toBe("failed_task");
      expect(snapshot.executor.lastError).toBe("demo_failure_requested");
    }).pipe(Effect.provide(testLayer)),
  );
});
