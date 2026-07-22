import { describe, expect, test } from "vite-plus/test";

import { checkIdePortableModel, transitionIdePortableModel } from "./ide13-model.js";

describe("IDE-13 bounded transition model", () => {
  test("exhausts move, replay, crash, stale-writer, cancel, stop, and failback states with no counterexample", () => {
    const receipt = checkIdePortableModel({ maximumDepth: 12 });
    expect(receipt.passed).toBe(true);
    expect(receipt.exploredStates).toBeGreaterThanOrEqual(9);
    expect(receipt.exploredTransitions).toBeGreaterThan(100);
    expect(receipt.replayTransitions).toBeGreaterThan(0);
    expect(receipt.crashTransitions).toBeGreaterThan(0);
    expect(receipt.staleWriteAttempts).toBeGreaterThan(0);
  });

  test("reports a concrete trace when a destination attaches before source revocation", () => {
    const receipt = checkIdePortableModel({
      maximumDepth: 8,
      transition: (state, action) => {
        if (
          action === "attach_destination" &&
          state.phase === "destination_staged" &&
          state.destinationGeneration !== null
        ) {
          return {
            state: {
              ...state,
              phase: "destination_attached",
              destinationAccepting: true,
              latestGeneration: state.destinationGeneration,
            },
            accepted: true,
            reason: "unsafe fixture",
          };
        }
        return transitionIdePortableModel(state, action);
      },
    });
    expect(receipt.passed).toBe(false);
    expect(
      receipt.counterexamples.some((value) => value.invariant === "source_revoked_before_attach"),
    ).toBe(true);
    expect(
      receipt.counterexamples.find((value) => value.invariant === "source_revoked_before_attach")
        ?.trace,
    ).toContain("attach_destination");
  });
});
