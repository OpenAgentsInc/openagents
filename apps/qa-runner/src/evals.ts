// Chill-evals (#6183): run ONE scenario across N variants, then COMPARE.
//
// "I want to see how agents perform with these MCP changes." A chill-eval holds
// a scenario (or set) FIXED and varies the AGENT CONFIG — a variant is a
// model/brain/config/MCP-set/tool-policy, or a before/after of a change. It runs
// each variant over the same scenario(s) and produces a side-by-side comparison:
// per-variant pass-rate, latency p50/p90, and behavior deltas, each with its own
// run artifact (video/result) ref.
//
// DESIGN BOUNDARIES (honored deliberately):
//   - Variant = which Brain/Backend config to use (the existing brain/backend
//     abstraction). We do NOT touch khala-config.ts / khala-driver.ts /
//     openrouter* — those are owned by another lane. A variant supplies its own
//     Brain factory, so a "model A vs model B" or "MCP-on vs MCP-off" comparison
//     is expressed as two variant entries, not a config-file edit.
//   - DETERMINISTIC CI: with a fixtures/scriptedBrain backend and an injected
//     clock, the whole eval is pure-enough to compare in CI with NO network and
//     NO spend. The live khalaBrain seam stays inert/owner-gated.
//   - PUBLIC-SAFE: the persisted eval result is tripwire-checked
//     (`assertPublicSafeResult`) before write — no prompts/tokens/secrets.
//   - HONEST: a metric we did not measure is the `not_measured` sentinel, never a
//     fabricated 0. Pass-rate is computed from real run outcomes only.
//
// The AGGREGATE MATH (latency percentiles, mean) is reused from the benchmark
// report (`inference/benchmark/report.ts` — the book's P50/P90 framing) so the
// eval and the inference benchmark agree on how a distribution is summarized.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Effect } from "effect";
import {
  mean,
  percentile,
} from "../../openagents.com/workers/api/src/inference/benchmark/report";
import type { Backend } from "./backend";
import type { Brain } from "./brain";
import { assertPublicSafeResult } from "./result";
import { runQaSession } from "./runner";
import type { Target } from "./target";

// ---------------------------------------------------------------------------
// not_measured sentinel (mirrors the benchmark telemetry honesty contract)
// ---------------------------------------------------------------------------

// A measured number OR the honest "we did not / could not measure this" marker.
// A latency percentile over zero samples is `not_measured`, never 0.
export type MeasuredNumber = number | "not_measured";

export const isMeasured = (value: MeasuredNumber): value is number =>
  value !== "not_measured";

const measuredOrNot = (value: number | null): MeasuredNumber =>
  value === null ? "not_measured" : value;

// ---------------------------------------------------------------------------
// Variant + eval input
// ---------------------------------------------------------------------------

// A VARIANT is one agent configuration under test. The comparison holds the
// scenario(s) fixed and varies this. `id` is stable + URL-safe (it appears in
// the persisted result and the /pro page). `label` is the human title. `note`
// records WHAT differs (e.g. "MCP filesystem server enabled", "model B",
// "before #6183"). The variant carries factories so each repetition gets a
// fresh Brain/Backend (a Brain is stateful — it must not be shared across runs).
export interface EvalVariant {
  readonly id: string;
  readonly label: string;
  /** Public-safe note describing what this variant changes. */
  readonly note?: string;
  /** Fresh Brain per run (the decision-maker / model / tool-policy under test). */
  readonly brain: () => Brain;
  /** Fresh Backend per run (isolation; usually shared-fixture in CI). */
  readonly backend: () => Backend;
}

export interface EvalScenario {
  /** Stable, URL-safe id, e.g. "login-regression". */
  readonly id: string;
  readonly label: string;
}

export interface EvalInput {
  /** Stable, URL-safe eval id — becomes /pro/evals/<id>. */
  readonly id: string;
  readonly title: string;
  readonly target: Target;
  /** The scenario held FIXED across variants (its id/label only; the steps
   *  live in each variant's brain factory so a "before/after" can change them). */
  readonly scenario: EvalScenario;
  /** The variants compared over the scenario (>= 2 for a real comparison). */
  readonly variants: ReadonlyArray<EvalVariant>;
  /** How many times to run each variant (default 1). >1 gives a latency
   *  distribution per variant; pass-rate is passes/total. */
  readonly repetitions?: number;
  /** Root directory; each (variant, rep) gets a subdir for its artifacts. */
  readonly artifactDir: string;
  /** Safety cap on steps per run. */
  readonly maxSteps?: number;
  /** Injectable clock (deterministic timestamps in tests). */
  readonly now?: () => Date;
}

// ---------------------------------------------------------------------------
// Per-variant metrics + the eval result schema (public-safe)
// ---------------------------------------------------------------------------

export const EVAL_SCHEMA_VERSION = "openagents.qa_runner.eval.v1" as const;

// One run's public-safe footprint inside a variant: status + duration + the
// artifact dir (relative) so the /pro page can dereference the video/result.
export interface EvalRunRef {
  readonly status: "pass" | "fail";
  readonly durationMs: number;
  /** Relative path (from the eval root) to this run's artifact dir. */
  readonly artifactDir: string;
  /** Relative path to the playable video, if one was captured. */
  readonly video?: string;
  readonly videoFormat?: "mp4" | "webm";
  /** Honest one-line failure summary when status is "fail". */
  readonly failure?: string;
}

export interface EvalVariantMetrics {
  readonly variantId: string;
  readonly label: string;
  readonly note?: string;
  readonly runs: ReadonlyArray<EvalRunRef>;
  /** passes / total, in [0,1]. */
  readonly passRate: number;
  readonly passCount: number;
  readonly runCount: number;
  /** Latency distribution over ALL runs of this variant (book P50/P90). */
  readonly latencyP50Ms: MeasuredNumber;
  readonly latencyP90Ms: MeasuredNumber;
  readonly latencyMeanMs: MeasuredNumber;
}

// A behavior delta of one variant relative to the comparison BASELINE (the
// first variant). Positive `passRateDelta` means this variant passed more
// often than the baseline; `latencyP50DeltaMs` is this minus baseline (negative
// = faster). Deltas are `not_measured` when either side lacks a measured value.
export interface EvalVariantDelta {
  readonly variantId: string;
  readonly passRateDelta: number;
  readonly latencyP50DeltaMs: MeasuredNumber;
  readonly latencyP90DeltaMs: MeasuredNumber;
}

export interface EvalResult {
  readonly schemaVersion: typeof EVAL_SCHEMA_VERSION;
  readonly id: string;
  readonly title: string;
  readonly target: { readonly name: string; readonly baseUrl: string };
  readonly scenario: EvalScenario;
  readonly startedAt: string;
  readonly endedAt: string;
  readonly repetitions: number;
  /** The baseline variant id (deltas are relative to this). */
  readonly baselineVariantId: string;
  readonly variants: ReadonlyArray<EvalVariantMetrics>;
  readonly deltas: ReadonlyArray<EvalVariantDelta>;
  /** HONEST: true only when every run used a real, spend-capable seam. CI fakes
   *  produce illustrative numbers (false) — they prove the harness, not lanes. */
  readonly decisionGrade: boolean;
}

// ---------------------------------------------------------------------------
// Pure comparison math (no IO) — testable on its own.
// ---------------------------------------------------------------------------

export const variantMetrics = (
  variant: EvalVariant,
  runs: ReadonlyArray<EvalRunRef>,
): EvalVariantMetrics => {
  const durations = runs.map((r) => r.durationMs);
  const passCount = runs.filter((r) => r.status === "pass").length;
  const runCount = runs.length;
  return {
    variantId: variant.id,
    label: variant.label,
    ...(variant.note !== undefined ? { note: variant.note } : {}),
    runs,
    passRate: runCount === 0 ? 0 : passCount / runCount,
    passCount,
    runCount,
    latencyP50Ms: measuredOrNot(percentile(durations, 50)),
    latencyP90Ms: measuredOrNot(percentile(durations, 90)),
    latencyMeanMs: measuredOrNot(mean(durations)),
  };
};

const subtractMeasured = (
  a: MeasuredNumber,
  b: MeasuredNumber,
): MeasuredNumber =>
  isMeasured(a) && isMeasured(b) ? a - b : "not_measured";

export const variantDeltas = (
  variants: ReadonlyArray<EvalVariantMetrics>,
): ReadonlyArray<EvalVariantDelta> => {
  const baseline = variants[0];
  if (baseline === undefined) return [];
  return variants.map((v) => ({
    variantId: v.variantId,
    passRateDelta: v.passRate - baseline.passRate,
    latencyP50DeltaMs: subtractMeasured(v.latencyP50Ms, baseline.latencyP50Ms),
    latencyP90DeltaMs: subtractMeasured(v.latencyP90Ms, baseline.latencyP90Ms),
  }));
};

// ---------------------------------------------------------------------------
// The eval driver: run each (variant, repetition) and assemble the comparison.
// ---------------------------------------------------------------------------

export interface EvalOutcome {
  readonly result: EvalResult;
  readonly resultPath: string;
}

export function runEval(input: EvalInput): Effect.Effect<EvalOutcome, Error> {
  const now = input.now ?? (() => new Date());
  const repetitions = Math.max(1, input.repetitions ?? 1);
  return Effect.gen(function* () {
    if (input.variants.length < 2) {
      return yield* Effect.fail(
        new Error(
          `a chill-eval compares variants: got ${input.variants.length}, need >= 2`,
        ),
      );
    }
    mkdirSync(input.artifactDir, { recursive: true });
    const startedAt = now();

    const variantMetricsList: EvalVariantMetrics[] = [];
    let anySpendCapable = false;

    for (const variant of input.variants) {
      const runs: EvalRunRef[] = [];
      for (let rep = 0; rep < repetitions; rep++) {
        const runDirName = `${variant.id}.${rep}`;
        const runDir = join(input.artifactDir, runDirName);
        const backend = variant.backend();
        // The local/fixture backend is not spend-capable; only an owner-armed
        // cloud-vm + live khala seam would flip this. We infer it honestly from
        // the backend name (the fixture/local path is never spend-capable).
        if (backend.name !== "local" && backend.name !== "cloud-vm") {
          anySpendCapable = true;
        }
        const outcome = yield* runQaSession({
          target: input.target,
          brain: variant.brain(),
          backend,
          artifactDir: runDir,
          ...(input.maxSteps !== undefined ? { maxSteps: input.maxSteps } : {}),
          now,
        });
        const r = outcome.result;
        runs.push({
          status: r.status,
          durationMs: r.durationMs,
          artifactDir: runDirName,
          ...(r.artifacts.video !== undefined
            ? { video: join(runDirName, r.artifacts.video) }
            : {}),
          ...(r.artifacts.videoFormat !== undefined
            ? { videoFormat: r.artifacts.videoFormat }
            : {}),
          ...(r.failure !== undefined ? { failure: r.failure } : {}),
        });
      }
      variantMetricsList.push(variantMetrics(variant, runs));
    }

    const endedAt = now();
    const baselineVariantId = input.variants[0]!.id;
    const result: EvalResult = {
      schemaVersion: EVAL_SCHEMA_VERSION,
      id: input.id,
      title: input.title,
      target: { name: input.target.name, baseUrl: input.target.baseUrl },
      scenario: input.scenario,
      startedAt: startedAt.toISOString(),
      endedAt: endedAt.toISOString(),
      repetitions,
      baselineVariantId,
      variants: variantMetricsList,
      deltas: variantDeltas(variantMetricsList),
      // Decision-grade only when a real spend-capable seam produced the numbers.
      // The default CI/fixture path is ILLUSTRATIVE (harness proof), never
      // decision-grade — no fake green.
      decisionGrade: anySpendCapable,
    };

    // Tripwire: never persist an eval result that leaks a forbidden field.
    assertPublicSafeResult(result);

    const resultPath = join(input.artifactDir, "eval.json");
    writeFileSync(resultPath, `${JSON.stringify(result, null, 2)}\n`);
    return { result, resultPath };
  });
}
