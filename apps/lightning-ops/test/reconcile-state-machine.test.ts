import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { ApertureConfigCompilerLive } from "../src/compiler/apertureCompiler.js";
import { makeInMemoryControlPlaneHarness } from "../src/controlPlane/inMemory.js";
import { makeInMemoryGatewayHarness } from "../src/gateway/inMemory.js";
import { reconcileAndDeployOnce } from "../src/programs/reconcileAndDeploy.js";

import { makePaywall } from "./fixtures.js";

const runReconcile = (input?: {
  readonly paywalls?: ReadonlyArray<ReturnType<typeof makePaywall>>;
  readonly failStages?: ReadonlyArray<
    "active_lookup" | "apply" | "health" | "challenge" | "proxy" | "rollback"
  >;
}) => {
  const controlPlaneHarness = makeInMemoryControlPlaneHarness({
    paywalls: input?.paywalls ?? [makePaywall("reconcile")],
  });

  const gatewayHarness = makeInMemoryGatewayHarness({
    initialDeployment: {
      deploymentId: "dep_prev",
      configHash: "cfg_prev",
      imageDigest: "sha256:prev",
    },
    ...(input?.failStages ? { failStages: input.failStages } : {}),
  });

  return Effect.gen(function* () {
    const summary = yield* reconcileAndDeployOnce({ requestId: "reconcile:test" }).pipe(
      Effect.provide(controlPlaneHarness.layer),
      Effect.provide(gatewayHarness.layer),
      Effect.provide(ApertureConfigCompilerLive),
    );

    return {
      summary,
      controlPlane: controlPlaneHarness.state,
      gateway: gatewayHarness.state,
    };
  });
};

describe("lightning-ops reconcile state machine", () => {
  it.effect("applies deployment and records health/challenge/proxy checks", () =>
    Effect.gen(function* () {
      const result = yield* runReconcile();

      expect(result.summary.deploymentStatus).toBe("applied");
      expect(result.summary.failureCode).toBeUndefined();
      expect(result.summary.healthOk).toBe(true);
      expect(result.summary.challengeOk).toBe(true);
      expect(result.summary.proxyOk).toBe(true);

      expect(result.controlPlane.deployments).toHaveLength(1);
      expect(result.controlPlane.deployments[0]?.status).toBe("applied");
      expect((result.controlPlane.deployments[0]?.diagnostics as any)?.metadata?.executionPath).toBe(
        "hosted-node",
      );

      const eventTypes = result.controlPlane.events.map((event) => event.eventType);
      expect(eventTypes).toContain("gateway_reconcile_health_ok");
      expect(eventTypes).toContain("gateway_reconcile_challenge_ok");
      expect(eventTypes).toContain("gateway_reconcile_proxy_ok");
    }),
  );

  it.effect("rolls back deterministically when health check fails", () =>
    Effect.gen(function* () {
      const result = yield* runReconcile({ failStages: ["health"] });

      expect(result.summary.deploymentStatus).toBe("rolled_back");
      expect(result.summary.failureCode).toBe("health_check_failed");
      expect(result.summary.rolledBackFrom).toBe("cfg_prev");
      expect(result.summary.healthOk).toBe(false);
      expect(result.summary.challengeOk).toBe(false);
      expect(result.summary.proxyOk).toBe(false);

      expect(result.controlPlane.deployments[0]?.status).toBe("rolled_back");
      const eventTypes = result.controlPlane.events.map((event) => event.eventType);
      expect(eventTypes).toContain("gateway_reconcile_rolled_back");
    }),
  );

  it.effect("emits rollback_failed terminal state when rollback also fails", () =>
    Effect.gen(function* () {
      const result = yield* runReconcile({ failStages: ["health", "rollback"] });

      expect(result.summary.deploymentStatus).toBe("failed");
      expect(result.summary.failureCode).toBe("rollback_failed");
      expect(result.controlPlane.deployments[0]?.status).toBe("failed");

      const eventTypes = result.controlPlane.events.map((event) => event.eventType);
      expect(eventTypes).toContain("gateway_reconcile_failed_rollback_failed");
    }),
  );

  it.effect("fails with compile_validation_failed before deployment when compile is invalid", () =>
    Effect.gen(function* () {
      const invalidPaywall = makePaywall("invalid", { fixedAmountMsats: 0 });
      const result = yield* runReconcile({ paywalls: [invalidPaywall] });

      expect(result.summary.deploymentStatus).toBe("failed");
      expect(result.summary.failureCode).toBe("compile_validation_failed");
      expect(result.summary.configHash.startsWith("cfg_")).toBe(true);
      expect(result.summary.challengeOk).toBe(false);
      expect(result.summary.proxyOk).toBe(false);
      expect(result.controlPlane.events).toHaveLength(0);
    }),
  );
});
