import { Effect, Layer, Schema as S } from "effect";

import type { TurnRefusalReason } from "@openagentsinc/agent-runtime-schema";

import type { AppleFmCompletionTurn } from "./client.js";
import {
  AmbientInference,
  AmbientResourceGate,
  AmbientTaskRunner,
  AmbientTaskRunnerLayer,
  BootExplanationInput,
  bootExplanationSignature,
  CommitMessageDraftInput,
  commitMessageDraftSignature,
  ContextSummaryInput,
  contextSummarySignature,
  DebugStateExplanationInput,
  debugStateExplanationSignature,
  DiagnosticExplanationInput,
  diagnosticExplanationSignature,
  DiffSummaryInput,
  diffSummarySignature,
  TestFailureExplanationInput,
  testFailureExplanationSignature,
  type AmbientExplanationOutput,
  type AmbientInferenceShape,
  type AmbientResourceGateShape,
  type AmbientTaskKind,
  type AmbientTaskSignature,
  type CommitMessageDraftOutput,
} from "./ambient-task.js";

/**
 * `@openagentsinc/apple-fm-runtime` ambient-task quality corpus (AFS-07).
 *
 * Each ambient task has its OWN quality corpus and quality floor. A corpus case
 * pins a deterministic input, a raw on-device completion, and the expected
 * bounded outcome: a completed advisory result that passes the task's floor, or
 * a typed refusal for empty, oversized, action-claiming, or below-floor output.
 * `evaluateAmbientCorpus` runs a corpus through the real runner with a fixed
 * fake inference, so the exit check "each task passes its own quality corpus" is
 * a deterministic, offline test.
 */

/** A single quality-corpus case for a typed ambient task. */
export interface AmbientCorpusCase<I, O> {
  readonly name: string;
  readonly facts: I;
  readonly rawCompletion: string;
  readonly expect:
    | { readonly _tag: "Completed"; readonly check: (result: O) => boolean }
    | { readonly _tag: "Refused"; readonly reason: TurnRefusalReason };
}

/** A typed ambient task paired with its quality corpus. */
export interface AmbientCorpus<I, O> {
  readonly signature: AmbientTaskSignature<I, O>;
  readonly cases: ReadonlyArray<AmbientCorpusCase<I, O>>;
}

/** The bounded result of evaluating one corpus case against the runner. */
export interface AmbientCorpusCaseResult {
  readonly kind: AmbientTaskKind;
  readonly name: string;
  readonly passed: boolean;
  readonly detail: string;
}

const fixedInference = (text: string): AmbientInferenceShape => ({
  complete: () =>
    Effect.succeed({ outcome: "completed", text, usageTruth: "estimated" } satisfies AppleFmCompletionTurn),
});

const readyGate: AmbientResourceGateShape = {
  snapshot: Effect.succeed({ appleFmReady: true, thermalState: "nominal", underMemoryPressure: false }),
};

/**
 * Evaluate every case in a corpus. Each case builds a self-contained runner over
 * a fixed fake inference and a ready gate, so the evaluation is deterministic and
 * requires no ambient context. The result is `never`-failing.
 */
export const evaluateAmbientCorpus = <I, O>(
  corpus: AmbientCorpus<I, O>,
): Effect.Effect<ReadonlyArray<AmbientCorpusCaseResult>> =>
  Effect.forEach(corpus.cases, (kase) =>
    Effect.gen(function* () {
      const layer = AmbientTaskRunnerLayer.pipe(
        Layer.provide(Layer.succeed(AmbientInference, AmbientInference.of(fixedInference(kase.rawCompletion)))),
        Layer.provide(Layer.succeed(AmbientResourceGate, AmbientResourceGate.of(readyGate))),
      );
      const outcome = yield* AmbientTaskRunner.pipe(
        Effect.flatMap((runner) => runner.run({ signature: corpus.signature, facts: kase.facts })),
        Effect.provide(layer),
      );
      if (kase.expect._tag === "Refused") {
        const passed = outcome._tag === "Refused" && outcome.reason === kase.expect.reason;
        return {
          kind: corpus.signature.kind,
          name: kase.name,
          passed,
          detail: passed ? "refused as expected" : `expected refusal ${kase.expect.reason}, got ${outcome._tag}`,
        } satisfies AmbientCorpusCaseResult;
      }
      const passed = outcome._tag === "Completed" && kase.expect.check(outcome.result);
      return {
        kind: corpus.signature.kind,
        name: kase.name,
        passed,
        detail: passed ? "completed and passed the quality floor" : `expected a passing completion, got ${outcome._tag}`,
      } satisfies AmbientCorpusCaseResult;
    }),
  );

const ACTION_CLAIM = "I ran the tests and committed the fix for you.";
const EMPTY = "   ";

const decode = <I>(schema: S.Codec<I, unknown, never, never>, raw: unknown): I =>
  S.decodeUnknownSync(schema)(raw);

// --- Per-task corpora ---------------------------------------------------------

export const commitMessageDraftCorpus: AmbientCorpus<CommitMessageDraftInput, CommitMessageDraftOutput> = {
  signature: commitMessageDraftSignature,
  cases: [
    {
      name: "drafts an imperative subject with a body",
      facts: decode(CommitMessageDraftInput, {
        sourceControlRef: "sc.main.1",
        branch: "main",
        stagedFileCount: 3,
        diffSummary: "Refactor the parser and add tests for the new tokenizer path.",
      }),
      rawCompletion: "Refactor parser and cover tokenizer path\n\nSplit the lexer and add tokenizer tests.",
      expect: { _tag: "Completed", check: (result) => result.subject.length > 0 && result.subject.length <= 120 },
    },
    {
      name: "refuses an empty draft",
      facts: decode(CommitMessageDraftInput, {
        sourceControlRef: "sc.main.2",
        branch: "main",
        stagedFileCount: 1,
        diffSummary: "Tweak a comment.",
      }),
      rawCompletion: EMPTY,
      expect: { _tag: "Refused", reason: "empty_output" },
    },
    {
      name: "refuses a first-person action claim",
      facts: decode(CommitMessageDraftInput, {
        sourceControlRef: "sc.main.3",
        branch: "main",
        stagedFileCount: 2,
        diffSummary: "Add retries to the uploader.",
      }),
      rawCompletion: ACTION_CLAIM,
      expect: { _tag: "Refused", reason: "action_claim_rejected" },
    },
  ],
};

const explanationCompleted = (result: { readonly explanation: string }): boolean =>
  result.explanation.length >= 16 && result.explanation.length <= 1200;

export const testFailureExplanationCorpus: AmbientCorpus<TestFailureExplanationInput, AmbientExplanationOutput> = {
  signature: testFailureExplanationSignature,
  cases: [
    {
      name: "explains an assertion failure",
      facts: decode(TestFailureExplanationInput, {
        runRef: "run.1",
        testName: "parser.test.ts > tokenizes strings",
        failureOutput: "AssertionError: expected 3 tokens, received 2. The closing quote was not consumed.",
      }),
      rawCompletion:
        "The tokenizer stops one token short because the closing quote is not consumed; check the string-scanning loop's terminator handling.",
      expect: { _tag: "Completed", check: explanationCompleted },
    },
    {
      name: "refuses an action-claiming explanation",
      facts: decode(TestFailureExplanationInput, {
        runRef: "run.2",
        testName: "uploader.test.ts > retries",
        failureOutput: "TimeoutError: no retry occurred within 2000ms.",
      }),
      rawCompletion: ACTION_CLAIM,
      expect: { _tag: "Refused", reason: "action_claim_rejected" },
    },
    {
      name: "refuses a below-floor explanation",
      facts: decode(TestFailureExplanationInput, {
        runRef: "run.3",
        testName: "math.test.ts > adds",
        failureOutput: "expected 4, got 5",
      }),
      rawCompletion: "off by one",
      expect: { _tag: "Refused", reason: "malformed_output" },
    },
  ],
};

export const diagnosticExplanationCorpus: AmbientCorpus<DiagnosticExplanationInput, AmbientExplanationOutput> = {
  signature: diagnosticExplanationSignature,
  cases: [
    {
      name: "explains a type diagnostic",
      facts: decode(DiagnosticExplanationInput, {
        contextManifestRef: "ctx.1",
        diagnosticCode: "TS2345",
        diagnosticMessage: "Argument of type 'string' is not assignable to parameter of type 'number'.",
        sourceExcerpt: "add(total, label)",
      }),
      rawCompletion:
        "The call passes a string where a number is expected; convert the label to a number or fix the argument order in add().",
      expect: { _tag: "Completed", check: explanationCompleted },
    },
    {
      name: "refuses empty diagnostic output",
      facts: decode(DiagnosticExplanationInput, {
        contextManifestRef: "ctx.2",
        diagnosticCode: "TS7006",
        diagnosticMessage: "Parameter 'x' implicitly has an 'any' type.",
        sourceExcerpt: "const f = (x) => x",
      }),
      rawCompletion: EMPTY,
      expect: { _tag: "Refused", reason: "empty_output" },
    },
  ],
};

export const contextSummaryCorpus: AmbientCorpus<ContextSummaryInput, AmbientExplanationOutput> = {
  signature: contextSummarySignature,
  cases: [
    {
      name: "summarizes the assembled context",
      facts: decode(ContextSummaryInput, {
        contextManifestRef: "ctx.3",
        itemCount: 4,
        factText: "Active file src/parser.ts, selection lines 20-40, one diagnostic, and the current diff.",
      }),
      rawCompletion:
        "The context covers the parser file around the tokenizer, a selection, one diagnostic, and the working diff.",
      expect: { _tag: "Completed", check: explanationCompleted },
    },
    {
      name: "refuses an action-claiming summary",
      facts: decode(ContextSummaryInput, {
        contextManifestRef: "ctx.4",
        itemCount: 2,
        factText: "Active file src/app.ts and the current diff.",
      }),
      rawCompletion: ACTION_CLAIM,
      expect: { _tag: "Refused", reason: "action_claim_rejected" },
    },
  ],
};

export const diffSummaryCorpus: AmbientCorpus<DiffSummaryInput, AmbientExplanationOutput> = {
  signature: diffSummarySignature,
  cases: [
    {
      name: "summarizes a diff",
      facts: decode(DiffSummaryInput, {
        sourceControlRef: "sc.feature.1",
        changedFileCount: 2,
        diffText: "Modified src/parser.ts to add tokenizer tests; updated README with the new flag.",
      }),
      rawCompletion: "Two files change: the parser gains tokenizer tests, and the README documents the new flag.",
      expect: { _tag: "Completed", check: explanationCompleted },
    },
    {
      name: "refuses empty diff summary",
      facts: decode(DiffSummaryInput, {
        sourceControlRef: "sc.feature.2",
        changedFileCount: 1,
        diffText: "Small formatting change.",
      }),
      rawCompletion: EMPTY,
      expect: { _tag: "Refused", reason: "empty_output" },
    },
  ],
};

export const debugStateExplanationCorpus: AmbientCorpus<DebugStateExplanationInput, AmbientExplanationOutput> = {
  signature: debugStateExplanationSignature,
  cases: [
    {
      name: "explains a paused breakpoint",
      facts: decode(DebugStateExplanationInput, {
        debugRef: "dbg.1",
        stoppedReason: "breakpoint",
        stateText: "Paused at parser.ts:42, frame tokenize(), local index=2, tokens.length=1.",
      }),
      rawCompletion:
        "Execution is paused at the tokenizer at line 42 with one token collected; inspect the loop index before the terminator check.",
      expect: { _tag: "Completed", check: explanationCompleted },
    },
    {
      name: "refuses an action-claiming debug explanation",
      facts: decode(DebugStateExplanationInput, {
        debugRef: "dbg.2",
        stoppedReason: "exception",
        stateText: "Uncaught TypeError at app.ts:10.",
      }),
      rawCompletion: ACTION_CLAIM,
      expect: { _tag: "Refused", reason: "action_claim_rejected" },
    },
  ],
};

export const bootExplanationCorpus: AmbientCorpus<BootExplanationInput, AmbientExplanationOutput> = {
  signature: bootExplanationSignature,
  cases: [
    {
      name: "explains the boot rows",
      facts: decode(BootExplanationInput, {
        bootRef: "boot.1",
        bootSequenceText: "Runtime gateway ready. Providers: codex ready, claude ready, apple_fm ready.",
      }),
      rawCompletion: "The runtime gateway started and all three providers, including on-device Apple FM, are ready.",
      expect: { _tag: "Completed", check: explanationCompleted },
    },
    {
      name: "refuses empty boot explanation",
      facts: decode(BootExplanationInput, {
        bootRef: "boot.2",
        bootSequenceText: "Runtime gateway ready.",
      }),
      rawCompletion: EMPTY,
      expect: { _tag: "Refused", reason: "empty_output" },
    },
  ],
};

/**
 * Every ambient task corpus, specialized to a homogeneous evaluation Effect so
 * the aggregate is non-generic. Each entry proves its task passes its own
 * quality corpus.
 */
export const AMBIENT_CORPUS_EVALUATIONS: ReadonlyArray<
  Effect.Effect<ReadonlyArray<AmbientCorpusCaseResult>>
> = [
  evaluateAmbientCorpus(commitMessageDraftCorpus),
  evaluateAmbientCorpus(testFailureExplanationCorpus),
  evaluateAmbientCorpus(diagnosticExplanationCorpus),
  evaluateAmbientCorpus(contextSummaryCorpus),
  evaluateAmbientCorpus(diffSummaryCorpus),
  evaluateAmbientCorpus(debugStateExplanationCorpus),
  evaluateAmbientCorpus(bootExplanationCorpus),
];
