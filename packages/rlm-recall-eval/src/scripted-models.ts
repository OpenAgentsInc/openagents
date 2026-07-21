/**
 * Deterministic scripted models for the hermetic dense-recall matrix.
 *
 * The eval holds MODEL CAPABILITY constant (a fixed, competent-but-literal
 * reader) so the comparison isolates the RETRIEVAL STRATEGY of each tier, not
 * model IQ. Every scripted call is pure and deterministic; there is no network
 * and no spend, which keeps CI hermetic.
 *
 * Token accounting is REAL, not invented: each call measures the exact prompt
 * and response strings the engine passes and records modeled tokens
 * (`chars / 4`) into a caller-owned {@link UsageSink}. A dedicated
 * "unknown usage" model omits token counts to exercise the honesty path where
 * usage stays unknown and cost is excluded.
 */

import { Effect } from "effect";
import type { RlmModelPlan } from "@openagentsinc/rlm";
import type { DensityFamily, PlantedQuestion } from "./transcripts.ts";

/** Modeled token count for a text (paper-agnostic chars/4 heuristic). */
export const modeledTokens = (text: string): number => Math.ceil(text.length / 4);

/** The reader's honest "I could not find it" token. */
export const UNKNOWN_ANSWER = "UNKNOWN" as const;

/**
 * Caller-owned usage accumulator. `reportedCalls < calls` means at least one
 * model call returned no usage, so the aggregate must treat the run's token
 * totals as incomplete.
 */
export interface UsageSink {
  calls: number;
  reportedCalls: number;
  inputTokens: number;
  outputTokens: number;
}

export const newUsageSink = (): UsageSink => ({
  calls: 0,
  reportedCalls: 0,
  inputTokens: 0,
  outputTokens: 0,
});

export const sinkCompleteness = (sink: UsageSink): "complete" | "partial" | "unavailable" => {
  if (sink.reportedCalls === 0) return "unavailable";
  if (sink.reportedCalls < sink.calls) return "partial";
  return "complete";
};

// ---------------------------------------------------------------------------
// Answer extraction — the fixed reader's deterministic capability.
// ---------------------------------------------------------------------------

const VALUE_RE = /^[A-Za-z0-9._-]+/;

/** Extract the value for `<locator>=` from a text blob, or null. */
export const extractSingle = (text: string, locator: string): string | null => {
  const needle = `${locator}=`;
  const idx = text.indexOf(needle);
  if (idx < 0) return null;
  const m = VALUE_RE.exec(text.slice(idx + needle.length));
  return m ? m[0] : null;
};

/** Extract every value for `<tag>=` occurrences, in order. */
export const extractAll = (text: string, tag: string): ReadonlyArray<string> => {
  const needle = `${tag}=`;
  const out: Array<string> = [];
  let from = 0;
  for (;;) {
    const idx = text.indexOf(needle, from);
    if (idx < 0) break;
    const m = VALUE_RE.exec(text.slice(idx + needle.length));
    if (m) out.push(m[0]);
    from = idx + needle.length;
  }
  return out;
};

/** The reader answer for a question over an arbitrary evidence blob. */
export const readerAnswer = (
  family: DensityFamily,
  grepPattern: string,
  evidence: string,
): string => {
  if (family === "pair") {
    const values = extractAll(evidence, grepPattern);
    const unique = [...new Set(values)];
    return unique.length >= 2
      ? unique.join("|")
      : unique.length === 1
        ? unique[0]!
        : UNKNOWN_ANSWER;
  }
  const value = extractSingle(evidence, grepPattern);
  return value ?? UNKNOWN_ANSWER;
};

// ---------------------------------------------------------------------------
// Scripted RlmModelPlan factory for the RLM engine (Tier S variants).
// ---------------------------------------------------------------------------

export interface SemanticProgramShape {
  /** Program JSON the root emits (already tailored to the question). */
  readonly programJson: string;
}

export interface ScriptedModelOptions {
  readonly question: PlantedQuestion;
  readonly sink: UsageSink;
  /** When false, calls omit token usage (exercises the honesty path). */
  readonly reportUsage?: boolean;
  readonly strategyRef: string;
}

const record = (sink: UsageSink, prompt: string, response: string, reportUsage: boolean): void => {
  sink.calls += 1;
  if (reportUsage) {
    sink.reportedCalls += 1;
    sink.inputTokens += modeledTokens(prompt);
    sink.outputTokens += modeledTokens(response);
  }
};

/**
 * Build a scripted `RlmModelPlan` whose root emits `programJson` and whose leaf
 * extracts the planted answer from the item text it is shown.
 */
export const makeScriptedModel = (
  program: SemanticProgramShape,
  options: ScriptedModelOptions,
): RlmModelPlan => {
  const reportUsage = options.reportUsage ?? true;
  const { question, sink } = options;
  return {
    strategyRef: options.strategyRef,
    completeRoot: (prompt) =>
      Effect.sync(() => {
        const text = program.programJson;
        record(sink, prompt, text, reportUsage);
        return reportUsage
          ? { text, inputTokens: modeledTokens(prompt), outputTokens: modeledTokens(text) }
          : { text };
      }),
    completeLeaf: (prompt) =>
      Effect.sync(() => {
        const text = readerAnswer(question.family, question.grepPattern, prompt);
        record(sink, prompt, text, reportUsage);
        return reportUsage
          ? { text, inputTokens: modeledTokens(prompt), outputTokens: modeledTokens(text) }
          : { text };
      }),
  };
};
