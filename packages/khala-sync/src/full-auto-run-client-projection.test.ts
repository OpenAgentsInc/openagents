import { describe, expect, test } from "vite-plus/test";

import { decodeFullAutoRunClientProjection } from "./full-auto-run-client-projection.js";

const timestamp = "2026-07-17T21:00:00.000Z";

const validRun = {
  runRef: "run.full-auto.abc123.def456",
  threadRef: "thread.abc123",
  objective: "Ship the mobile live-run projection.",
  doneCondition: "The new endpoint round-trips a projection end to end.",
  lifecycleState: "running",
  workspaceLabel: "openagents",
  startedAt: timestamp,
  updatedAt: timestamp,
  lastTransition: { actor: "control_api", at: timestamp },
  laneRef: "codex-local",
  accountRef: null,
  turnCap: 20,
  successfulAttempts: 3,
  failedAttempts: 0,
  rotationCount: 0,
  receiptSummary: null,
};

const digest = "a".repeat(64);
const validReceiptSummary = {
  schema: "full_auto_run.mobile_receipt.v1",
  runRef: validRun.runRef,
  threadRef: validRun.threadRef,
  objectiveDigest: digest,
  doneConditionDigest: digest,
  workspaceRefDigest: digest,
  state: "completed",
  startedAt: timestamp,
  endedAt: timestamp,
  turnCap: 20,
  successfulAttempts: 5,
  failedAttempts: 1,
  providerIdentities: ["codex-local"],
  providerTransitionCount: 0,
  providerTransitionDispositions: [],
  livenessGapCount: 0,
  recoveryActionsUsed: [],
  verifiedRefCount: 0,
  claimedRefCount: 0,
  progressDisposition: "unknown",
  usageKnown: false,
  reportRevision: 1,
  createdAt: timestamp,
};

describe("FullAutoRun client projection", () => {
  test("decodes a running projection", () => {
    const projection = decodeFullAutoRunClientProjection({
      schema: "full_auto_run.mobile_projection.v1",
      privateMaterialExcluded: true,
      generatedAt: timestamp,
      run: validRun,
    });
    expect(projection.run).toMatchObject({ lifecycleState: "running", workspaceLabel: "openagents" });
  });

  test("decodes a null run (no active Full Auto run)", () => {
    const projection = decodeFullAutoRunClientProjection({
      schema: "full_auto_run.mobile_projection.v1",
      privateMaterialExcluded: true,
      generatedAt: timestamp,
      run: null,
    });
    expect(projection.run).toBeNull();
  });

  test("every FullAutoRunStateSchema (FA-RUN-01 #8969) literal round-trips", () => {
    // Keeps this schema's lifecycle enum in lockstep with the registry's
    // FullAutoRunStateSchema (apps/openagents-desktop/src/full-auto-run-registry.ts)
    // without importing the Desktop app package (a cross-app dependency this
    // shared schema package must not take on).
    const states = [
      "draft", "running", "pausing", "paused", "retrying",
      "stalled", "completed", "failed", "stopped", "cap_reached",
    ];
    for (const lifecycleState of states) {
      const projection = decodeFullAutoRunClientProjection({
        schema: "full_auto_run.mobile_projection.v1",
        privateMaterialExcluded: true,
        generatedAt: timestamp,
        run: { ...validRun, lifecycleState },
      });
      expect(projection.run?.lifecycleState).toBe(lifecycleState);
    }
    expect(states.length).toBe(10);
  });

  test("rejects a raw local filesystem path smuggled through workspaceLabel", () => {
    expect(() =>
      decodeFullAutoRunClientProjection({
        schema: "full_auto_run.mobile_projection.v1",
        privateMaterialExcluded: true,
        generatedAt: timestamp,
        run: { ...validRun, workspaceLabel: "/Users/private/repo" },
      }),
    ).toThrow();
  });

  test("rejects an unknown lifecycle state", () => {
    expect(() =>
      decodeFullAutoRunClientProjection({
        schema: "full_auto_run.mobile_projection.v1",
        privateMaterialExcluded: true,
        generatedAt: timestamp,
        run: { ...validRun, lifecycleState: "bogus" },
      }),
    ).toThrow();
  });

  test("rejects excess properties (e.g. an accidentally-included raw prompt field)", () => {
    expect(() =>
      decodeFullAutoRunClientProjection({
        schema: "full_auto_run.mobile_projection.v1",
        privateMaterialExcluded: true,
        generatedAt: timestamp,
        run: { ...validRun, rawPrompt: "do not ever send this" },
      }),
    ).toThrow();
  });

  test("every FullAutoRunActorSchema (#8928/#8994) literal round-trips as lastTransition.actor, including mobile", () => {
    // Keeps this schema's actor enum in lockstep with the registry's
    // FullAutoRunActorSchema (apps/openagents-desktop/src/full-auto-run-registry.ts).
    const actors = [
      "owner_ui", "control_api", "cli", "mcp", "workspace_guard",
      "continuation_cap", "dispatch_failure_limit", "turn_resolution",
      "thread_state_sync", "legacy_migration", "liveness_monitor", "guardrail",
      "mobile",
    ];
    for (const actor of actors) {
      const projection = decodeFullAutoRunClientProjection({
        schema: "full_auto_run.mobile_projection.v1",
        privateMaterialExcluded: true,
        generatedAt: timestamp,
        run: { ...validRun, lastTransition: { actor, at: timestamp } },
      });
      expect(projection.run?.lastTransition.actor).toBe(actor);
    }
    expect(actors.length).toBe(13);
  });

  test("decodes MOB-FA-02 (#8994) lane/account/rotation/cap fields", () => {
    const projection = decodeFullAutoRunClientProjection({
      schema: "full_auto_run.mobile_projection.v1",
      privateMaterialExcluded: true,
      generatedAt: timestamp,
      run: { ...validRun, laneRef: "codex-local", accountRef: "codex-2", rotationCount: 3 },
    });
    expect(projection.run).toMatchObject({
      laneRef: "codex-local",
      accountRef: "codex-2",
      turnCap: 20,
      successfulAttempts: 3,
      failedAttempts: 0,
      rotationCount: 3,
    });
  });

  test("decodes a bounded receiptSummary once a run is terminal", () => {
    const projection = decodeFullAutoRunClientProjection({
      schema: "full_auto_run.mobile_projection.v1",
      privateMaterialExcluded: true,
      generatedAt: timestamp,
      run: { ...validRun, lifecycleState: "completed", receiptSummary: validReceiptSummary },
    });
    expect(projection.run?.receiptSummary).toMatchObject({
      schema: "full_auto_run.mobile_receipt.v1",
      state: "completed",
      successfulAttempts: 5,
    });
  });

  test("rejects a receiptSummary carrying an excess field (e.g. raw prompt text)", () => {
    expect(() =>
      decodeFullAutoRunClientProjection({
        schema: "full_auto_run.mobile_projection.v1",
        privateMaterialExcluded: true,
        generatedAt: timestamp,
        run: {
          ...validRun,
          receiptSummary: { ...validReceiptSummary, rawPrompt: "never" },
        },
      }),
    ).toThrow();
  });
});
