import { describe, expect, test } from "vite-plus/test";

import { DEFAULT_CANDIDATE_CAP } from "../contract/index.js";
import { honestProgram } from "../test-support.js";
import { generateCandidates, type CandidateKnobs } from "./search.js";

const base = honestProgram("Base instruction.");

const knobs = (instructions: ReadonlyArray<string>): CandidateKnobs => ({
  instructions,
  fewShotSets: [],
  modelRoles: [],
  decodePolicies: [],
});

describe("deterministic bounded candidate generation", () => {
  test("the default candidate cap is the terminal DSE cap of 128", () => {
    expect(DEFAULT_CANDIDATE_CAP).toBe(128);
  });

  test("is a pure function of its inputs", () => {
    const args = {
      algorithm: "instruction_grid.v1" as const,
      base,
      knobs: knobs(["a", "b", "c"]),
      cap: 128,
    };
    expect(generateCandidates(args)).toEqual(generateCandidates(args));
  });

  test("keeps the base program first and truncates to the cap", () => {
    const instructions = Array.from({ length: 50 }, (_, index) => `variant ${index}`);
    const candidates = generateCandidates({
      algorithm: "instruction_grid.v1",
      base,
      knobs: knobs(instructions),
      cap: 5,
    });
    expect(candidates).toHaveLength(5);
    expect(candidates[0]).toEqual(base);
  });

  test("removes duplicate candidates by canonical bytes", () => {
    const candidates = generateCandidates({
      algorithm: "instruction_grid.v1",
      base,
      knobs: knobs(["Base instruction.", "Base instruction.", "unique"]),
      cap: 128,
    });
    // base + the one unique variant; the duplicate of base collapses.
    expect(candidates).toHaveLength(2);
  });

  test("the knob grid honors the cap across the full cartesian product", () => {
    const candidates = generateCandidates({
      algorithm: "knobs_grid.v1",
      base,
      knobs: {
        instructions: ["a", "b", "c", "d"],
        fewShotSets: [[], []],
        modelRoles: ["r1", "r2"],
        decodePolicies: [
          { maxRepairs: 0, maxOutputChars: 1000 },
          { maxRepairs: 1, maxOutputChars: 1000 },
        ],
      },
      cap: 7,
    });
    expect(candidates).toHaveLength(7);
    expect(candidates[0]).toEqual(base);
  });
});
