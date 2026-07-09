/**
 * SQ-5 (#8622): sales-quality eval pack oracles.
 *
 * Deterministic guards over fixture transcripts for the seven sales-quality
 * dimensions (pain-hunting, mirroring, one-product strike, momentum, voice
 * length, non-pushy account/funding move, human-handoff briefs), plus the
 * persona-contract check that `agent/instructions.md` still carries the lines
 * those guards enforce. Rubrics for the judgment residue live in
 * `evals/sarah-sales-quality-fixtures.json`; an LLM judge may add signal but
 * never replaces these hard checks.
 */

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  evaluateSalesQualityDimension,
  evaluateSalesQualityTranscript,
  SALES_QUALITY_DIMENSIONS,
  SALES_QUALITY_INSTRUCTION_LINES,
  salesQualityFixturesSchema,
  SARAH_VOICE_WORD_CAP,
  wordCount,
} from "./sales-quality.ts";

const HERE = dirname(fileURLToPath(import.meta.url));

const fixtures = salesQualityFixturesSchema.parse(
  JSON.parse(
    readFileSync(
      resolve(HERE, "../../evals/sarah-sales-quality-fixtures.json"),
      "utf8",
    ),
  ),
);

const instructions = readFileSync(
  resolve(HERE, "../../agent/instructions.md"),
  "utf8",
);

describe("SQ-5 sales-quality eval pack (#8622)", () => {
  test("fixture pack covers every dimension with a pass and a fail case", () => {
    for (const dimension of SALES_QUALITY_DIMENSIONS) {
      const cases = fixtures.cases.filter(
        (candidate) => candidate.dimension === dimension,
      );
      expect(cases.some((candidate) => candidate.expect === "pass")).toBe(true);
      expect(cases.some((candidate) => candidate.expect === "fail")).toBe(true);
      expect(fixtures.rubrics[dimension].length).toBeGreaterThan(0);
    }
  });

  test("fixture word cap matches the persona-contract cap", () => {
    expect(fixtures.voiceWordCap).toBe(SARAH_VOICE_WORD_CAP);
  });

  for (const testCase of fixtures.cases) {
    test(`${testCase.dimension}: ${testCase.id} -> ${testCase.expect}`, () => {
      const verdict = evaluateSalesQualityDimension(
        testCase.dimension,
        testCase.transcript,
      );
      expect(verdict.dimension).toBe(testCase.dimension);
      expect(verdict.ok).toBe(testCase.expect === "pass");
      if (testCase.expect === "fail") {
        expect(verdict.violations.length).toBeGreaterThan(0);
      } else {
        expect(verdict.violations).toEqual([]);
      }
    });
  }

  test("full-transcript evaluation returns one verdict per dimension", () => {
    const clean = fixtures.cases.find(
      (candidate) => candidate.id === "pain_hunting_two_concrete_openers",
    );
    expect(clean).toBeDefined();
    const verdicts = evaluateSalesQualityTranscript(clean!.transcript);
    expect(verdicts.map((verdict) => verdict.dimension)).toEqual([
      ...SALES_QUALITY_DIMENSIONS,
    ]);
  });

  test("persona contract in agent/instructions.md carries every enforced line", () => {
    for (const line of SALES_QUALITY_INSTRUCTION_LINES) {
      expect(instructions).toContain(line);
    }
  });

  test("word counter treats punctuation-heavy speech sanely", () => {
    expect(wordCount("Hi — I'm Sarah, an AI.")).toBe(5);
    expect(wordCount("")).toBe(0);
  });
});
