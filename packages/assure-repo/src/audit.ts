import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";

import {
  classifyTestSource,
  summarizeCandidates,
  type FalseGreenCandidate,
  type FalseGreenSummary,
} from "./false-green.ts";
import { compareStrings } from "./schema.ts";
import { trackedFiles } from "./workspace.ts";

/**
 * AR-2 (issue #9058): scan every tracked test source into a deterministic
 * candidate report of false-green leads. Candidates are heuristic leads, never
 * findings — a finding requires a demonstrated reproduction (surviving
 * mutation). The report is committed and guarded like the inventory.
 */

export const FALSE_GREEN_REPORT_PATH = "docs/assure-repo/false-green-candidates.v1.json" as const;

export type FalseGreenReport = {
  readonly schemaVersion: "1";
  readonly repository: "OpenAgentsInc/openagents";
  readonly note: string;
  readonly sourceDigest: string;
  readonly summary: FalseGreenSummary;
  readonly candidates: ReadonlyArray<FalseGreenCandidate>;
};

const TEST_FILE = /\.(test|spec)\.(ts|tsx|mts|cts)$/;

/** Tracked test files worth scanning (excludes conformance fixtures and generated trees). */
export const scannableTestFiles = (tracked: ReadonlyArray<string>): ReadonlyArray<string> =>
  tracked
    .filter((path) => TEST_FILE.test(path))
    .filter((path) => !path.includes("/conformance/"))
    .filter((path) => !path.endsWith(".gen.test.ts"))
    .sort(compareStrings);

export const buildFalseGreenReport = (root: string): FalseGreenReport => {
  const files = scannableTestFiles(trackedFiles(root));
  const candidates: FalseGreenCandidate[] = [];
  for (const file of files) {
    let source: string;
    try {
      source = readFileSync(join(root, file), "utf8");
    } catch {
      continue;
    }
    candidates.push(...classifyTestSource(file, source));
  }
  const sorted = [...candidates].sort((a, b) =>
    compareStrings(
      `${a.file}:${String(a.line).padStart(6, "0")}:${a.mode}`,
      `${b.file}:${String(b.line).padStart(6, "0")}:${b.mode}`,
    ),
  );
  const summary = summarizeCandidates(sorted, files.length);
  const digestInput = sorted
    .map((candidate) => `${candidate.file}:${candidate.line}:${candidate.mode}`)
    .join("\n");
  const sourceDigest = `sha256:${createHash("sha256").update(digestInput).digest("hex")}`;
  return {
    schemaVersion: "1",
    repository: "OpenAgentsInc/openagents",
    note: "Heuristic false-green LEADS, not findings. A finding requires a demonstrated reproduction (surviving mutation via mutation-runner). Do not treat a candidate as a confirmed false green. Coverage-theater leads may include tests that delegate their assertion to a custom helper the classifier does not recognise; verify before acting.",
    sourceDigest,
    summary,
    candidates: sorted,
  };
};

export const serializeFalseGreenReport = (report: FalseGreenReport): string =>
  `${JSON.stringify(report, null, 2)}\n`;
