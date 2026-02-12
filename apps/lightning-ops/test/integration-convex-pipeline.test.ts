import { Effect, Ref } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { ApertureConfigCompilerLive } from "../src/compiler/apertureCompiler.js";
import {
  CONVEX_LIST_PAYWALLS_FN,
  CONVEX_RECORD_DEPLOYMENT_FN,
  ConvexControlPlaneLive,
} from "../src/controlPlane/convex.js";
import { makeConvexTransportTestLayer } from "../src/controlPlane/convexTransport.js";
import { compileAndPersistOnce } from "../src/programs/compileAndPersist.js";
import { makeOpsRuntimeConfigTestLayer } from "../src/runtime/config.js";

import { makePaywall } from "./fixtures.js";

describe("lightning-ops convex pipeline", () => {
  it.effect("fetches paywall state -> compiles -> writes deployment intent", () =>
    Effect.gen(function* () {
      const callsRef = yield* Ref.make<Array<{ kind: "query" | "mutation"; fn: string; args: Record<string, unknown> }>>([]);

      const transportLayer = makeConvexTransportTestLayer({
        query: (fn, args) =>
          Effect.gen(function* () {
            yield* Ref.update(callsRef, (calls) => [...calls, { kind: "query" as const, fn, args }]);
            if (fn !== CONVEX_LIST_PAYWALLS_FN) {
              throw new Error(`unexpected query function ${fn}`);
            }
            return {
              ok: true,
              paywalls: [makePaywall("from-convex", { priority: 7 })],
            };
          }),
        mutation: (fn, args) =>
          Effect.gen(function* () {
            yield* Ref.update(callsRef, (calls) => [...calls, { kind: "mutation" as const, fn, args }]);
            if (fn !== CONVEX_RECORD_DEPLOYMENT_FN) {
              throw new Error(`unexpected mutation function ${fn}`);
            }
            return {
              ok: true,
              deployment: {
                deploymentId: "dep_from_convex_1",
                paywallId: undefined,
                ownerId: undefined,
                configHash: String(args.configHash),
                imageDigest: undefined,
                status: String(args.status),
                diagnostics: args.diagnostics,
                appliedAtMs: undefined,
                rolledBackFrom: undefined,
                createdAtMs: 1_733_000_000_000,
                updatedAtMs: 1_733_000_000_001,
              },
            };
          }),
      });

      const summary = yield* compileAndPersistOnce({ requestId: "integration-1" }).pipe(
        Effect.provide(ApertureConfigCompilerLive),
        Effect.provide(ConvexControlPlaneLive),
        Effect.provide(transportLayer),
        Effect.provide(
          makeOpsRuntimeConfigTestLayer({
            convexUrl: "https://example.convex.cloud",
            opsSecret: "ops-secret",
          }),
        ),
      );

      const calls = yield* Ref.get(callsRef);
      expect(summary.valid).toBe(true);
      expect(summary.ruleCount).toBe(1);
      expect(summary.configHash.startsWith("cfg_")).toBe(true);
      expect(summary.deploymentStatus).toBe("pending");

      expect(calls).toHaveLength(2);
      expect(calls[0]).toMatchObject({ kind: "query", fn: CONVEX_LIST_PAYWALLS_FN });
      expect(calls[1]).toMatchObject({ kind: "mutation", fn: CONVEX_RECORD_DEPLOYMENT_FN });
      expect(calls[1]?.args?.configHash).toBe(summary.configHash);
      expect(calls[1]?.args?.status).toBe("pending");
    }),
  );

  it.effect("writes failed deployment intent when compile validation fails", () =>
    Effect.gen(function* () {
      const callsRef = yield* Ref.make<Array<{ kind: "query" | "mutation"; fn: string; args: Record<string, unknown> }>>([]);

      const transportLayer = makeConvexTransportTestLayer({
        query: (fn, args) =>
          Effect.gen(function* () {
            yield* Ref.update(callsRef, (calls) => [...calls, { kind: "query" as const, fn, args }]);
            return {
              ok: true,
              paywalls: [makePaywall("invalid", { fixedAmountMsats: 0 })],
            };
          }),
        mutation: (fn, args) =>
          Effect.gen(function* () {
            yield* Ref.update(callsRef, (calls) => [...calls, { kind: "mutation" as const, fn, args }]);
            return {
              ok: true,
              deployment: {
                deploymentId: "dep_failed_1",
                paywallId: undefined,
                ownerId: undefined,
                configHash: String(args.configHash),
                imageDigest: undefined,
                status: String(args.status),
                diagnostics: args.diagnostics,
                appliedAtMs: undefined,
                rolledBackFrom: undefined,
                createdAtMs: 1_733_000_000_000,
                updatedAtMs: 1_733_000_000_001,
              },
            };
          }),
      });

      const summary = yield* compileAndPersistOnce({ requestId: "integration-2" }).pipe(
        Effect.provide(ApertureConfigCompilerLive),
        Effect.provide(ConvexControlPlaneLive),
        Effect.provide(transportLayer),
        Effect.provide(
          makeOpsRuntimeConfigTestLayer({
            convexUrl: "https://example.convex.cloud",
            opsSecret: "ops-secret",
          }),
        ),
      );

      const calls = yield* Ref.get(callsRef);
      expect(summary.valid).toBe(false);
      expect(summary.ruleCount).toBe(0);
      expect(summary.deploymentStatus).toBe("failed");
      expect(summary.configHash.startsWith("cfg_")).toBe(true);
      expect(calls[1]?.args?.status).toBe("failed");
    }),
  );
});
