// The distiller (spec §D + §E.2): lower a `KhalaSessionTrace` into a reusable
// candidate, with TWO emitters. v1 is a DETERMINISTIC reducer over the trace
// (spec §H.1 — start deterministic; the Khala-distills-its-own-session variant
// is FUTURE).
//
//   distill(trace) -> DistillResult {
//     signatureCandidate  // typed I/O contract inferred from goal/inputs/outputs
//     moduleCandidate     // moduleKind: 'deterministic_reducer' (v1)
//     verificationClass   // honest: reflects what was actually proven
//     emitters: {
//       skill?  // (E.1) marketplace adapter — typed SEAM + TODO (FUTURE)
//       e2e?    // (E.2) executor-style Target scenario file — THE deliverable
//     }
//   }
//
// The E.2 emitter renders a black-box, executor-style e2e `Target` scenario:
// public surfaces only, role/text locators, NAMED steps that read as user
// actions, OUTCOME assertions (no implementation detail), DETERMINISTIC waits
// (no sleeps — wait on conditions, e.g. url-not-includes). The generated file is
// a real, runnable `bun test` against the qa-runner runner.
//
// Acceptance bar (spec §D.2) is enforced by `assessCandidate`: replayable,
// typed (no `any`), honestly-graded verification class, public-safe, asserts
// outcomes (not tautologies).

import {
  assertSessionTracePublicSafe,
  verifyTraceDigest,
  type KhalaSessionTrace,
  type SessionBeat,
  type TypedField,
} from "./session-trace";
import { emitSkillCandidate, type SkillCandidate } from "./skill-candidate";

export type { SkillCandidate } from "./skill-candidate";

/** The verification class the distilled candidate honestly carries. */
export type VerificationClass = "none" | "seeded" | "test_passed" | "exact_trace_replay";

/** A typed I/O contract inferred from the trace (no `any`). */
export interface BlueprintProgramSignature {
  readonly name: string;
  readonly description: string;
  readonly inputs: ReadonlyArray<TypedField>;
  readonly outputs: ReadonlyArray<TypedField>;
}

/** The distilled module candidate. v1 is a deterministic reducer. */
export interface BlueprintModuleVersionCandidate {
  readonly moduleKind: "deterministic_reducer" | "optimizer_candidate";
  /** The ordered, public-safe steps the candidate replays (from the trace). */
  readonly steps: ReadonlyArray<DistilledStep>;
}

/** One distilled step: a named user action + an optional outcome assertion. */
export type DistilledStep =
  | { readonly kind: "navigate"; readonly name: string; readonly url: string }
  | { readonly kind: "wait"; readonly name: string; readonly condition: WaitCondition }
  | { readonly kind: "screenshot"; readonly name: string; readonly label: string }
  | { readonly kind: "assert"; readonly name: string; readonly check: AssertCheck };

export type WaitCondition =
  | { readonly kind: "url-includes"; readonly value: string }
  | { readonly kind: "url-not-includes"; readonly value: string }
  | { readonly kind: "text-visible"; readonly value: string };

export type AssertCheck =
  | { readonly kind: "url-includes"; readonly value: string }
  | { readonly kind: "url-not-includes"; readonly value: string }
  | { readonly kind: "text-contains"; readonly value: string }
  | { readonly kind: "text-not-contains"; readonly value: string };

/** The E.2 emitter output: a committable executor-style e2e scenario. */
export interface E2eScenarioCandidate {
  /** A url-safe slug for the file name. */
  readonly slug: string;
  /** The human scenario name. */
  readonly scenarioName: string;
  /** The full generated TypeScript source (a runnable bun test). */
  readonly source: string;
  /** Number of outcome assertions (must be >= 1 for the acceptance bar). */
  readonly assertionCount: number;
}

export interface DistillResult {
  readonly signatureCandidate: BlueprintProgramSignature;
  readonly moduleCandidate: BlueprintModuleVersionCandidate;
  readonly verificationClass: VerificationClass;
  readonly emitters: {
    readonly e2e: E2eScenarioCandidate;
    /**
     * (E.1) The governed Blueprint optimizer skill candidate. A candidate ONLY:
     * evidence-only, Release-Gate-gated, never self-promoted (see
     * `skill-candidate.ts`). It is emitted alongside the committed e2e test from
     * the SAME capture — one pipeline, two artifacts.
     */
    readonly skill: SkillCandidate;
  };
}

export interface CandidateAssessment {
  readonly admissible: boolean;
  readonly reasons: ReadonlyArray<string>;
}

export class DistillError extends Error {
  constructor(reason: string) {
    super(`distill_error: ${reason}`);
    this.name = "DistillError";
  }
}

const slugify = (text: string): string =>
  text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60) || "scenario";

/**
 * Reduce the trace's browser beats into named, ordered distilled steps. Only
 * public-safe, deterministic surfaces are kept: navigate / wait / screenshot /
 * assert. (type/click on private selectors are not lowered into the black-box
 * scenario — the scenario asserts OUTCOMES, not implementation detail.)
 *
 * The `targetHint` carries the path/condition value (already public-safe in the
 * trace). Names read as user actions.
 */
function reduceBeats(beats: ReadonlyArray<SessionBeat>): DistilledStep[] {
  const steps: DistilledStep[] = [];
  for (const beat of beats) {
    if (beat.kind !== "browser") continue;
    switch (beat.action) {
      case "navigate":
        steps.push({ kind: "navigate", name: `open ${beat.targetHint}`, url: beat.targetHint });
        break;
      case "wait": {
        const cond = parseConditionHint(beat.targetHint);
        if (cond) steps.push({ kind: "wait", name: `wait until ${describeCondition(cond)}`, condition: cond });
        break;
      }
      case "screenshot":
        steps.push({ kind: "screenshot", name: `capture ${beat.targetHint}`, label: beat.targetHint });
        break;
      case "assert": {
        // The assert's targetHint is the model's assertion LABEL; we only emit a
        // black-box assertion when we can recover a concrete check from the label
        // (url/text). Otherwise we keep the assertion as a named outcome over the
        // page state via a text-contains on the label's quoted value.
        const check = recoverAssertCheck(beat.targetHint);
        if (check) steps.push({ kind: "assert", name: `verify ${beat.targetHint}`, check });
        break;
      }
      default:
        // click / type / readText are not lowered into the black-box scenario.
        break;
    }
  }
  return steps;
}

/** Parse a wait condition from a `kind:value` target hint. */
function parseConditionHint(hint: string): WaitCondition | undefined {
  const idx = hint.indexOf(":");
  if (idx === -1) return undefined;
  const kind = hint.slice(0, idx);
  const value = hint.slice(idx + 1);
  if (kind === "url-includes" || kind === "url-not-includes" || kind === "text-visible") {
    return { kind, value };
  }
  return undefined;
}

function describeCondition(cond: WaitCondition): string {
  switch (cond.kind) {
    case "url-includes":
      return `the URL includes "${cond.value}"`;
    case "url-not-includes":
      return `the URL no longer includes "${cond.value}"`;
    case "text-visible":
      return `"${cond.value}" is visible`;
  }
}

/**
 * Recover a concrete black-box assert check from the model's assertion label.
 * Heuristic but deterministic: a label mentioning a /path implies a url check;
 * a quoted phrase implies a text-contains. Labels with "no redirect"/"stays"
 * map to url-includes; "redirects"/"away" map to url-not-includes.
 */
function recoverAssertCheck(label: string): AssertCheck | undefined {
  const lower = label.toLowerCase();
  // Recover a quoted phrase the assertion is about (single, double, or curly
  // quotes). The phrase becomes a text-contains / -not-contains check.
  const quoted = /["'“‘]([^"'”’]{2,})["'”’]/.exec(label);
  const path = /(\/[a-z0-9][a-z0-9/_-]*)/i.exec(label);
  if (quoted) {
    const neg = /\b(no|not|never|without)\b/.test(lower);
    return neg
      ? { kind: "text-not-contains", value: quoted[1]! }
      : { kind: "text-contains", value: quoted[1]! };
  }
  if (path) {
    const away = /\b(redirect|away|leaves?|not)\b/.test(lower) && !/\bstays?\b/.test(lower);
    return away
      ? { kind: "url-not-includes", value: path[1]! }
      : { kind: "url-includes", value: path[1]! };
  }
  return undefined;
}

/** Infer a typed signature from the trace (no `any`). */
function inferSignature(trace: KhalaSessionTrace): BlueprintProgramSignature {
  return {
    name: slugify(trace.goal).replace(/-/g, "_"),
    description: trace.goal,
    inputs: trace.inputs,
    outputs: trace.outputs,
  };
}

/** Map the trace's terminal verdict to an honest distilled verification class. */
function honestVerificationClass(trace: KhalaSessionTrace): VerificationClass {
  const verdict = trace.beats.find((b): b is Extract<SessionBeat, { kind: "verdict" }> => b.kind === "verdict");
  switch (verdict?.verificationClass) {
    case "test_passed":
      // A deterministic-replayable scenario whose digest matches is exact replay.
      return verifyTraceDigest(trace) ? "exact_trace_replay" : "test_passed";
    case "seeded":
      return "seeded";
    case "exact_trace_replay":
      return "exact_trace_replay";
    default:
      // failed / none -> nothing was honestly proven.
      return "none";
  }
}

/**
 * Assess a distilled candidate against the spec §D.2 acceptance bar. Returns the
 * reasons it is/ isn't admissible. The distiller computes this; promotion is a
 * separate (owner-gated) Release Gate concern.
 */
export function assessCandidate(result: DistillResult, trace: KhalaSessionTrace): CandidateAssessment {
  const reasons: string[] = [];
  // 1. Replayable: the trace digest must re-derive.
  if (!verifyTraceDigest(trace)) reasons.push("not replayable: trace digest does not re-derive");
  // 2. Typed: no `any` in the signature.
  const anyTyped = [...result.signatureCandidate.inputs, ...result.signatureCandidate.outputs].some(
    (f) => f.type.toLowerCase() === "any" || f.type.trim() === "",
  );
  if (anyTyped) reasons.push("not typed: a signature field has type 'any' or empty");
  // 3. Honestly graded: a failed/none verdict must NOT claim a passing class.
  if (result.verificationClass === "none" && trace.beats.some((b) => b.kind === "verdict" && b.verificationClass === "test_passed")) {
    reasons.push("dishonest grade: verdict says test_passed but class is none");
  }
  // 4. Public-safe (throws if not).
  try {
    assertSessionTracePublicSafe(trace);
  } catch (error) {
    reasons.push(`not public-safe: ${error instanceof Error ? error.message : String(error)}`);
  }
  // 5. Asserts outcomes (>=1 assertion, no tautology — a check must carry a value).
  if (result.emitters.e2e.assertionCount < 1) reasons.push("no outcome assertions");
  return { admissible: reasons.length === 0, reasons };
}

/**
 * Distill a `KhalaSessionTrace` into a `DistillResult`. Deterministic v1 reducer.
 * Throws `DistillError` if the trace is not public-safe (fail closed).
 */
export function distill(trace: KhalaSessionTrace): DistillResult {
  // Fail closed on a non-public-safe trace — never emit a candidate from one.
  try {
    assertSessionTracePublicSafe(trace);
  } catch (error) {
    throw new DistillError(`trace is not public-safe: ${error instanceof Error ? error.message : String(error)}`);
  }

  const signatureCandidate = inferSignature(trace);
  const steps = reduceBeats(trace.beats);
  const verificationClass = honestVerificationClass(trace);
  const e2e = emitE2eScenario(trace, signatureCandidate, steps, verificationClass);
  // (E.1) The governed optimizer skill candidate — emitted from the SAME capture
  // as the e2e test. A candidate only: evidence-only, Release-Gate-gated, never
  // self-promoted. (FUTURE marketplace listing + rev-share settlement stay
  // INERT/OWNER-GATED in `run-settlement.ts`; this is committable evidence today.)
  const skill = emitSkillCandidate({ trace, signature: signatureCandidate, verificationClass, slug: e2e.slug });

  return {
    signatureCandidate,
    moduleCandidate: { moduleKind: "deterministic_reducer", steps },
    verificationClass,
    emitters: { e2e, skill },
  };
}

// ---------------------------------------------------------------------------
// E.2 emitter: render the executor-style black-box scenario source.
// ---------------------------------------------------------------------------

function emitE2eScenario(
  trace: KhalaSessionTrace,
  signature: BlueprintProgramSignature,
  steps: ReadonlyArray<DistilledStep>,
  verificationClass: VerificationClass,
): E2eScenarioCandidate {
  const scenarioName = trace.goal;
  const slug = `${slugify(trace.goal)}`;
  const assertionCount = steps.filter((s) => s.kind === "assert").length;
  const source = renderScenarioSource({ trace, signature, steps, verificationClass, scenarioName, slug });
  return { slug, scenarioName, source, assertionCount };
}

function renderScenarioSource(input: {
  readonly trace: KhalaSessionTrace;
  readonly signature: BlueprintProgramSignature;
  readonly steps: ReadonlyArray<DistilledStep>;
  readonly verificationClass: VerificationClass;
  readonly scenarioName: string;
  readonly slug: string;
}): string {
  const { trace, steps, verificationClass, scenarioName } = input;
  const stepLines = steps.map(renderStep).join("\n");

  return `// GENERATED by the qa-runner distiller (spec §E.2). Do not hand-edit; re-distill.
//
// Black-box, executor-style e2e scenario distilled from a Khala-driven session.
//   goal:               ${escapeComment(scenarioName)}
//   model:              ${trace.model}
//   verification class: ${verificationClass}
//   source digest:      ${trace.digest}
//
// It drives ONLY public surfaces, with named steps that read as user actions,
// outcome assertions (no implementation detail), and DETERMINISTIC waits (no
// sleeps — waits are conditions). Pointing TARGET_URL at dev or prod runs the
// same scenario against either deployment.

import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import { localBackend } from "../src/backend";
import { scriptedBrain, type BrainStep } from "../src/brain";
import { runQaSession } from "../src/runner";
import { makeTarget } from "../src/target";

const TARGET_URL = process.env.TARGET_URL ?? "${trace.target.baseUrl}";

// Named steps that read as user actions (the review artifact).
const steps: ReadonlyArray<BrainStep> = [
${stepLines}
];

describe(${quote(`distilled: ${scenarioName}`)}, () => {
  test(
    "verifies the flow against the target (black-box)",
    async () => {
      const outcome = await Effect.runPromise(
        runQaSession({
          target: makeTarget({ name: "distilled-target", baseUrl: TARGET_URL }),
          brain: scriptedBrain(steps),
          backend: localBackend(),
          artifactDir: process.env.ARTIFACT_DIR ?? "./runs/distilled-${input.slug}",
        }),
      );
      // The scenario PASSES only if every named step + outcome assertion held.
      expect(outcome.result.failure).toBeUndefined();
      expect(outcome.result.status).toBe("pass");
    },
    120_000,
  );
});
`;
}

function renderStep(step: DistilledStep): string {
  switch (step.kind) {
    case "navigate":
      return `  { kind: "navigate", url: ${quote(step.url)}, label: ${quote(step.name)} },`;
    case "wait":
      return `  { kind: "wait-for", condition: ${renderCondition(step.condition)}, label: ${quote(step.name)} },`;
    case "screenshot":
      return `  { kind: "screenshot", label: ${quote(step.label)} },`;
    case "assert":
      return `  { kind: "assert", label: ${quote(step.name)}, check: ${renderCheck(step.check)} },`;
  }
}

function renderCondition(cond: WaitCondition): string {
  return `{ kind: ${quote(cond.kind)}, value: ${quote(cond.value)} }`;
}

function renderCheck(check: AssertCheck): string {
  return `{ kind: ${quote(check.kind)}, value: ${quote(check.value)} }`;
}

function quote(text: string): string {
  return JSON.stringify(text);
}

function escapeComment(text: string): string {
  return text.replace(/\*\//g, "*\\/").replace(/[\r\n]+/g, " ");
}
