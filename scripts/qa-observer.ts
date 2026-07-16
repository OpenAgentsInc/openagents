#!/usr/bin/env node
// QA-2 (#8907): Observer execution loop — runs the registry checks against
// live production, records a dated JSON results artifact, and prints a
// bounded human summary.
//
// Usage (from the repo root):
//   node --import tsx scripts/qa-observer.ts             # run due checks
//   node --import tsx scripts/qa-observer.ts --all       # force every check
//   node --import tsx scripts/qa-observer.ts --file-issues  # actually file
//                                                           # GitHub issues
//   node --import tsx scripts/qa-observer.ts --base-url https://... --out-dir docs/qa/observer/results
//
// Honest states: pass | drift | unrunnable(reason). A probe whose
// precondition is missing (e.g. admin bearer env absent) is `unrunnable`; a
// probe that ran but whose surface violated the expectation (including HTTP
// non-2xx / network failure reaching the surface) is `drift`. Nothing
// silently passes.
//
// GitHub integration: on sustained drift (>= 2 consecutive runs, judged from
// prior artifacts in the results directory) the executor EMITS the exact
// `gh issue create` / `gh issue comment` command it would run, and executes
// it only behind the explicit `--file-issues` flag — a scheduled run cannot
// spam issues by default.
//
// Exit code: 1 when any check with severityOnDrift high|critical drifted in
// this run, else 0.
//
// Secrets: bearer tokens are read from the environment and sent as headers
// only. They are never printed, never written to artifacts.

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import {
  OBSERVER_CHECK_REGISTRY,
  decodeObserverRegistry,
  parseCadenceMs,
  type ExpectationRule,
  type ObserverCheck,
} from "./qa-observer-registry.js";

export const QA_OBSERVER_RESULTS_SCHEMA = "openagents.qa_observer_results.v1";
export const QA_OBSERVER_DEFAULT_BASE_URL = "https://openagents.com";
export const QA_OBSERVER_ISSUE_REPO = "OpenAgentsInc/openagents";
export const QA_OBSERVER_ISSUE_LABEL = "qa-observer";
export const QA_OBSERVER_SUSTAINED_DRIFT_RUNS = 2;
export const QA_OBSERVER_EVIDENCE_LIMIT = 600;
const HTTP_TIMEOUT_MS = 20_000;

export type ObserverStatus = "pass" | "drift" | "unrunnable";

export type ObserverCheckResult = Readonly<{
  id: string;
  surface: string;
  severityOnDrift: ObserverCheck["severityOnDrift"];
  status: ObserverStatus;
  /** Present when status is "drift" (failed rules) or "unrunnable" (cause). */
  reason?: string;
  /** Bounded probe output (never includes credentials). */
  evidence?: string;
  durationMs: number;
  /** Consecutive drift runs INCLUDING this one; 0 unless status is drift. */
  consecutiveDriftRuns: number;
}>;

export type ObserverRunArtifact = Readonly<{
  schemaVersion: typeof QA_OBSERVER_RESULTS_SCHEMA;
  runAt: string;
  baseUrl: string;
  issue: "OpenAgentsInc/openagents#8907";
  results: readonly ObserverCheckResult[];
  summary: Readonly<{
    checksTotal: number;
    pass: number;
    drift: number;
    unrunnable: number;
    highSeverityDrift: number;
    exitCode: 0 | 1;
  }>;
}>;

const getPath = (value: unknown, path: string): unknown =>
  path
    .split(".")
    .reduce<unknown>(
      (current, key) =>
        typeof current === "object" && current !== null
          ? (current as Record<string, unknown>)[key]
          : undefined,
      value,
    );

const timestampToMs = (value: unknown): number | undefined => {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Date.parse(value);
    return Number.isNaN(parsed) ? undefined : parsed;
  }
  return undefined;
};

/** Evaluate one rule against a parsed probe body. Returns undefined on pass. */
export const evaluateRule = (
  rule: ExpectationRule,
  body: unknown,
  nowMs: number,
): string | undefined => {
  const value = getPath(body, rule.path);
  switch (rule.kind) {
    case "number_gt":
      return typeof value === "number" && value > rule.value
        ? undefined
        : `${rule.path}=${JSON.stringify(value)} is not a number > ${rule.value}`;
    case "timestamp_within_ms": {
      const timestampMs = timestampToMs(value);
      if (timestampMs === undefined) {
        return `${rule.path}=${JSON.stringify(value)} is not a parseable timestamp`;
      }
      const ageMs = nowMs - timestampMs;
      return ageMs <= rule.maxAgeMs
        ? undefined
        : `${rule.path} is ${Math.round(ageMs / 1000)}s old (max ${Math.round(rule.maxAgeMs / 1000)}s)`;
    }
    case "array_non_empty":
      return Array.isArray(value) && value.length > 0
        ? undefined
        : `${rule.path} is not a non-empty array`;
    case "string_equals":
      return value === rule.value
        ? undefined
        : `${rule.path}=${JSON.stringify(value)} !== ${JSON.stringify(rule.value)}`;
    case "field_type":
      return typeof value === rule.type
        ? undefined
        : `${rule.path}=${JSON.stringify(value)} is not a ${rule.type}`;
    case "every_item_has_keys": {
      if (!Array.isArray(value)) return `${rule.path} is not an array`;
      for (const [index, item] of value.entries()) {
        for (const key of rule.keys) {
          if (typeof item !== "object" || item === null || !(key in item)) {
            return `${rule.path}[${index}] is missing key ${JSON.stringify(key)}`;
          }
        }
      }
      return undefined;
    }
  }
};

/** Evaluate all rules; empty result means pass. */
export const evaluateExpectation = (
  rules: readonly ExpectationRule[],
  body: unknown,
  nowMs: number,
): readonly string[] =>
  rules
    .map((rule) => evaluateRule(rule, body, nowMs))
    .filter((failure): failure is string => failure !== undefined);

export type ProbeOutcome =
  | Readonly<{ outcome: "ran"; body: unknown; evidence: string }>
  | Readonly<{ outcome: "probe_failed"; reason: string; evidence?: string }>
  | Readonly<{ outcome: "unrunnable"; reason: string }>;

export type ProbeContext = Readonly<{
  baseUrl: string;
  env: Readonly<Record<string, string | undefined>>;
  fetchImpl: typeof fetch;
  execImpl: (
    command: readonly string[],
  ) => Readonly<{ status: number | null; stdout: string; stderr: string }>;
}>;

const boundEvidence = (raw: string): string =>
  raw.length > QA_OBSERVER_EVIDENCE_LIMIT ? `${raw.slice(0, QA_OBSERVER_EVIDENCE_LIMIT)}…` : raw;

export const runProbe = async (check: ObserverCheck, ctx: ProbeContext): Promise<ProbeOutcome> => {
  const probe = check.probe;
  if (probe.kind === "http") {
    const headers: Record<string, string> = { accept: "application/json" };
    if (probe.bearerEnv !== undefined) {
      const token = ctx.env[probe.bearerEnv];
      if (token === undefined || token.trim() === "") {
        return {
          outcome: "unrunnable",
          reason: `requires bearer env ${probe.bearerEnv} which is not present in the environment`,
        };
      }
      headers.authorization = `Bearer ${token.trim()}`;
    }
    try {
      const response = await ctx.fetchImpl(new URL(probe.path, ctx.baseUrl), {
        headers,
        method: probe.method,
        signal: AbortSignal.timeout(HTTP_TIMEOUT_MS),
      });
      const text = await response.text();
      if (!response.ok) {
        return {
          evidence: boundEvidence(text),
          outcome: "probe_failed",
          reason: `HTTP ${response.status} from ${probe.path}`,
        };
      }
      try {
        return { body: JSON.parse(text) as unknown, evidence: boundEvidence(text), outcome: "ran" };
      } catch {
        return {
          evidence: boundEvidence(text),
          outcome: "probe_failed",
          reason: `non-JSON response from ${probe.path}`,
        };
      }
    } catch (error) {
      return {
        outcome: "probe_failed",
        reason: `fetch failed for ${probe.path}: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
  }
  try {
    const { status, stdout, stderr } = ctx.execImpl(probe.command);
    if (status !== 0) {
      return {
        evidence: boundEvidence(stderr === "" ? stdout : stderr),
        outcome: "probe_failed",
        reason: `command exited ${status ?? "null"}`,
      };
    }
    try {
      return {
        body: JSON.parse(stdout) as unknown,
        evidence: boundEvidence(stdout),
        outcome: "ran",
      };
    } catch {
      // A clean exit with non-JSON stdout still counts as a ran probe; rules
      // then evaluate against undefined paths and report drift honestly.
      return { body: undefined, evidence: boundEvidence(stdout), outcome: "ran" };
    }
  } catch (error) {
    return {
      outcome: "unrunnable",
      reason: `command could not be spawned: ${error instanceof Error ? error.message : String(error)}`,
    };
  }
};

/** A prior run's per-check statuses, newest first. */
export type PriorRunStatuses = readonly Readonly<Record<string, ObserverStatus>>[];

/**
 * Consecutive drift runs including the current one. A non-drift current
 * status is always 0. Prior runs where the check is missing or non-drift
 * break the streak.
 */
export const computeConsecutiveDriftRuns = (
  currentStatus: ObserverStatus,
  checkId: string,
  priorRunsNewestFirst: PriorRunStatuses,
): number => {
  if (currentStatus !== "drift") return 0;
  let streak = 1;
  for (const run of priorRunsNewestFirst) {
    if (run[checkId] === "drift") streak += 1;
    else break;
  }
  return streak;
};

const shellQuote = (part: string): string =>
  /^[A-Za-z0-9._/:=@#-]+$/.test(part) ? part : `'${part.replaceAll("'", String.raw`'\''`)}'`;

export const renderCommand = (command: readonly string[]): string =>
  command.map(shellQuote).join(" ");

export const buildDriftIssueTitle = (checkId: string): string => `QA Observer drift: ${checkId}`;

export const buildDriftIssueBody = (result: ObserverCheckResult, runAt: string): string =>
  [
    `The QA Observer (QA-2 #8907) found sustained drift on \`${result.id}\`.`,
    "",
    `- Surface: ${result.surface}`,
    `- Severity on drift: ${result.severityOnDrift}`,
    `- Consecutive drifting runs: ${result.consecutiveDriftRuns}`,
    `- Run at: ${runAt}`,
    `- Reason: ${result.reason ?? "(none recorded)"}`,
    "",
    "Bounded probe evidence:",
    "```",
    result.evidence ?? "(no evidence captured)",
    "```",
    "",
    "Filed by scripts/qa-observer.ts. See docs/qa/observer/README.md.",
  ].join("\n");

/** The `gh issue create` argv the executor would run for a sustained drift. */
export const buildDriftIssueCreateCommand = (
  result: ObserverCheckResult,
  runAt: string,
): readonly string[] => [
  "gh",
  "issue",
  "create",
  "--repo",
  QA_OBSERVER_ISSUE_REPO,
  "--title",
  buildDriftIssueTitle(result.id),
  "--label",
  QA_OBSERVER_ISSUE_LABEL,
  "--body",
  buildDriftIssueBody(result, runAt),
];

export const buildDriftIssueCommentCommand = (
  issueNumber: number,
  result: ObserverCheckResult,
  runAt: string,
): readonly string[] => [
  "gh",
  "issue",
  "comment",
  String(issueNumber),
  "--repo",
  QA_OBSERVER_ISSUE_REPO,
  "--body",
  buildDriftIssueBody(result, runAt),
];

export const buildArtifact = (
  results: readonly ObserverCheckResult[],
  runAt: string,
  baseUrl: string,
): ObserverRunArtifact => {
  const drifted = results.filter((result) => result.status === "drift");
  const highSeverityDrift = drifted.filter(
    (result) => result.severityOnDrift === "high" || result.severityOnDrift === "critical",
  ).length;
  return {
    baseUrl,
    issue: "OpenAgentsInc/openagents#8907",
    results,
    runAt,
    schemaVersion: QA_OBSERVER_RESULTS_SCHEMA,
    summary: {
      checksTotal: results.length,
      drift: drifted.length,
      exitCode: highSeverityDrift > 0 ? 1 : 0,
      highSeverityDrift,
      pass: results.filter((result) => result.status === "pass").length,
      unrunnable: results.filter((result) => result.status === "unrunnable").length,
    },
  };
};

export type PriorRun = Readonly<{
  runAt: string;
  statuses: Readonly<Record<string, ObserverStatus>>;
}>;

/** Read prior artifacts from the results dir, newest first. Tolerates junk. */
export const readPriorRuns = (resultsDir: string): readonly PriorRun[] => {
  let files: string[];
  try {
    files = readdirSync(resultsDir).filter((name) => name.endsWith(".json"));
  } catch {
    return [];
  }
  const runs: PriorRun[] = [];
  for (const name of files) {
    try {
      const parsed = JSON.parse(readFileSync(join(resultsDir, name), "utf8")) as {
        schemaVersion?: string;
        runAt?: string;
        results?: readonly { id?: string; status?: string }[];
      };
      if (parsed.schemaVersion !== QA_OBSERVER_RESULTS_SCHEMA) continue;
      if (typeof parsed.runAt !== "string" || !Array.isArray(parsed.results)) continue;
      const statuses: Record<string, ObserverStatus> = {};
      for (const result of parsed.results) {
        if (
          typeof result.id === "string" &&
          (result.status === "pass" || result.status === "drift" || result.status === "unrunnable")
        ) {
          statuses[result.id] = result.status;
        }
      }
      runs.push({ runAt: parsed.runAt, statuses });
    } catch {
      // ignore unreadable files — never let a corrupt artifact kill the loop
    }
  }
  return runs.sort((a, b) => (a.runAt < b.runAt ? 1 : -1));
};

/** Which checks are due, given the newest prior run per check. */
export const selectDueChecks = (
  checks: readonly ObserverCheck[],
  priorRunsNewestFirst: readonly PriorRun[],
  nowMs: number,
  forceAll: boolean,
): readonly ObserverCheck[] => {
  if (forceAll) return checks;
  return checks.filter((check) => {
    const cadenceMs = parseCadenceMs(check.cadence) ?? 0;
    const lastRun = priorRunsNewestFirst.find((run) => run.statuses[check.id] !== undefined);
    if (lastRun === undefined) return true;
    const lastRunMs = Date.parse(lastRun.runAt);
    return Number.isNaN(lastRunMs) || nowMs - lastRunMs >= cadenceMs;
  });
};

export const artifactFileName = (runAt: string): string =>
  `qa-observer-run-${runAt.replaceAll(":", "-").replace(/\.\d+Z$/, "Z")}.json`;

const summaryLine = (result: ObserverCheckResult): string => {
  const head = `[${result.status}] ${result.id} (${result.severityOnDrift}, ${result.durationMs}ms)`;
  if (result.status === "pass") return head;
  const tail = result.reason ?? "";
  return `${head} — ${tail.length > 200 ? `${tail.slice(0, 200)}…` : tail}`;
};

const defaultExec = (command: readonly string[]) => {
  const [file, ...args] = command;
  try {
    const stdout = execFileSync(file as string, args, { encoding: "utf8", timeout: 120_000 });
    return { status: 0, stderr: "", stdout };
  } catch (error) {
    const failure = error as {
      status?: number | null;
      stdout?: string;
      stderr?: string;
      code?: string;
    };
    if (failure.code === "ENOENT") throw error;
    return {
      status: failure.status ?? null,
      stderr: failure.stderr ?? "",
      stdout: failure.stdout ?? "",
    };
  }
};

const findOpenDriftIssue = (checkId: string): number | undefined => {
  const output = execFileSync(
    "gh",
    [
      "issue",
      "list",
      "--repo",
      QA_OBSERVER_ISSUE_REPO,
      "--label",
      QA_OBSERVER_ISSUE_LABEL,
      "--state",
      "open",
      "--search",
      `in:title ${buildDriftIssueTitle(checkId)}`,
      "--json",
      "number,title",
    ],
    { encoding: "utf8" },
  );
  const issues = JSON.parse(output) as readonly { number: number; title: string }[];
  return issues.find((issue) => issue.title === buildDriftIssueTitle(checkId))?.number;
};

export const runObserver = async (argv: readonly string[]): Promise<number> => {
  const forceAll = argv.includes("--all");
  const fileIssues = argv.includes("--file-issues");
  const baseUrlIndex = argv.indexOf("--base-url");
  const baseUrl =
    baseUrlIndex >= 0
      ? (argv[baseUrlIndex + 1] ?? QA_OBSERVER_DEFAULT_BASE_URL)
      : QA_OBSERVER_DEFAULT_BASE_URL;
  const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const outDirIndex = argv.indexOf("--out-dir");
  const resultsDir =
    outDirIndex >= 0
      ? resolve(argv[outDirIndex + 1] ?? "")
      : join(repoRoot, "docs/qa/observer/results");

  const decoded = decodeObserverRegistry(OBSERVER_CHECK_REGISTRY);
  if ("problems" in decoded) {
    console.error("[qa-observer] FATAL: seed registry is invalid:");
    for (const problem of decoded.problems) console.error(`  - ${problem}`);
    return 2;
  }

  const priorRuns = readPriorRuns(resultsDir);
  const nowMs = Date.now();
  const runAt = new Date(nowMs).toISOString();
  const dueChecks = selectDueChecks(decoded.checks, priorRuns, nowMs, forceAll);
  const priorStatuses: PriorRunStatuses = priorRuns.map((run) => run.statuses);

  const ctx: ProbeContext = { baseUrl, env: process.env, execImpl: defaultExec, fetchImpl: fetch };
  const results: ObserverCheckResult[] = [];
  for (const check of dueChecks) {
    const startedAt = Date.now();
    const outcome = await runProbe(check, ctx);
    const durationMs = Date.now() - startedAt;
    const base = {
      durationMs,
      id: check.id,
      severityOnDrift: check.severityOnDrift,
      surface: check.surface,
    };
    if (outcome.outcome === "unrunnable") {
      results.push({
        ...base,
        consecutiveDriftRuns: 0,
        reason: outcome.reason,
        status: "unrunnable",
      });
      continue;
    }
    if (outcome.outcome === "probe_failed") {
      const consecutiveDriftRuns = computeConsecutiveDriftRuns("drift", check.id, priorStatuses);
      results.push({
        ...base,
        consecutiveDriftRuns,
        ...(outcome.evidence === undefined ? {} : { evidence: outcome.evidence }),
        reason: outcome.reason,
        status: "drift",
      });
      continue;
    }
    const failures = evaluateExpectation(check.expectation.rules, outcome.body, nowMs);
    if (failures.length === 0) {
      results.push({ ...base, consecutiveDriftRuns: 0, status: "pass" });
    } else {
      const consecutiveDriftRuns = computeConsecutiveDriftRuns("drift", check.id, priorStatuses);
      results.push({
        ...base,
        consecutiveDriftRuns,
        evidence: outcome.evidence,
        reason: failures.join("; "),
        status: "drift",
      });
    }
  }

  const artifact = buildArtifact(results, runAt, baseUrl);
  mkdirSync(resultsDir, { recursive: true });
  const artifactPath = join(resultsDir, artifactFileName(runAt));
  writeFileSync(artifactPath, `${JSON.stringify(artifact, null, 2)}\n`);

  console.log(`qa-observer run ${runAt} against ${baseUrl}`);
  console.log(
    `checks due: ${dueChecks.length}/${decoded.checks.length}${forceAll ? " (--all)" : ""}`,
  );
  for (const result of results) console.log(`  ${summaryLine(result)}`);
  console.log(
    `summary: ${artifact.summary.pass} pass, ${artifact.summary.drift} drift ` +
      `(${artifact.summary.highSeverityDrift} high-severity), ${artifact.summary.unrunnable} unrunnable`,
  );
  console.log(`artifact: ${artifactPath}`);

  const sustained = results.filter(
    (result) =>
      result.status === "drift" && result.consecutiveDriftRuns >= QA_OBSERVER_SUSTAINED_DRIFT_RUNS,
  );
  for (const result of sustained) {
    const createCommand = buildDriftIssueCreateCommand(result, runAt);
    if (!fileIssues) {
      console.log(
        `sustained drift on ${result.id} (${result.consecutiveDriftRuns} runs) — would run:`,
      );
      console.log(`  ${renderCommand(createCommand)}`);
      console.log("  (pass --file-issues to actually file)");
      continue;
    }
    try {
      const existing = findOpenDriftIssue(result.id);
      const command =
        existing === undefined
          ? createCommand
          : buildDriftIssueCommentCommand(existing, result, runAt);
      console.log(
        `sustained drift on ${result.id} — filing via: ${renderCommand(command.slice(0, 6))} …`,
      );
      execFileSync(command[0] as string, command.slice(1), { encoding: "utf8" });
    } catch (error) {
      console.error(
        `[qa-observer] WARNING: failed to file issue for ${result.id}: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }

  return artifact.summary.exitCode;
};

if (import.meta.main) {
  process.exit(await runObserver(process.argv.slice(2)));
}
