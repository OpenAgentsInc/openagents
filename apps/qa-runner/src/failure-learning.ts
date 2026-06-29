// QA failure learning (#6195): a failed/REFUTED run or a low-scoring eval is
// SIGNAL, not just a red. This module captures a public-safe FAILURE PATTERN
// from a post-run `result.json` (status + the #6192 `verify` verdict) and
// produces a fix / scenario-update SUGGESTION — config-selectable per project.
//
// Adopts droid-control / Factory Automated-QA failure-learning, but stays HONEST
// and EVIDENCE-ONLY:
//   - A failure is recorded AS a failure. Nothing is hidden, suppressed, or
//     faked green. The capture exists BECAUSE the run did not pass.
//   - DETECT-BY-READING-RESULT: we inspect the result the runner already wrote
//     (status === "fail", or verify.verdict === "REFUTED", or a low eval score).
//     We do NOT edit `runner.ts` (a peer lane owns it) — this is a post-run
//     reducer over the dereferenceable receipt.
//   - PUBLIC-SAFE: the captured pattern carries labels / verdicts / evidence
//     summaries only (no prompts, tokens, secrets). The same forbidden-field
//     tripwire the result uses (`assertPublicSafeResult`) re-checks every capture.
//
// STRATEGIES (config-selectable, default suggest-only):
//   - suggest_in_report (DEFAULT) — render a copy-paste fix/scenario snippet for
//     the PR comment; a human reviews and applies it.
//   - auto_commit  — (flag-gated, default OFF) commit the scenario/sub-skill
//     update after a run. INERT here: this module produces the PLAN only; it
//     never shells out / writes git. Arming the executor is a separate, gated
//     follow-up so the default path can never silently mutate a repo.
//   - open_pr      — (flag-gated, default OFF) open a draft PR with the
//     failure-catalog / scenario update. Same INERT posture: plan only.
//
// The Blueprint/GEPA half (the claim-level candidate-feedback signal) lives in
// `failure-learning-gepa.ts`; this module owns the report-side strategies and the
// shared FailurePattern capture both halves consume.

import { assertPublicSafeResult, type QaRunResult } from "./result";

export const FAILURE_PATTERN_SCHEMA_VERSION =
  "openagents.qa_runner.failure_pattern.v1" as const;

// ---------------------------------------------------------------------------
// Config: the report-side strategy (config-selectable per project).
// ---------------------------------------------------------------------------

/** The report-side strategy a project selects for failure learning. */
export type FailureLearningStrategy = "suggest_in_report" | "auto_commit" | "open_pr";

/**
 * Per-project failure-learning configuration. `strategy` defaults to the
 * suggest-only path; the two mutating strategies (`auto_commit`, `open_pr`) are
 * ADDITIONALLY flag-gated by `armMutations` so a stray config value can never
 * arm a write path. With `armMutations` false (the default), `auto_commit` and
 * `open_pr` are downgraded to `suggest_in_report` and the plan stays inert.
 */
export interface FailureLearningConfig {
  /** Selected strategy; defaults to `suggest_in_report`. */
  readonly strategy?: FailureLearningStrategy;
  /**
   * Second gate for the mutating strategies. Default OFF. Even when true, this
   * module only PLANS the mutation (it never executes git / gh) — arming the
   * executor is a separate, owner-gated follow-up.
   */
  readonly armMutations?: boolean;
}

/** The effective, resolved strategy after applying the default + the arm gate. */
export type ResolvedFailureLearningStrategy =
  | { readonly strategy: "suggest_in_report" }
  // A mutating strategy that was REQUESTED but NOT armed -> downgraded, with the
  // reason recorded honestly (so the report says why no mutation happened).
  | {
      readonly strategy: "suggest_in_report";
      readonly downgradedFrom: "auto_commit" | "open_pr";
      readonly downgradeReason: string;
    }
  // A mutating strategy that is armed -> the PLAN is produced (still inert: no
  // git/gh is executed by this module).
  | { readonly strategy: "auto_commit"; readonly planOnly: true }
  | { readonly strategy: "open_pr"; readonly planOnly: true };

/**
 * Resolve the requested strategy against the default + the `armMutations` gate.
 * Default-off: a mutating strategy without `armMutations` is downgraded to
 * suggest-only (never silently mutating), and the downgrade reason is recorded.
 */
export function resolveFailureLearningStrategy(
  config: FailureLearningConfig = {},
): ResolvedFailureLearningStrategy {
  const requested = config.strategy ?? "suggest_in_report";
  if (requested === "suggest_in_report") return { strategy: "suggest_in_report" };
  if (config.armMutations !== true) {
    return {
      strategy: "suggest_in_report",
      downgradedFrom: requested,
      downgradeReason:
        `strategy "${requested}" requires armMutations=true (default OFF); ` +
        "downgraded to suggest_in_report so no repo mutation happens",
    };
  }
  return requested === "auto_commit"
    ? { strategy: "auto_commit", planOnly: true }
    : { strategy: "open_pr", planOnly: true };
}

// ---------------------------------------------------------------------------
// FailurePattern: the public-safe capture shared by both halves.
// ---------------------------------------------------------------------------

/** How a run was detected as a failure (the trigger). */
export type FailureSource =
  | "run_failed" // result.status === "fail"
  | "verify_refuted" // result.verify.verdict === "REFUTED"
  | "low_eval_score"; // an eval variant scored below the floor

/** One refuted/failed claim or step, captured public-safe (no prompts/tokens). */
export interface CapturedFinding {
  /** Stable id of the commitment/step the failure is about. */
  readonly id: string;
  /** The public-safe claim/label that was contradicted. */
  readonly claim: string;
  /** The public-safe one-line account of the contradicting evidence. */
  readonly evidenceSummary: string;
}

/**
 * A captured, public-safe failure pattern. This is the SIGNAL: it names the
 * target, the source/trigger, the contradicted findings, and a stable
 * `patternRef` so a reviewer (and the GEPA half) can dereference it. It carries
 * NO secrets — only labels/verdicts/evidence summaries.
 */
export interface FailurePattern {
  readonly schemaVersion: typeof FAILURE_PATTERN_SCHEMA_VERSION;
  /** Stable, public-safe ref derived from target + a digest of the findings. */
  readonly patternRef: string;
  /** What triggered the capture. */
  readonly source: FailureSource;
  /** The target the run was against (public-safe name + base url). */
  readonly target: { readonly name: string; readonly baseUrl: string };
  /** The honest verify verdict, when the run declared commitments. */
  readonly verdict?: "REFUTED" | "INCONCLUSIVE";
  /** The contradicted findings (refuted commitments and/or failed steps). */
  readonly findings: ReadonlyArray<CapturedFinding>;
  /** A public-safe headline describing the pattern. */
  readonly summary: string;
}

/** A small, deterministic, dependency-free fnv-1a digest (hex) for refs. */
function digestHex(text: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    hash ^= text.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash.toString(16).padStart(8, "0");
}

const slugifyRef = (text: string): string =>
  text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48) || "run";

// ---------------------------------------------------------------------------
// Detection: is this run a failure worth learning from? (read result only)
// ---------------------------------------------------------------------------

/**
 * Decide whether a post-run result is a failure-learning trigger, WITHOUT a
 * local run — purely by reading the result the runner already wrote. Honest:
 *   - verify.verdict === "REFUTED" -> verify_refuted (most specific: a false
 *     claim is a finding with the contradicting evidence inline). A REFUTED run
 *     is also status=fail, so this is checked FIRST to keep the richer source.
 *   - status === "fail"            -> run_failed (failed without commitments)
 *   - else                         -> not a trigger (a clean pass is not signal)
 * An INCONCLUSIVE verify is NOT a learning trigger on its own (uncertainty is
 * never rounded up to a failure); only an observed contradiction is.
 */
export function detectRunFailure(
  result: QaRunResult,
): { readonly isFailure: true; readonly source: FailureSource } | { readonly isFailure: false } {
  if (result.verify?.verdict === "REFUTED") {
    return { isFailure: true, source: "verify_refuted" };
  }
  if (result.status === "fail") return { isFailure: true, source: "run_failed" };
  return { isFailure: false };
}

/**
 * Capture a public-safe `FailurePattern` from a failed/REFUTED run result.
 * Returns `undefined` when the run is NOT a failure (a clean pass is not signal —
 * never fabricate a failure). The capture prefers the verify findings (the
 * contradicting commitment evidence); when a run failed without commitments it
 * falls back to the failed step labels + the honest `failure` summary.
 *
 * Public-safe by construction: the produced pattern is re-checked by
 * `assertPublicSafeResult` before it is returned (fail closed on a leak).
 */
export function captureFailurePattern(result: QaRunResult): FailurePattern | undefined {
  const detection = detectRunFailure(result);
  if (!detection.isFailure) return undefined;

  // Prefer the verify findings (REFUTED commitments carry the contradicting
  // evidence inline); else fall back to the failed steps.
  const refutedFindings: CapturedFinding[] = (result.verify?.findings ?? [])
    .filter((f) => f.verdict === "REFUTED")
    .map((f) => ({ id: f.id, claim: f.claim, evidenceSummary: f.evidenceSummary }));

  const failedStepFindings: CapturedFinding[] = result.steps
    .filter((s) => s.status === "failed")
    .map((s) => ({
      id: `step:${s.index}`,
      claim: s.label,
      evidenceSummary: `observed step "${s.label}" (${s.kind}) = failed`,
    }));

  // Use refuted-commitment findings when present; otherwise the failed steps.
  const findings = refutedFindings.length > 0 ? refutedFindings : failedStepFindings;

  const summary =
    detection.source === "verify_refuted"
      ? `verify verdict REFUTED on ${result.target.name}: ${findings.length} contradicted claim(s)`
      : `run failed on ${result.target.name}${result.failure ? `: ${result.failure}` : ""}`;

  const digest = digestHex(
    JSON.stringify({ target: result.target.name, findings, summary }),
  );

  const pattern: FailurePattern = {
    schemaVersion: FAILURE_PATTERN_SCHEMA_VERSION,
    patternRef: `failure_pattern:qa_runner:${slugifyRef(result.target.name)}:${digest}`,
    source: detection.source,
    target: { name: result.target.name, baseUrl: result.target.baseUrl },
    ...(result.verify?.verdict === "REFUTED" || result.verify?.verdict === "INCONCLUSIVE"
      ? { verdict: result.verify.verdict }
      : {}),
    findings,
    summary,
  };

  // Tripwire: never emit a pattern that leaks a forbidden field.
  assertPublicSafeResult(pattern);
  return pattern;
}

/**
 * Capture a failure pattern from a LOW EVAL SCORE (a variant whose pass-rate is
 * below the floor). A low eval is signal even when no single run threw a
 * contradiction. Honest: the floor is explicit and the captured finding records
 * the observed pass-rate vs the floor. Returns `undefined` when the variant met
 * the floor (never fabricate a failure).
 */
export function captureLowEvalPattern(input: {
  readonly evalId: string;
  readonly variantId: string;
  readonly passRate: number;
  /** The acceptance floor in [0,1]; a variant below it is captured. */
  readonly passRateFloor: number;
  readonly target: { readonly name: string; readonly baseUrl: string };
}): FailurePattern | undefined {
  if (input.passRate >= input.passRateFloor) return undefined;
  const finding: CapturedFinding = {
    id: `eval:${input.evalId}:${input.variantId}`,
    claim: `variant "${input.variantId}" must meet pass-rate floor ${input.passRateFloor}`,
    evidenceSummary: `observed pass-rate ${input.passRate.toFixed(2)} < floor ${input.passRateFloor.toFixed(2)}`,
  };
  const summary = `low eval score on ${input.target.name}: ${input.variantId} below floor`;
  const digest = digestHex(JSON.stringify({ eval: input.evalId, finding, summary }));
  const pattern: FailurePattern = {
    schemaVersion: FAILURE_PATTERN_SCHEMA_VERSION,
    patternRef: `failure_pattern:qa_runner:${slugifyRef(input.evalId)}:${digest}`,
    source: "low_eval_score",
    target: input.target,
    findings: [finding],
    summary,
  };
  assertPublicSafeResult(pattern);
  return pattern;
}

// ---------------------------------------------------------------------------
// Suggestion: a fix / scenario-update from a captured pattern.
// ---------------------------------------------------------------------------

/**
 * A fix / scenario-update suggestion produced from a captured failure pattern.
 * It is a PLAN + a copy-paste snippet; whether it is surfaced as a report line,
 * a commit, or a PR is governed by the resolved strategy. The mutating
 * strategies stay INERT (plan only) in this module.
 */
export interface FailureSuggestion {
  /** The pattern this suggestion is for. */
  readonly patternRef: string;
  /** The resolved strategy (after the default + arm gate). */
  readonly resolved: ResolvedFailureLearningStrategy;
  /** A public-safe, copy-paste markdown snippet for the PR comment. */
  readonly snippet: string;
  /**
   * For an armed mutating strategy: a public-safe PLAN of what WOULD be done.
   * Always present for `auto_commit`/`open_pr` (plan only — never executed here).
   */
  readonly mutationPlan?: {
    readonly kind: "auto_commit" | "open_pr";
    /** A public-safe description of the planned change (no diffs of secrets). */
    readonly description: string;
    /** Honest: this module never executes the plan. */
    readonly executed: false;
  };
}

/** Render the copy-paste fix/scenario-update snippet (public-safe markdown). */
function renderSnippet(pattern: FailurePattern): string {
  const lines: string[] = [];
  lines.push(`**Captured failure pattern** \`${pattern.patternRef}\``);
  lines.push("");
  lines.push(`- source: \`${pattern.source}\``);
  if (pattern.verdict) lines.push(`- verify verdict: \`${pattern.verdict}\``);
  lines.push(`- ${pattern.summary}`);
  if (pattern.findings.length > 0) {
    lines.push("");
    lines.push("Contradicted claim(s):");
    for (const f of pattern.findings) {
      lines.push(`- \`${f.id}\` — ${f.claim}: ${f.evidenceSummary}`);
    }
  }
  lines.push("");
  lines.push("Suggested next step (manual review):");
  lines.push(
    pattern.source === "verify_refuted"
      ? "- Either fix the regression so the claim holds, or correct the scenario commitment if the claim itself was wrong."
      : pattern.source === "low_eval_score"
        ? "- Investigate the regressed variant; tighten or update the scenario, or fix the underlying behavior, then re-run the eval."
        : "- Reproduce the failed step locally, fix the underlying behavior, and add/adjust an outcome assertion that locks the fix in.",
  );
  return lines.join("\n");
}

/**
 * Produce a `FailureSuggestion` from a captured pattern + the project config.
 * The default path is suggest-only (a snippet for the PR comment). A mutating
 * strategy is downgraded unless `armMutations` is set; when armed, the PLAN is
 * produced but NEVER executed by this module (the executor is a gated follow-up).
 */
export function suggestFromPattern(
  pattern: FailurePattern,
  config: FailureLearningConfig = {},
): FailureSuggestion {
  const resolved = resolveFailureLearningStrategy(config);
  const snippet = renderSnippet(pattern);

  if (resolved.strategy === "auto_commit") {
    return {
      patternRef: pattern.patternRef,
      resolved,
      snippet,
      mutationPlan: {
        kind: "auto_commit",
        description:
          `Would commit a scenario/failure-catalog update for ${pattern.patternRef} ` +
          "(plan only — no git is executed by failure-learning; the executor is owner-gated).",
        executed: false,
      },
    };
  }
  if (resolved.strategy === "open_pr") {
    return {
      patternRef: pattern.patternRef,
      resolved,
      snippet,
      mutationPlan: {
        kind: "open_pr",
        description:
          `Would open a DRAFT PR with the scenario/failure-catalog update for ${pattern.patternRef} ` +
          "(plan only — no gh is executed by failure-learning; the executor is owner-gated).",
        executed: false,
      },
    };
  }
  // suggest_in_report (default, or a downgraded mutating strategy).
  return { patternRef: pattern.patternRef, resolved, snippet };
}

/**
 * Convenience: from a result + config, capture the pattern and produce the
 * suggestion in one call. Returns `undefined` when the run is not a failure
 * (no signal -> no suggestion; never fabricate one).
 */
export function learnFromRun(
  result: QaRunResult,
  config: FailureLearningConfig = {},
): { readonly pattern: FailurePattern; readonly suggestion: FailureSuggestion } | undefined {
  const pattern = captureFailurePattern(result);
  if (pattern === undefined) return undefined;
  return { pattern, suggestion: suggestFromPattern(pattern, config) };
}
