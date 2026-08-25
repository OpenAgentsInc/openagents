import { describe, expect, it } from "vitest";

import { budgetedResult, describeBudget, toolResultBudget } from "../src/coder-tool-budget.js";
import type { ToolFamily } from "../src/coder-tool-families.js";
import { toolFamilyOf } from "../src/coder-tool-families.js";

/** One oversized result, the same one for every family. */
const oversized = "x".repeat(20_000);

/** The characters a family keeps of that result, notice excluded. */
const kept = (family: ToolFamily): number =>
  budgetedResult(oversized, family).replaceAll(/\n\n\[[\s\S]*?\]\n\n/g, "").length;

describe("toolResultBudget", () => {
  it("gives each family its own allowance, in tokens and in characters", () => {
    const hosted = toolResultBudget("default");
    const gemini = toolResultBudget("gemini");
    const local = toolResultBudget("local");

    expect(hosted.characters).not.toBe(gemini.characters);
    expect(gemini.characters).not.toBe(local.characters);
    // Every budget is a token allowance converted through that family's own
    // approximate density, not a character count someone picked.
    for (const budget of [hosted, gemini, local]) {
      expect(budget.characters).toBe(Math.floor(budget.tokens * budget.charactersPerToken));
      expect(budget.substituted).toBe(false);
    }
    expect(gemini.contextWindowTokens).toBeGreaterThan(local.contextWindowTokens);
  });

  it("substitutes the smallest budget for a family it does not know, and says so", () => {
    const unknown = toolResultBudget("mystery-lane" as ToolFamily);

    expect(unknown.substituted).toBe(true);
    expect(unknown.characters).toBe(
      Math.min(
        toolResultBudget("default").characters,
        toolResultBudget("gemini").characters,
        toolResultBudget("local").characters,
      ),
    );
    expect(describeBudget(unknown)).toContain("substituted");
  });

  it("states the approximation rather than implying a token count was measured", () => {
    expect(describeBudget(toolResultBudget("gemini"))).toContain("approximate");
  });
});

describe("budgetedResult", () => {
  it("budgets one oversized result differently for two model families", () => {
    const forGemini = kept("gemini");
    const forHosted = kept("default");

    expect(forGemini).not.toBe(forHosted);
    expect(forGemini).toBeLessThan(oversized.length);
    expect(forHosted).toBeLessThan(oversized.length);
    expect(forGemini).toBeLessThanOrEqual(toolResultBudget("gemini").characters);
    expect(forHosted).toBeLessThanOrEqual(toolResultBudget("default").characters);
  });

  it("reports the cut to both families, with how much went and out of what", () => {
    for (const family of ["gemini", "default", "local"] as const) {
      const budget = toolResultBudget(family);
      const result = budgetedResult(oversized, family);
      const omitted = oversized.length - budget.characters;

      expect(result).toContain(`${String(omitted)} of ${String(oversized.length)} characters`);
      expect(result).toContain(`${String(budget.characters)} characters for the ${family}`);
      expect(result).toContain("must not be summarized as if it were the whole answer");
    }
  });

  it("keeps both ends of what it does show", () => {
    const output = `${"head".padEnd(4_000, "h")}${"tail".padStart(4_000, "t")}`;
    const result = budgetedResult(output, "default");

    expect(result.startsWith("head")).toBe(true);
    expect(result.endsWith("tail")).toBe(true);
  });

  it("leaves a result inside the budget exactly as the tool produced it", () => {
    const small = "the command succeeded";

    expect(budgetedResult(small, "local")).toBe(small);
    expect(budgetedResult(small, "gemini")).toBe(small);
  });

  it("budgets the lanes the catalog actually runs, through their families", () => {
    // The catalog names lanes, not families, so the two steps compose: a lane
    // resolves to a family and the family carries the allowance.
    const gemini = budgetedResult(oversized, toolFamilyOf("gemini-3.7-flash"));
    const luna = budgetedResult(oversized, toolFamilyOf("gpt-5.6-luna"));
    const ox = budgetedResult(oversized, toolFamilyOf("ox-alpha"));

    expect(gemini.length).not.toBe(luna.length);
    expect(luna.length).toBe(ox.length);
  });
});
