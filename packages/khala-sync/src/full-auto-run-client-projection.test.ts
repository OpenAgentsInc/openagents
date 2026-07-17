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
});
