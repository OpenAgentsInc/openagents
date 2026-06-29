// Brain tests: scriptedBrain replays a fixed list; khalaBrain is owner-gated
// and inert by default (throws without an injected driver) so CI never makes a
// live inference call.

import { describe, expect, test } from "bun:test";
import {
  KhalaBrainNotArmedError,
  khalaBrain,
  scriptedBrain,
  type BrainContext,
  type BrainStep,
} from "./brain";

const ctx = (stepIndex: number): BrainContext =>
  ({ stepIndex, browser: {} as BrainContext["browser"] });

describe("scriptedBrain", () => {
  test("replays steps in order then returns null", async () => {
    const steps: BrainStep[] = [
      { kind: "navigate", url: "/login" },
      { kind: "screenshot", label: "s" },
    ];
    const brain = scriptedBrain(steps);
    expect(await brain.next(ctx(0))).toEqual(steps[0]!);
    expect(await brain.next(ctx(1))).toEqual(steps[1]!);
    expect(await brain.next(ctx(2))).toBeNull();
  });
});

describe("khalaBrain (owner-gated)", () => {
  test("is inert by default: throws KhalaBrainNotArmedError", async () => {
    const brain = khalaBrain();
    expect(brain.name).toBe("khala");
    await expect(brain.next(ctx(0))).rejects.toBeInstanceOf(KhalaBrainNotArmedError);
  });

  test("uses an injected driver when armed", async () => {
    const step: BrainStep = { kind: "navigate", url: "/" };
    const brain = khalaBrain({ driver: { next: async () => step } });
    expect(await brain.next(ctx(0))).toEqual(step);
  });
});
