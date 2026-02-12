import * as Fs from "node:fs/promises";
import * as Os from "node:os";
import * as Path from "node:path";

import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { runFullFlowSmoke } from "../src/programs/fullFlow.js";

const mkdtemp = () =>
  Effect.tryPromise({
    try: () => Fs.mkdtemp(Path.join(Os.tmpdir(), "openagents-full-flow-")),
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  });

const writeJson = (path: string, value: unknown) =>
  Effect.tryPromise({
    try: async () => {
      await Fs.mkdir(Path.dirname(path), { recursive: true });
      await Fs.writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
    },
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  });

const readText = (path: string) =>
  Effect.tryPromise({
    try: () => Fs.readFile(path, "utf8"),
    catch: (error) => (error instanceof Error ? error : new Error(String(error))),
  });

describe("lightning-ops smoke:full-flow", () => {
  it.effect("mock mode emits summary/events and enforces local parity keys", () =>
    Effect.gen(function* () {
      const tempRoot = yield* mkdtemp();
      const artifactDir = Path.join(tempRoot, "hosted");
      const localArtifactPath = Path.join(tempRoot, "local-node-artifact.json");
      yield* writeJson(localArtifactPath, {
        generatedAtMs: 1_736_000_000_000,
        flows: [
          {
            flow: "success",
            taskId: "task_local_1",
            createRequestId: "req_local_1",
            proofReference: "lightning_preimage:abc",
            observabilityRecords: [
              {
                requestId: "req_local_1",
                taskId: "task_local_1",
                paymentProofRef: "lightning_preimage:abc",
                executionPath: "local-node",
              },
            ],
          },
        ],
      });

      const summary = yield* runFullFlowSmoke({
        mode: "mock",
        requestId: "smoke:full-flow:test",
        artifactDir,
        localArtifactPath,
        strictLocalParity: true,
      });

      expect(summary.ok).toBe(true);
      expect(summary.gatewayReconcile.challengeOk).toBe(true);
      expect(summary.gatewayReconcile.proxyOk).toBe(true);
      expect(summary.policyDeniedRequest.status).toBe("denied");
      expect(summary.parity.localArtifactPresent).toBe(true);
      expect(summary.parity.hostedMissingKeys).toEqual([]);
      expect(summary.parity.localMissingKeys).toEqual([]);
      expect(summary.coverage.failedChecks).toEqual([]);

      const eventsText = yield* readText(summary.artifacts.eventsPath);
      expect(eventsText).toContain("\"stage\":\"gateway.reconcile\"");

      const summaryText = yield* readText(summary.artifacts.summaryPath);
      const persistedSummary = JSON.parse(summaryText) as { requestId: string; ok: boolean };
      expect(persistedSummary.requestId).toBe("smoke:full-flow:test");
      expect(persistedSummary.ok).toBe(true);
    }),
  );

  it.effect("fails in strict mode when local-node parity artifact is missing", () =>
    Effect.gen(function* () {
      const tempRoot = yield* mkdtemp();
      const attempted = yield* Effect.either(
        runFullFlowSmoke({
          mode: "mock",
          requestId: "smoke:full-flow:missing-local",
          artifactDir: Path.join(tempRoot, "hosted"),
          localArtifactPath: Path.join(tempRoot, "missing.json"),
          strictLocalParity: true,
        }),
      );

      expect(attempted._tag).toBe("Left");
      if (attempted._tag === "Left") {
        expect(String(attempted.left)).toContain("full_flow_local_artifact_missing");
      }
    }),
  );
});
