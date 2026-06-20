import { describe, expect, test } from "vitest";

import {
  closePattern,
  middlePattern,
  openPattern,
  scanForConflictMarkers,
  scanText,
} from "./check-conflict-markers.mjs";

// Build marker lines dynamically so this test file never contains a real
// committed conflict marker at column 0.
const open = "<".repeat(7);
const middle = "=".repeat(7);
const close = ">".repeat(7);

describe("conflict-marker guard", () => {
  test("flags a full conflict block with file and line numbers", () => {
    const text = [
      "const a = 1",
      `${open} HEAD`,
      "const b = 2",
      middle,
      "const b = 3",
      `${close} origin/feature`,
      "const c = 4",
    ].join("\n");

    const findings = scanText(text, "example.ts");

    expect(findings.map((f) => f.lineNumber)).toEqual([2, 4, 6]);
    expect(findings.every((f) => f.path === "example.ts")).toBe(true);
  });

  test("does NOT flag a bare ======= that is a Markdown Setext heading rule", () => {
    // A real false positive seen in docs/nips/*.md: a 7-char title underlined
    // with exactly seven '='. There is no surrounding conflict block, so it
    // must not be reported.
    const text = ["NIP-SKL", middle, "", "Agent Skill Registry"].join("\n");

    expect(scanText(text)).toEqual([]);
  });

  test("DOES flag ======= when it sits inside an open conflict block", () => {
    const text = [`${open} HEAD`, "a", middle, "b", `${close} branch`].join(
      "\n",
    );

    expect(scanText(text).map((f) => f.lineNumber)).toEqual([1, 3, 5]);
  });

  test("does not flag clean source or marker-like content not at column 0", () => {
    const text = [
      "const arrow = () => 1",
      `  // ${open} indented label is not a marker`,
      "const eq = a === b",
      "const shift = a >> b",
      `const note = "${middle} inside a string is fine"`,
    ].join("\n");

    expect(scanText(text)).toEqual([]);
  });

  test("requires a label after <<<<<<< and >>>>>>>", () => {
    expect(openPattern.test(open)).toBe(false);
    expect(openPattern.test(`${open} HEAD`)).toBe(true);
    expect(closePattern.test(`${close} branch`)).toBe(true);
    expect(middlePattern.test(middle)).toBe(true);
    expect(middlePattern.test(`${middle} trailing`)).toBe(false);
  });

  test("the real repo source trees are free of conflict markers", () => {
    const findings = scanForConflictMarkers();

    expect(findings.map((f) => `${f.path}:${f.lineNumber}`)).toEqual([]);
  });
});
