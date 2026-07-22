/**
 * Hermetic dense-recall harness.
 *
 * Runs the full tier matrix over the synthetic OpenAgents transcript families
 * at representative history sizes, aggregates the results, and evaluates the
 * two product-admission gates. Everything is deterministic and scripted, so a
 * clean checkout reproduces byte-identical aggregates with no network or spend.
 *
 * A separate honesty probe runs a synthesising tier with usage reporting turned
 * OFF, proving that unknown usage is excluded from cost aggregates rather than
 * counted as zero.
 */

import { Effect } from "effect";

import {
  RLM_EVAL_DEFAULT_MODEL_ID,
  RLM_EVAL_PRICE_CATALOG_SOURCE_REF,
  RLM_EVAL_PRICE_CATALOG_VERSION,
} from "./price-catalog.ts";
import { EVAL_STRATEGY_REF, runAllTiersForQuestion, runSemanticModelMap } from "./tiers.ts";
import { evaluateDepthGate, evaluateEscalationGate, type GateResult } from "./gates.ts";
import { aggregateTier, type TierAggregate, type TierId, type TierRunResult } from "./scoring.ts";
import { generateTranscript, type DensityFamily } from "./transcripts.ts";

/** Pinned SDK version the eval consumes (matches package.json dependency). */
export const RLM_SDK_VERSION = "0.2.1-rc.2" as const;

export interface HarnessConfig {
  readonly historySizes: ReadonlyArray<number>;
  readonly families: ReadonlyArray<DensityFamily>;
  readonly modelId: string;
}

/**
 * Default hermetic configuration. Sizes stay under the SDK inline corpus byte
 * ceiling (4 MiB) so real runs execute; the million-token and 10M-token scales
 * are documented as modeled limitations in the evidence report.
 */
export const DEFAULT_HERMETIC_CONFIG: HarnessConfig = {
  historySizes: [8, 64, 256, 1024, 4096],
  families: ["constant", "linear", "pair"],
  modelId: RLM_EVAL_DEFAULT_MODEL_ID,
};

const ALL_TIERS: ReadonlyArray<TierId> = [
  "direct",
  "tier_d",
  "semantic_depth0",
  "semantic_modelmap",
  "semantic_depth1",
  "semantic_depth2",
  "bounded_window",
  "provider_compaction",
];

export interface HonestyProbe {
  readonly note: string;
  readonly runs: number;
  readonly unknownUsageCount: number;
  readonly costExcludedCorrectly: boolean;
}

export interface HarnessOutput {
  readonly meta: {
    readonly kind: "hermetic";
    readonly sdkVersion: string;
    readonly strategyRef: string;
    readonly priceCatalogVersion: string;
    readonly priceCatalogSourceRef: string;
    readonly config: HarnessConfig;
    readonly totalRuns: number;
  };
  readonly tierAggregates: ReadonlyArray<TierAggregate>;
  readonly gates: {
    readonly escalation: GateResult;
    readonly depth: GateResult;
  };
  readonly honesty: HonestyProbe;
}

export const runHermeticMatrix = (
  config: HarnessConfig = DEFAULT_HERMETIC_CONFIG,
): Effect.Effect<HarnessOutput, never> =>
  Effect.gen(function* () {
    const all: Array<TierRunResult> = [];
    for (const family of config.families) {
      for (const historySize of config.historySizes) {
        const transcript = generateTranscript(family, historySize);
        for (const question of transcript.questions) {
          const results = yield* runAllTiersForQuestion({
            transcript,
            question,
            modelId: config.modelId,
          });
          all.push(...results);
        }
      }
    }

    const tierAggregates = ALL_TIERS.map((tierId) =>
      aggregateTier(
        tierId,
        all.filter((r) => r.tierId === tierId),
      ),
    );

    const honesty = yield* runHonestyProbe(config.modelId);

    return {
      meta: {
        kind: "hermetic",
        sdkVersion: RLM_SDK_VERSION,
        strategyRef: EVAL_STRATEGY_REF,
        priceCatalogVersion: RLM_EVAL_PRICE_CATALOG_VERSION,
        priceCatalogSourceRef: RLM_EVAL_PRICE_CATALOG_SOURCE_REF,
        config,
        totalRuns: all.length,
      },
      tierAggregates,
      gates: {
        escalation: evaluateEscalationGate(tierAggregates),
        depth: evaluateDepthGate(tierAggregates),
      },
      honesty,
    } satisfies HarnessOutput;
  });

const runHonestyProbe = (modelId: string): Effect.Effect<HonestyProbe, never> =>
  Effect.gen(function* () {
    const sizes = [8, 64, 256] as const;
    const results: Array<TierRunResult> = [];
    for (const size of sizes) {
      const transcript = generateTranscript("linear", size);
      for (const question of transcript.questions) {
        results.push(
          yield* runSemanticModelMap({ transcript, question, modelId, reportUsage: false }),
        );
      }
    }
    const unknownUsageCount = results.filter((r) => r.cost.disposition === "unknown_usage").length;
    const costExcludedCorrectly = results.every(
      (r) => r.cost.usd === null && r.tokenCompleteness === "unavailable",
    );
    return {
      note: "A synthesising semantic tier was run with model usage reporting OFF. Every run keeps usage unknown and is excluded from cost, never priced as zero.",
      runs: results.length,
      unknownUsageCount,
      costExcludedCorrectly,
    };
  });
