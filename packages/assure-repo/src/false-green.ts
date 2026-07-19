import { compareStrings } from "./schema.ts";

/**
 * AR-2 (issue #9058): heuristic false-green classifier over test sources.
 *
 * These are the named false-green modes from the assurance-spec taxonomy. The
 * classifier emits CANDIDATES (leads), never findings. A candidate becomes a
 * confirmed finding only when demonstrated by reproduction (a surviving
 * mutation via the mutation runner). The classifier never asserts a false
 * green from inspection alone.
 */

export const FALSE_GREEN_MODES = [
  "false_green_coverage_theater",
  "false_green_mocked_seam",
  "false_green_round_up",
] as const;
export type FalseGreenMode = (typeof FALSE_GREEN_MODES)[number];

export type FalseGreenCandidate = {
  readonly file: string;
  readonly line: number;
  readonly mode: FalseGreenMode;
  readonly evidence: string;
};

// Matches direct assertions, assertion-helper calls (expectFoo/assertBar), and
// matcher-method chains, so a test that delegates its assertion to a helper is
// not mistaken for coverage theater.
const ASSERTION =
  /\b(expect|assert)\w*\s*\(|\bassert\s*\.\s*\w+\s*\(|\.(toBe|toEqual|toStrictEqual|toMatch|toMatchObject|toMatchInlineSnapshot|toMatchSnapshot|toThrow|toContain|toHaveBeen|toHaveLength|toHaveProperty|toBeUndefined|toBeNull|toBeTruthy|toBeFalsy|toBeGreaterThan|toBeLessThan|toBeCloseTo|toBeDefined|toBeInstanceOf|resolves|rejects)\b/;
const TEST_BLOCK = /\b(test|it)\s*\(\s*[`'"]/g;
const SKIP = /\b(test|it|describe)\.(skip|todo|only)\s*\(/g;
const MOCK = /\b(vi|vitest)\.mock\s*\(|\bmock\s*\(\s*[`'"]/g;

const lineOf = (text: string, index: number): number => text.slice(0, index).split("\n").length;

const ISSUE_REF = /#\d{3,6}\b/;

/**
 * Text around a skip: from ~3 lines before to the end of the skip's own line.
 * A tracking ref in that window (a comment or the test title) marks the skip
 * as an intentional, tracked deferral rather than a silent one.
 */
const skipContext = (source: string, index: number): string => {
  const lineStart = source.lastIndexOf("\n", index) + 1;
  let start = lineStart;
  for (let i = 0; i < 3 && start > 0; i += 1) start = source.lastIndexOf("\n", start - 2) + 1;
  const lineEnd = source.indexOf("\n", index);
  return source.slice(start, lineEnd < 0 ? source.length : lineEnd);
};

/**
 * Find the index of the callback body's opening `{` for a test block, skipping
 * string literals and comments. Prefers an arrow-function opener (`=> {`) so a
 * leading options-object argument (e.g. `test(name, { timeout }, () => {…})`)
 * or a brace inside the test-name string is not mistaken for the body. Falls
 * back to the first real `{`.
 */
const callbackBraceStart = (text: string, startIndex: number, end: number): number => {
  let sawArrow = false;
  let firstBrace = -1;
  let str: string | null = null;
  let comment: "line" | "block" | null = null;
  for (let i = startIndex; i < end; i += 1) {
    const ch = text[i];
    const next = text[i + 1];
    if (comment === "line") {
      if (ch === "\n") comment = null;
      continue;
    }
    if (comment === "block") {
      if (ch === "*" && next === "/") {
        comment = null;
        i += 1;
      }
      continue;
    }
    if (str) {
      if (ch === "\\") {
        i += 1;
        continue;
      }
      if (ch === str) str = null;
      continue;
    }
    if (ch === "/" && next === "/") {
      comment = "line";
      i += 1;
      continue;
    }
    if (ch === "/" && next === "*") {
      comment = "block";
      i += 1;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === "`") {
      str = ch;
      continue;
    }
    if (ch === "=" && next === ">") {
      sawArrow = true;
      i += 1;
      continue;
    }
    if (ch === "{") {
      if (firstBrace < 0) firstBrace = i;
      if (sawArrow) return i;
    }
  }
  return firstBrace;
};

/**
 * Extract the body of a test block starting at `startIndex` by string-aware
 * brace matching from the callback opener. Bounded and lenient.
 */
const blockBody = (text: string, startIndex: number): string => {
  const braceStart = callbackBraceStart(text, startIndex, text.length);
  if (braceStart < 0) return "";
  let depth = 0;
  let str: string | null = null;
  let comment: "line" | "block" | null = null;
  for (let i = braceStart; i < text.length; i += 1) {
    const ch = text[i];
    const next = text[i + 1];
    if (comment === "line") {
      if (ch === "\n") comment = null;
      continue;
    }
    if (comment === "block") {
      if (ch === "*" && next === "/") {
        comment = null;
        i += 1;
      }
      continue;
    }
    if (str) {
      if (ch === "\\") {
        i += 1;
        continue;
      }
      if (ch === str) str = null;
      continue;
    }
    if (ch === "/" && next === "/") {
      comment = "line";
      i += 1;
      continue;
    }
    if (ch === "/" && next === "*") {
      comment = "block";
      i += 1;
      continue;
    }
    if (ch === '"' || ch === "'" || ch === "`") {
      str = ch;
      continue;
    }
    if (ch === "{") depth += 1;
    else if (ch === "}") {
      depth -= 1;
      if (depth === 0) return text.slice(braceStart, i + 1);
    }
  }
  return text.slice(braceStart);
};

/** Classify one test file's source into candidate false-green leads. */
export const classifyTestSource = (
  file: string,
  source: string,
): ReadonlyArray<FalseGreenCandidate> => {
  const candidates: FalseGreenCandidate[] = [];

  // Coverage theater: a test block that never asserts.
  for (const match of source.matchAll(TEST_BLOCK)) {
    const body = blockBody(source, match.index);
    // Skip empty arrow-only or trivially short bodies handled by the assertion check.
    if (body.length > 0 && !ASSERTION.test(body)) {
      candidates.push({
        file,
        line: lineOf(source, match.index),
        mode: "false_green_coverage_theater",
        evidence: "test block contains no expect/assert call",
      });
    }
  }

  // Round-up: skipped / todo / only hides results from the summary. A `.only`
  // is always flagged (it silently disables every other test in the file). A
  // `.skip`/`.todo` is flagged only when it is SILENT — an intentional deferral
  // that carries a tracking ref (a `#NNNN` issue) within 3 lines is tracked,
  // not a false green, and is not reported.
  for (const match of source.matchAll(SKIP)) {
    const modifier = match[2];
    const line = lineOf(source, match.index);
    if (modifier === "only") {
      candidates.push({
        file,
        line,
        mode: "false_green_round_up",
        evidence: `uses ${match[0].trim()} which silently disables every other test in the file`,
      });
      continue;
    }
    if (ISSUE_REF.test(skipContext(source, match.index))) continue;
    candidates.push({
      file,
      line,
      mode: "false_green_round_up",
      evidence: `uses ${match[0].trim()} with no tracking ref; silently excluded from a green summary`,
    });
  }

  // Mocked seam: the test mocks a module (may be mocking the seam under test).
  for (const match of source.matchAll(MOCK)) {
    candidates.push({
      file,
      line: lineOf(source, match.index),
      mode: "false_green_mocked_seam",
      evidence: `mocks a module (${match[0].trim()}); verify the real seam is still exercised`,
    });
  }

  return candidates.sort((a, b) =>
    compareStrings(`${a.file}:${a.line}:${a.mode}`, `${b.file}:${b.line}:${b.mode}`),
  );
};

export type FalseGreenSummary = {
  readonly filesScanned: number;
  readonly candidateCount: number;
  readonly byMode: Record<string, number>;
};

export const summarizeCandidates = (
  candidates: ReadonlyArray<FalseGreenCandidate>,
  filesScanned: number,
): FalseGreenSummary => {
  const byMode: Record<string, number> = {};
  for (const candidate of candidates) byMode[candidate.mode] = (byMode[candidate.mode] ?? 0) + 1;
  const sorted: Record<string, number> = {};
  for (const key of Object.keys(byMode).sort(compareStrings)) sorted[key] = byMode[key]!;
  return { filesScanned, candidateCount: candidates.length, byMode: sorted };
};
