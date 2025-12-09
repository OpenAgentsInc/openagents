/**
 * Guardrail Test: Detect Task-Specific Hardcoding
 *
 * This test ensures that HillClimber files do NOT contain task-specific
 * hardcoding that would undermine the thesis: "Architecture beats model size."
 *
 * If we hardcode task-specific knowledge, we're NOT proving architecture works
 * — we're proving we can game benchmarks. This is the OPPOSITE of what we're
 * trying to demonstrate.
 */

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, test, expect } from "bun:test";

const HILLCLIMBER_DIR = join(import.meta.dir, ".");
const SKILLS_DIR = join(import.meta.dir, "..", "skills", "library");

// Files that must NOT contain task-specific hardcoding
const FILES_TO_CHECK = [
  "decomposer.ts",
  "test-generator-iterative.ts",
  "meta-reasoner.ts",
  "monitor.ts",
  "map-orchestrator.ts",
  "sampling-orchestrator.ts",
];

// Task-specific terms that must NOT appear in code
const FORBIDDEN_TERMS = [
  "regex-log",
  "path-tracing",
  "model-extraction",
  "video-processing",
  "dna-assembly",
  "IPv4",
  "YYYY-MM-DD",
  "/app/regex.txt",
  "regex.txt",
  "image.c",
  "steal.py",
  "jump_analyzer.py",
  "primers.fasta",
];

// Skills file to check
const SKILLS_FILE = "tb2-skills.ts";

describe("No Task-Specific Hardcoding", () => {
  for (const file of FILES_TO_CHECK) {
    test(`${file} should not contain task-specific hardcoding`, () => {
      const filePath = join(HILLCLIMBER_DIR, file);
      const content = readFileSync(filePath, "utf-8");

      for (const term of FORBIDDEN_TERMS) {
        // Check for exact matches (case-insensitive)
        const regex = new RegExp(`\\b${term.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\b`, "i");
        if (regex.test(content)) {
          // Allow if it's in a comment explaining why it's forbidden
          const lines = content.split("\n");
          const lineNumbers: number[] = [];
          lines.forEach((line, idx) => {
            if (regex.test(line) && !line.includes("GUARDRAIL") && !line.includes("forbidden")) {
              lineNumbers.push(idx + 1);
            }
          });

          if (lineNumbers.length > 0) {
            throw new Error(
              `Found forbidden term "${term}" in ${file} at lines: ${lineNumbers.join(", ")}\n` +
                `This violates the "no task-specific hardcoding" rule.`
            );
          }
        }
      }
    });
  }

  test(`${SKILLS_FILE} should not contain task-specific skills`, () => {
    const filePath = join(SKILLS_DIR, SKILLS_FILE);
    const content = readFileSync(filePath, "utf-8");

    // Check that TB2_SKILLS array is empty or contains only general skills
    if (content.includes("REGEX_BOUNDARY_SKILL") ||
        content.includes("PPM_FORMAT_SKILL") ||
        content.includes("NUMPY_ARRAY_MANIPULATION_SKILL") ||
        content.includes("VIDEO_FRAME_ANALYSIS_SKILL") ||
        content.includes("BIOPYTHON_PATTERNS_SKILL")) {
      throw new Error(
        `Found task-specific skills in ${SKILLS_FILE}. ` +
        `All task-specific skills must be removed.`
      );
    }

    // Check that getSkillsForTask returns empty array
    if (!content.includes("return []") && !content.includes("return []")) {
      throw new Error(
        `${SKILLS_FILE} should return empty array from getSkillsForTask().`
      );
    }
  });
});
