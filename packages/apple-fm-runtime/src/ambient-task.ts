import { Clock, Context, Duration, Effect, Exit, Fiber, Layer, Option, Schema as S, Semaphore } from "effect";
import type { Scope } from "effect";

import {
  brandedTurnRef,
  ContextManifestRef,
  DebugRef,
  RunRef,
  SourceControlRef,
  TurnTaskClass,
  TurnUsageTruth,
  type TurnRefusalReason,
} from "@openagentsinc/agent-runtime-schema";

import { APPLE_FM_DATA_DESTINATION, APPLE_FM_DEFAULT_MODEL_ID } from "./identity.js";
import type { AppleFmCompletionTurn } from "./client.js";

/**
 * `@openagentsinc/apple-fm-runtime` bounded ambient local tasks (AFS-07).
 *
 * Apple FM here does SMALL ADVISORY work over deterministic host facts and adds
 * NO action authority. Each ambient task is a separate typed signature with its
 * own input, time, output, concurrency, and thermal bounds. A task is bounded,
 * cancellable, and non-blocking: it never blocks startup, composer send, apply,
 * run, debug, commit, push, or release. Unsupported, slow, cancelled, and
 * resource-limited tasks DEGRADE — they resolve to a typed non-completion, never
 * a thrown failure or a lost deterministic capability.
 *
 * Zero-token invariant: a local Apple FM ambient task never produces a provider
 * token row. Its provenance carries `usageTruth` only (estimated or unknown for
 * on-device inference); it never mints an exact token count unless the bridge
 * itself supplies exact usage, and it never emits a billing/accounting artifact.
 * The advisory result stays on the device (`on_device_local`).
 *
 * This module is portable: it imports no Node, platform API, provider SDK,
 * store driver, or cloud client. The Desktop host composes the concrete
 * `AmbientInference` and `AmbientResourceGate` adapters over the main-owned
 * Apple FM helper; the renderer never supplies readiness, facts, or authority.
 */

export const AMBIENT_TASK_PROVENANCE_SCHEMA_LITERAL =
  "openagents.apple_fm.ambient_task_provenance.v1" as const;

/**
 * The bounded ambient task kinds, in the AFS-07 task order: commit-message
 * draft; failed-test and diagnostic explanation; context and diff summary;
 * debug-state explanation; boot explanation. Completion and next-edit are NOT
 * ambient tasks here — the plan gates them behind AFS-09.
 */
export const AmbientTaskKind = S.Literals([
  "commit_message_draft",
  "explain_test_failure",
  "explain_diagnostic",
  "summarize_context",
  "summarize_diff",
  "explain_debug_state",
  "explain_boot",
]);
export type AmbientTaskKind = typeof AmbientTaskKind.Type;

export const AMBIENT_TASK_KINDS: ReadonlyArray<AmbientTaskKind> = [
  "commit_message_draft",
  "explain_test_failure",
  "explain_diagnostic",
  "summarize_context",
  "summarize_diff",
  "explain_debug_state",
  "explain_boot",
];

/**
 * The device thermal state a resource gate reports. A task declares the WORST
 * thermal state at which it may still run; above that ceiling it degrades.
 */
export const AmbientThermalState = S.Literals(["nominal", "fair", "serious", "critical"]);
export type AmbientThermalState = typeof AmbientThermalState.Type;

const thermalRank: Readonly<Record<AmbientThermalState, number>> = {
  nominal: 0,
  fair: 1,
  serious: 2,
  critical: 3,
};

/** Why an ambient task degraded. Every value is advisory-safe: no failure surface. */
export type AmbientDegradeReason =
  | "not_ready"
  | "unsupported"
  | "timed_out"
  | "resource_limited"
  | "input_too_large";

/** Per-task separate bounds: input, time, output, concurrency, and thermal. */
export interface AmbientTaskBounds {
  readonly maxInputChars: number;
  readonly timeoutMs: number;
  readonly maxOutputChars: number;
  readonly maxConcurrency: number;
  readonly thermalCeiling: AmbientThermalState;
}

/** A branded reference to a recorded boot sequence (no BootRef exists upstream). */
export const AmbientBootRef = brandedTurnRef("AmbientBootRef");
export type AmbientBootRef = typeof AmbientBootRef.Type;

const boundedFact = (max: number) => S.String.check(S.isMinLength(1), S.isMaxLength(max));
const boundedCount = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(100_000));

// --- Typed per-task input signatures (deterministic host facts) ---------------

export const CommitMessageDraftInput = S.Struct({
  sourceControlRef: SourceControlRef,
  branch: boundedFact(200),
  stagedFileCount: boundedCount,
  diffSummary: boundedFact(3600),
});
export type CommitMessageDraftInput = typeof CommitMessageDraftInput.Type;

export const CommitMessageDraftOutput = S.Struct({
  subject: S.String.check(S.isMinLength(1), S.isMaxLength(120)),
  body: S.String.check(S.isMaxLength(600)),
});
export type CommitMessageDraftOutput = typeof CommitMessageDraftOutput.Type;

export const TestFailureExplanationInput = S.Struct({
  runRef: RunRef,
  testName: boundedFact(400),
  failureOutput: boundedFact(3400),
});
export type TestFailureExplanationInput = typeof TestFailureExplanationInput.Type;

export const DiagnosticExplanationInput = S.Struct({
  contextManifestRef: ContextManifestRef,
  diagnosticCode: boundedFact(120),
  diagnosticMessage: boundedFact(1200),
  sourceExcerpt: boundedFact(2400),
});
export type DiagnosticExplanationInput = typeof DiagnosticExplanationInput.Type;

export const ContextSummaryInput = S.Struct({
  contextManifestRef: ContextManifestRef,
  itemCount: boundedCount,
  factText: boundedFact(3400),
});
export type ContextSummaryInput = typeof ContextSummaryInput.Type;

export const DiffSummaryInput = S.Struct({
  sourceControlRef: SourceControlRef,
  changedFileCount: boundedCount,
  diffText: boundedFact(3400),
});
export type DiffSummaryInput = typeof DiffSummaryInput.Type;

export const DebugStateExplanationInput = S.Struct({
  debugRef: DebugRef,
  stoppedReason: boundedFact(400),
  stateText: boundedFact(3400),
});
export type DebugStateExplanationInput = typeof DebugStateExplanationInput.Type;

export const BootExplanationInput = S.Struct({
  bootRef: AmbientBootRef,
  bootSequenceText: boundedFact(2400),
});
export type BootExplanationInput = typeof BootExplanationInput.Type;

/** The shared advisory-explanation output for the explanation/summary tasks. */
export const AmbientExplanationOutput = S.Struct({
  explanation: S.String.check(S.isMinLength(1), S.isMaxLength(1200)),
});
export type AmbientExplanationOutput = typeof AmbientExplanationOutput.Type;

// --- Advisory provenance ------------------------------------------------------

/**
 * The explicit advisory provenance recorded for every ambient result. It marks
 * the result advisory, names the on-device destination and local cost class,
 * carries only honest usage truth (never an invented exact token count), and
 * preserves the deterministic host-fact references the task consumed.
 */
export const AmbientTaskProvenance = S.Struct({
  schema: S.Literal(AMBIENT_TASK_PROVENANCE_SCHEMA_LITERAL),
  advisory: S.Literal(true),
  kind: AmbientTaskKind,
  taskClass: TurnTaskClass,
  model: S.String.check(S.isMinLength(1), S.isMaxLength(200)),
  dataDestination: S.Literal(APPLE_FM_DATA_DESTINATION),
  costClass: S.Literal("local_resource_only"),
  usageTruth: TurnUsageTruth,
  latencyMs: S.Number.check(S.isGreaterThanOrEqualTo(0)),
  factRefs: S.Array(S.String.check(S.isMaxLength(256))).check(S.isMaxLength(8)),
});
export type AmbientTaskProvenance = typeof AmbientTaskProvenance.Type;

// --- Task outcome -------------------------------------------------------------

/**
 * The typed outcome of one ambient task. Only `Completed` carries an advisory
 * result. `Refused` keeps the exact refusal reason (an empty, oversized,
 * action-claiming, or below-quality-floor model output). `Degraded` and
 * `Cancelled` are advisory-safe non-completions: they surface no failure and
 * cost no deterministic host capability.
 */
export type AmbientTaskOutcome<O> =
  | { readonly _tag: "Completed"; readonly kind: AmbientTaskKind; readonly result: O; readonly provenance: AmbientTaskProvenance }
  | { readonly _tag: "Refused"; readonly kind: AmbientTaskKind; readonly reason: TurnRefusalReason; readonly provenance: AmbientTaskProvenance }
  | { readonly _tag: "Degraded"; readonly kind: AmbientTaskKind; readonly reason: AmbientDegradeReason }
  | { readonly _tag: "Cancelled"; readonly kind: AmbientTaskKind };

const degraded = (kind: AmbientTaskKind, reason: AmbientDegradeReason): AmbientTaskOutcome<never> => ({
  _tag: "Degraded",
  kind,
  reason,
});

const cancelled = (kind: AmbientTaskKind): AmbientTaskOutcome<never> => ({ _tag: "Cancelled", kind });

// --- Quality floor ------------------------------------------------------------

/**
 * A first-person completed-action claim from a tool-less, memory-less advisory
 * task is a hallucination: the task cannot have run a command, changed a file,
 * committed, pushed, dispatched an agent, set a reminder, or remembered across
 * chats. Any such claim is refused, never surfaced as an advisory result.
 */
const AMBIENT_ACTION_CLAIM_PATTERNS: ReadonlyArray<RegExp> = [
  /\bI (?:ran|executed) (?:the |a |your )?(?:command|shell|script|test)/iu,
  /\bI (?:edited|modified|changed|created|deleted|wrote|removed|applied) (?:the |a |your )?(?:file|change|edit|patch)/iu,
  /\bI (?:committed|pushed|staged|merged|reverted|checked out)\b/iu,
  /\bI (?:dispatched|delegated|spawned|launched) (?:a |an |the )?(?:sub-?)?agent/iu,
  /\bI set (?:a |the )?reminder/iu,
  /\bI(?:'ll| will)? remember (?:this|that|it) (?:across|for future|between)/iu,
];

const claimsAction = (text: string): boolean =>
  AMBIENT_ACTION_CLAIM_PATTERNS.some((pattern) => pattern.test(text));

/** The bounded quality-floor decision over a raw advisory completion. */
type QualityFloor =
  | { readonly _tag: "Text"; readonly text: string }
  | { readonly _tag: "Refuse"; readonly reason: TurnRefusalReason };

/**
 * Apply the shared advisory quality floor: reject empty, oversized, and
 * action-claiming output, and reject output below the minimum useful length.
 */
const applyQualityFloor = (
  rawText: string,
  bounds: { readonly minChars: number; readonly maxChars: number },
): QualityFloor => {
  const trimmed = rawText.trim();
  if (trimmed.length === 0) return { _tag: "Refuse", reason: "empty_output" };
  if (rawText.length > bounds.maxChars) return { _tag: "Refuse", reason: "oversized_output" };
  if (claimsAction(rawText)) return { _tag: "Refuse", reason: "action_claim_rejected" };
  if (trimmed.length < bounds.minChars) return { _tag: "Refuse", reason: "malformed_output" };
  return { _tag: "Text", text: trimmed };
};

// --- Signature ----------------------------------------------------------------

/** The bounded quality-floor decode result for a task. */
export type AmbientResultDecode<O> =
  | { readonly _tag: "Ok"; readonly result: O }
  | { readonly _tag: "Refuse"; readonly reason: TurnRefusalReason };

/**
 * A typed ambient task signature. It binds the task kind and advisory task
 * class, the deterministic input schema, the advisory output schema, the
 * per-task bounds, a DETERMINISTIC prompt builder over the facts, a
 * quality-floor result decoder, and a preserved fact-reference extractor.
 */
export interface AmbientTaskSignature<I, O> {
  readonly kind: AmbientTaskKind;
  readonly taskClass: TurnTaskClass;
  readonly title: string;
  readonly input: S.Schema<I>;
  readonly output: S.Schema<O>;
  readonly bounds: AmbientTaskBounds;
  readonly buildPrompt: (facts: I) => string;
  readonly decodeResult: (input: { readonly facts: I; readonly rawText: string }) => AmbientResultDecode<O>;
  readonly factRefs: (facts: I) => ReadonlyArray<string>;
}

interface MakeAmbientTaskSignatureArgs<I, O> {
  readonly kind: AmbientTaskKind;
  readonly taskClass: TurnTaskClass;
  readonly title: string;
  readonly input: S.Schema<I>;
  readonly output: S.Schema<O>;
  readonly bounds: AmbientTaskBounds;
  readonly buildPrompt: (facts: I) => string;
  readonly decodeResult: (input: { readonly facts: I; readonly rawText: string }) => AmbientResultDecode<O>;
  readonly factRefs: (facts: I) => ReadonlyArray<string>;
}

const makeAmbientTaskSignature = <I, O>(args: MakeAmbientTaskSignatureArgs<I, O>): AmbientTaskSignature<I, O> => args;

/** A pure decoder over an explanation completion using the shared quality floor. */
const decodeExplanation = (
  rawText: string,
  bounds: AmbientTaskBounds,
): AmbientResultDecode<AmbientExplanationOutput> => {
  const floor = applyQualityFloor(rawText, { minChars: 16, maxChars: bounds.maxOutputChars });
  if (floor._tag === "Refuse") return { _tag: "Refuse", reason: floor.reason };
  const decoded = decodeExplanationExit({ explanation: floor.text.slice(0, 1200) });
  if (decoded._tag === "Failure") return { _tag: "Refuse", reason: "malformed_output" };
  return { _tag: "Ok", result: decoded.value };
};

const decodeExplanationExit = S.decodeUnknownExit(AmbientExplanationOutput);
const decodeCommitExit = S.decodeUnknownExit(CommitMessageDraftOutput);

// --- The seven ambient task signatures ---------------------------------------

export const commitMessageDraftSignature: AmbientTaskSignature<
  CommitMessageDraftInput,
  CommitMessageDraftOutput
> = makeAmbientTaskSignature({
  kind: "commit_message_draft",
  taskClass: "draft_commit_message",
  title: "Draft a commit message from staged changes",
  input: CommitMessageDraftInput,
  output: CommitMessageDraftOutput,
  bounds: { maxInputChars: 4000, timeoutMs: 8000, maxOutputChars: 600, maxConcurrency: 1, thermalCeiling: "fair" },
  buildPrompt: (facts) =>
    [
      "Draft a concise Git commit message for the staged changes below.",
      "Write one short imperative subject line, then an optional body.",
      "Do not claim to have committed, pushed, or run anything.",
      `Branch: ${facts.branch}`,
      `Staged files: ${facts.stagedFileCount}`,
      "Diff summary:",
      facts.diffSummary,
    ].join("\n"),
  decodeResult: ({ rawText }) => {
    const floor = applyQualityFloor(rawText, { minChars: 3, maxChars: 600 });
    if (floor._tag === "Refuse") return { _tag: "Refuse", reason: floor.reason };
    const lines = floor.text.split("\n");
    const subjectLine = lines.find((line) => line.trim().length > 0) ?? "";
    const subject = subjectLine.trim().slice(0, 120);
    const bodyStart = lines.indexOf(subjectLine) + 1;
    const body = lines.slice(bodyStart).join("\n").trim().slice(0, 600);
    const decoded = decodeCommitExit({ subject, body });
    if (decoded._tag === "Failure") return { _tag: "Refuse", reason: "malformed_output" };
    return { _tag: "Ok", result: decoded.value };
  },
  factRefs: (facts) => [facts.sourceControlRef],
});

export const testFailureExplanationSignature: AmbientTaskSignature<
  TestFailureExplanationInput,
  AmbientExplanationOutput
> = makeAmbientTaskSignature({
  kind: "explain_test_failure",
  taskClass: "explain_failure",
  title: "Explain a failed test",
  input: TestFailureExplanationInput,
  output: AmbientExplanationOutput,
  bounds: { maxInputChars: 4000, timeoutMs: 12_000, maxOutputChars: 1200, maxConcurrency: 1, thermalCeiling: "fair" },
  buildPrompt: (facts) =>
    [
      "Explain, for a developer, why the test below failed and what to check.",
      "Be specific and do not claim to have fixed or run anything.",
      `Test: ${facts.testName}`,
      "Failure output:",
      facts.failureOutput,
    ].join("\n"),
  decodeResult: ({ rawText }) => decodeExplanation(rawText, testFailureExplanationSignature.bounds),
  factRefs: (facts) => [facts.runRef],
});

export const diagnosticExplanationSignature: AmbientTaskSignature<
  DiagnosticExplanationInput,
  AmbientExplanationOutput
> = makeAmbientTaskSignature({
  kind: "explain_diagnostic",
  taskClass: "explain_failure",
  title: "Explain a language diagnostic",
  input: DiagnosticExplanationInput,
  output: AmbientExplanationOutput,
  bounds: { maxInputChars: 4000, timeoutMs: 10_000, maxOutputChars: 1000, maxConcurrency: 2, thermalCeiling: "fair" },
  buildPrompt: (facts) =>
    [
      "Explain the language diagnostic below and suggest what to check.",
      "Do not claim to have edited the file.",
      `Code: ${facts.diagnosticCode}`,
      `Message: ${facts.diagnosticMessage}`,
      "Source excerpt:",
      facts.sourceExcerpt,
    ].join("\n"),
  decodeResult: ({ rawText }) => decodeExplanation(rawText, diagnosticExplanationSignature.bounds),
  factRefs: (facts) => [facts.contextManifestRef],
});

export const contextSummarySignature: AmbientTaskSignature<
  ContextSummaryInput,
  AmbientExplanationOutput
> = makeAmbientTaskSignature({
  kind: "summarize_context",
  taskClass: "local_answer",
  title: "Summarize the assembled context",
  input: ContextSummaryInput,
  output: AmbientExplanationOutput,
  bounds: { maxInputChars: 4000, timeoutMs: 12_000, maxOutputChars: 1200, maxConcurrency: 1, thermalCeiling: "nominal" },
  buildPrompt: (facts) =>
    [
      "Summarize the assembled work context below for a developer.",
      "Describe only what the facts contain; invent nothing.",
      `Context items: ${facts.itemCount}`,
      "Context facts:",
      facts.factText,
    ].join("\n"),
  decodeResult: ({ rawText }) => decodeExplanation(rawText, contextSummarySignature.bounds),
  factRefs: (facts) => [facts.contextManifestRef],
});

export const diffSummarySignature: AmbientTaskSignature<
  DiffSummaryInput,
  AmbientExplanationOutput
> = makeAmbientTaskSignature({
  kind: "summarize_diff",
  taskClass: "local_answer",
  title: "Summarize a working-tree diff",
  input: DiffSummaryInput,
  output: AmbientExplanationOutput,
  bounds: { maxInputChars: 4000, timeoutMs: 12_000, maxOutputChars: 1000, maxConcurrency: 1, thermalCeiling: "fair" },
  buildPrompt: (facts) =>
    [
      "Summarize the working-tree diff below at a high level.",
      "Do not claim to have staged, committed, or applied anything.",
      `Changed files: ${facts.changedFileCount}`,
      "Diff:",
      facts.diffText,
    ].join("\n"),
  decodeResult: ({ rawText }) => decodeExplanation(rawText, diffSummarySignature.bounds),
  factRefs: (facts) => [facts.sourceControlRef],
});

export const debugStateExplanationSignature: AmbientTaskSignature<
  DebugStateExplanationInput,
  AmbientExplanationOutput
> = makeAmbientTaskSignature({
  kind: "explain_debug_state",
  taskClass: "explain_debug",
  title: "Explain the current debug state",
  input: DebugStateExplanationInput,
  output: AmbientExplanationOutput,
  bounds: { maxInputChars: 4000, timeoutMs: 12_000, maxOutputChars: 1200, maxConcurrency: 1, thermalCeiling: "fair" },
  buildPrompt: (facts) =>
    [
      "Explain the paused debug state below for a developer.",
      "Do not claim to have stepped, continued, or changed the session.",
      `Stopped reason: ${facts.stoppedReason}`,
      "Debug state:",
      facts.stateText,
    ].join("\n"),
  decodeResult: ({ rawText }) => decodeExplanation(rawText, debugStateExplanationSignature.bounds),
  factRefs: (facts) => [facts.debugRef],
});

export const bootExplanationSignature: AmbientTaskSignature<
  BootExplanationInput,
  AmbientExplanationOutput
> = makeAmbientTaskSignature({
  kind: "explain_boot",
  taskClass: "local_answer",
  title: "Explain the boot sequence",
  input: BootExplanationInput,
  output: AmbientExplanationOutput,
  bounds: { maxInputChars: 3000, timeoutMs: 6000, maxOutputChars: 800, maxConcurrency: 1, thermalCeiling: "nominal" },
  buildPrompt: (facts) =>
    [
      "Explain the app boot sequence below in one short paragraph.",
      "Describe only what the rows show.",
      "Boot sequence:",
      facts.bootSequenceText,
    ].join("\n"),
  decodeResult: ({ rawText }) => decodeExplanation(rawText, bootExplanationSignature.bounds),
  factRefs: (facts) => [facts.bootRef],
});

/** The per-kind bounds table, derived from the signatures for external inspection. */
export const AMBIENT_TASK_BOUNDS: Readonly<Record<AmbientTaskKind, AmbientTaskBounds>> = {
  commit_message_draft: commitMessageDraftSignature.bounds,
  explain_test_failure: testFailureExplanationSignature.bounds,
  explain_diagnostic: diagnosticExplanationSignature.bounds,
  summarize_context: contextSummarySignature.bounds,
  summarize_diff: diffSummarySignature.bounds,
  explain_debug_state: debugStateExplanationSignature.bounds,
  explain_boot: bootExplanationSignature.bounds,
};

/** A non-generic catalog row for external inspection (no generic widening). */
export interface AmbientTaskCatalogEntry {
  readonly kind: AmbientTaskKind;
  readonly taskClass: TurnTaskClass;
  readonly title: string;
  readonly bounds: AmbientTaskBounds;
}

export const ambientTaskCatalog: ReadonlyArray<AmbientTaskCatalogEntry> = [
  commitMessageDraftSignature,
  testFailureExplanationSignature,
  diagnosticExplanationSignature,
  contextSummarySignature,
  diffSummarySignature,
  debugStateExplanationSignature,
  bootExplanationSignature,
].map((signature) => ({
  kind: signature.kind,
  taskClass: signature.taskClass,
  title: signature.title,
  bounds: signature.bounds,
}));

// --- Provenance construction --------------------------------------------------

const makeProvenance = (
  kind: AmbientTaskKind,
  taskClass: TurnTaskClass,
  model: string,
  usageTruth: TurnUsageTruth,
  latencyMs: number,
  factRefs: ReadonlyArray<string>,
): AmbientTaskProvenance => ({
  schema: AMBIENT_TASK_PROVENANCE_SCHEMA_LITERAL,
  advisory: true,
  kind,
  taskClass,
  model,
  dataDestination: APPLE_FM_DATA_DESTINATION,
  costClass: "local_resource_only",
  usageTruth,
  latencyMs: Math.max(0, Math.round(latencyMs)),
  factRefs: factRefs.slice(0, 8),
});

// --- Ports --------------------------------------------------------------------

export interface AmbientInferenceInput {
  readonly kind: AmbientTaskKind;
  /** The DETERMINISTIC prompt the task built from host facts. */
  readonly prompt: string;
  readonly maxOutputChars: number;
}

/**
 * `AmbientInference` runs one bounded on-device completion. The Desktop host
 * composes it over the main-owned Apple FM helper; it never fails (a transport,
 * readiness, or shape problem maps to a failed completion turn), so the runner
 * can keep its total, non-throwing contract.
 */
export interface AmbientInferenceShape {
  readonly complete: (input: AmbientInferenceInput) => Effect.Effect<AppleFmCompletionTurn>;
}
export class AmbientInference extends Context.Service<AmbientInference, AmbientInferenceShape>()(
  "apple-fm-runtime.AmbientInference",
) {}

export interface AmbientResourceSnapshot {
  readonly appleFmReady: boolean;
  readonly thermalState: AmbientThermalState;
  readonly underMemoryPressure: boolean;
}

/**
 * `AmbientResourceGate` reports the current on-device readiness and resource
 * pressure. The runner reads it BEFORE inference so a not-ready helper, a
 * thermal ceiling breach, or memory pressure degrades the task without any work
 * or failure surface.
 */
export interface AmbientResourceGateShape {
  readonly snapshot: Effect.Effect<AmbientResourceSnapshot>;
}
export class AmbientResourceGate extends Context.Service<AmbientResourceGate, AmbientResourceGateShape>()(
  "apple-fm-runtime.AmbientResourceGate",
) {}

// --- Runner -------------------------------------------------------------------

export interface AmbientTaskRunInput<I, O> {
  readonly signature: AmbientTaskSignature<I, O>;
  readonly facts: I;
}

/**
 * `AmbientTaskRunner` runs one ambient task to a typed outcome. `run` is TOTAL
 * and never fails: every gate, timeout, and inference problem resolves to a
 * `Degraded` outcome, and every model-output problem resolves to `Refused`. It
 * is cancellable through Effect interruption (see `runAmbientTaskDetached`).
 */
export interface AmbientTaskRunnerShape {
  readonly run: <I, O>(input: AmbientTaskRunInput<I, O>) => Effect.Effect<AmbientTaskOutcome<O>>;
}
export class AmbientTaskRunner extends Context.Service<AmbientTaskRunner, AmbientTaskRunnerShape>()(
  "apple-fm-runtime.AmbientTaskRunner",
) {}

/**
 * The default runner layer. It builds one semaphore per kind from the per-task
 * concurrency bound, gates on readiness/thermal/memory/input-size, enforces the
 * per-task timeout, admits work only when a concurrency slot is immediately
 * available (never blocking), and applies the per-task quality floor.
 */
export const AmbientTaskRunnerLayer: Layer.Layer<
  AmbientTaskRunner,
  never,
  AmbientInference | AmbientResourceGate
> = Layer.effect(
  AmbientTaskRunner,
  Effect.gen(function* () {
    const inference = yield* AmbientInference;
    const gate = yield* AmbientResourceGate;

    const semaphores = new Map<AmbientTaskKind, Semaphore.Semaphore>();
    for (const kind of AMBIENT_TASK_KINDS) {
      semaphores.set(kind, yield* Semaphore.make(AMBIENT_TASK_BOUNDS[kind].maxConcurrency));
    }

    const run = <I, O>(input: AmbientTaskRunInput<I, O>): Effect.Effect<AmbientTaskOutcome<O>> =>
      Effect.gen(function* () {
        const { signature, facts } = input;
        const { bounds, kind } = signature;
        const startedAt = yield* Clock.currentTimeMillis;

        const snapshot = yield* gate.snapshot;
        if (!snapshot.appleFmReady) return degraded(kind, "not_ready");
        if (snapshot.underMemoryPressure || thermalRank[snapshot.thermalState] > thermalRank[bounds.thermalCeiling]) {
          return degraded(kind, "resource_limited");
        }

        const prompt = signature.buildPrompt(facts);
        if (prompt.length > bounds.maxInputChars) return degraded(kind, "input_too_large");

        const semaphore = semaphores.get(kind);
        if (semaphore === undefined) return degraded(kind, "unsupported");

        const guarded = inference
          .complete({ kind, prompt, maxOutputChars: bounds.maxOutputChars })
          .pipe(Effect.timeoutOption(Duration.millis(bounds.timeoutMs)));

        // Admit only when a concurrency slot is immediately free: a busy kind
        // degrades instead of queueing, so the task stays bounded and non-blocking.
        const admitted = yield* semaphore.withPermitsIfAvailable(1)(guarded);
        if (Option.isNone(admitted)) return degraded(kind, "resource_limited");
        if (Option.isNone(admitted.value)) return degraded(kind, "timed_out");

        const turn = admitted.value.value;
        const latencyMs = (yield* Clock.currentTimeMillis) - startedAt;
        if (turn.outcome === "failed" || turn.text === undefined) return degraded(kind, "not_ready");

        const provenance = makeProvenance(
          kind,
          signature.taskClass,
          APPLE_FM_DEFAULT_MODEL_ID,
          // Honest usage truth only: never invent an exact token count on device.
          turn.usageTruth,
          latencyMs,
          signature.factRefs(facts),
        );

        const decoded = signature.decodeResult({ facts, rawText: turn.text });
        if (decoded._tag === "Refuse") {
          return { _tag: "Refused", kind, reason: decoded.reason, provenance } satisfies AmbientTaskOutcome<O>;
        }
        return { _tag: "Completed", kind, result: decoded.result, provenance } satisfies AmbientTaskOutcome<O>;
      });

    return AmbientTaskRunner.of({ run });
  }),
);

// --- Detached, cancellable, non-blocking dispatch -----------------------------

/** A handle over a forked ambient task: await its outcome or cancel it. */
export interface AmbientTaskHandle<O> {
  /** Resolves to the terminal outcome, including `Cancelled` on interruption. */
  readonly outcome: Effect.Effect<AmbientTaskOutcome<O>>;
  /** Interrupts the task; its outcome resolves to `Cancelled`. */
  readonly cancel: Effect.Effect<void>;
}

/**
 * Fork one ambient task into a scoped fiber and return a handle. The caller
 * (boot, composer, apply, run, debug, commit, push, release) NEVER awaits the
 * outcome, so the ambient task cannot block it. Interrupting the fiber resolves
 * the outcome to `Cancelled` rather than a bare interruption.
 */
export const runAmbientTaskDetached = <I, O>(
  input: AmbientTaskRunInput<I, O>,
): Effect.Effect<AmbientTaskHandle<O>, never, AmbientTaskRunner | Scope.Scope> =>
  Effect.gen(function* () {
    const runner = yield* AmbientTaskRunner;
    const fiber = yield* runner.run(input).pipe(Effect.forkScoped);
    // `run` never fails, so the only non-success exit is interruption: await the
    // fiber's terminal exit and map an interrupt to a `Cancelled` outcome.
    const outcome = Fiber.await(fiber).pipe(
      Effect.map((exit) => (Exit.isSuccess(exit) ? exit.value : cancelled(input.signature.kind))),
    );
    return {
      outcome,
      cancel: Fiber.interrupt(fiber).pipe(Effect.asVoid),
    };
  });
