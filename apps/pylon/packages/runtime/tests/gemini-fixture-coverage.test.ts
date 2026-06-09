import { describe, expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { join } from "node:path";

const runtimeRoot = join(import.meta.dir, "..");

const requiredFixtureCoverage = [
  {
    label: "auth precedence",
    file: "tests/gemini-backend-profile.test.ts",
  },
  {
    label: "missing API key failure redaction",
    file: "tests/gemini-receipts-capability.test.ts",
  },
  {
    label: "body prep",
    file: "tests/gemini-protocol.test.ts",
  },
  {
    label: "tool declarations",
    file: "tests/gemini-protocol.test.ts",
  },
  {
    label: "tool history",
    file: "tests/gemini-protocol.test.ts",
  },
  {
    label: "schema sanitation",
    file: "tests/gemini-protocol.test.ts",
  },
  {
    label: "SSE parsing",
    file: "tests/gemini-stream-parser.test.ts",
  },
  {
    label: "fake E2E tool round trip",
    file: "tests/gemini-tool-loop.test.ts",
  },
  {
    label: "malformed provider output",
    file: "tests/gemini-stream-parser.test.ts",
  },
] as const;

describe("Gemini fixture coverage", () => {
  test("keeps required Gemini fixture coverage present", () => {
    expect(requiredFixtureCoverage.map((entry) => entry.label)).toEqual([
      "auth precedence",
      "missing API key failure redaction",
      "body prep",
      "tool declarations",
      "tool history",
      "schema sanitation",
      "SSE parsing",
      "fake E2E tool round trip",
      "malformed provider output",
    ]);

    for (const entry of requiredFixtureCoverage) {
      expect(existsSync(join(runtimeRoot, entry.file)), entry.label).toBe(true);
    }
  });
});
