import { describe, expect, it } from "vite-plus/test";

import {
  applySandboxModelEvent,
  advanceSandboxModelGeneration,
  enumerateSandboxModel,
  initialSandboxModelState,
  sandboxInvariantViolations,
  SandboxTransitionRefused,
} from "./lifecycle.ts";

const event = (kind: Parameters<typeof applySandboxModelEvent>[1]["kind"], sequence: number) => ({
  kind,
  resourceGeneration: 0,
  sequence,
});

describe("managed sandbox lifecycle model", () => {
  it("stops only after a durable filesystem checkpoint and deletes only after observed cleanup", () => {
    let state = initialSandboxModelState();
    state = applySandboxModelEvent(state, event("GuestReady", 1));
    state = applySandboxModelEvent(state, event("StopRequested", 2));

    expect(() => applySandboxModelEvent(state, event("GuestStopped", 3))).toThrow(
      SandboxTransitionRefused,
    );

    state = applySandboxModelEvent(state, event("FilesystemCheckpointed", 3));
    state = applySandboxModelEvent(state, event("GuestStopped", 4));
    state = applySandboxModelEvent(state, event("DeleteRequested", 5));

    expect(state.cleanupComplete).toBe(false);
    state = applySandboxModelEvent(state, event("CleanupObserved", 6));
    expect(state.lifecycle).toBe("deleted");
    expect(state.cleanupComplete).toBe(true);
    expect(state.guestState).toBe("absent");
  });

  it("refuses generation mismatch, stale events, and sequence gaps", () => {
    const initial = initialSandboxModelState(4);
    expect(() =>
      applySandboxModelEvent(initial, {
        kind: "GuestReady",
        resourceGeneration: 3,
        sequence: 1,
      }),
    ).toThrowError(/generation/);
    expect(() =>
      applySandboxModelEvent(initial, {
        kind: "GuestReady",
        resourceGeneration: 4,
        sequence: 2,
      }),
    ).toThrowError(/sequence/);
  });

  it("fences resume into one fresh generation before guest readiness", () => {
    let state = initialSandboxModelState(4);
    state = applySandboxModelEvent(state, {
      kind: "GuestReady",
      resourceGeneration: 4,
      sequence: 1,
    });
    state = applySandboxModelEvent(state, {
      kind: "StopRequested",
      resourceGeneration: 4,
      sequence: 2,
    });
    state = applySandboxModelEvent(state, {
      kind: "FilesystemCheckpointed",
      resourceGeneration: 4,
      sequence: 3,
    });
    state = applySandboxModelEvent(state, {
      kind: "GuestStopped",
      resourceGeneration: 4,
      sequence: 4,
    });
    state = applySandboxModelEvent(state, {
      kind: "ResumeRequested",
      resourceGeneration: 4,
      sequence: 5,
    });

    expect(() => advanceSandboxModelGeneration(state, 6)).toThrowError(/expected generation 5/);
    state = advanceSandboxModelGeneration(state, 5);
    state = applySandboxModelEvent(state, {
      kind: "GuestReady",
      resourceGeneration: 5,
      sequence: 6,
    });
    expect(state).toMatchObject({
      lifecycle: "ready",
      resourceGeneration: 5,
      acceptingWork: true,
    });
  });

  it("keeps every reachable state valid in the bounded graph", () => {
    const states = enumerateSandboxModel(10);
    expect(states.length).toBeGreaterThan(10);
    for (const state of states) {
      expect(sandboxInvariantViolations(state)).toEqual([]);
    }
    expect(states.some((state) => state.lifecycle === "deleted")).toBe(true);
    expect(states.some((state) => state.lifecycle === "recovery_required")).toBe(true);
  });
});
