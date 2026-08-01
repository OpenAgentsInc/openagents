import { describe, expect, it } from "vite-plus/test";

import { forensicCanonicalJson, forensicSha256Digest, strictDecode } from "../src/canonical.ts";
import {
  ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF,
  CONTROL_FALSE_POSITIVE_METRIC_REF,
  FROZEN_FORENSIC_METRIC_REGISTRY,
  FORENSIC_EVALUATOR_ADJUDICATION_VERSION,
  FORENSIC_METRIC_DEFINITION_VERSION,
  FORENSIC_METRIC_REGISTRY_VERSION,
  FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION,
  QUALIFIED_HIT_METRIC_REF,
  TOKENS_TO_IDENTIFICATION_METRIC_REF,
  TOTAL_RUN_TOKENS_METRIC_REF,
  ForensicEvaluatorAdjudicationSchema,
  ForensicMetricRegistrySchema,
  ForensicPromptPromotionSchema,
  ForensicProviderUsageReceiptSchema,
  ForensicScorecardSchema,
  rebuildForensicScorecard,
  type ForensicMetricDefinition,
  type ForensicScorecardRunInput,
} from "../src/metrics.ts";
import { projectForensicScorecardPublicSafe } from "../src/projection.ts";
import { FORENSIC_RUN_EVENT_VERSION, ForensicRunEventSchema } from "../src/run.ts";

const digest = (character: string) => `sha256:${character.repeat(64)}`;
const evaluatorRevisionDigest = digest("9");

const definition = (
  metricRef: string,
  unit: "boolean" | "milliseconds" | "tokens",
): ForensicMetricDefinition => ({
  schema: FORENSIC_METRIC_DEFINITION_VERSION,
  metricRef,
  name: metricRef,
  formula: `Frozen formula for ${metricRef}.`,
  unit,
  eligiblePopulationRef: "population.forensic.frozen.v1",
  censorPolicy: "Right-censor eligible misses at their retained boundary.",
  missPolicy: "Retain misses, failures, retries, and spent budget.",
  exactnessPolicy: "Unavailable observations never carry numeric values.",
  dimensionRefs: ["dimension.split", "dimension.population"],
  aggregation: unit === "boolean" ? "mean" : "survival_estimator",
  displayMetadataRefs: ["display.metric.provenance"],
  revisionDigest: digest("a"),
  createdAt: "2026-08-01T12:00:00.000Z",
});

const definitions = [
  definition(QUALIFIED_HIT_METRIC_REF, "boolean"),
  definition(ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF, "milliseconds"),
  definition(TOKENS_TO_IDENTIFICATION_METRIC_REF, "tokens"),
  definition(TOTAL_RUN_TOKENS_METRIC_REF, "tokens"),
  definition(CONTROL_FALSE_POSITIVE_METRIC_REF, "boolean"),
];

const registry = strictDecode(ForensicMetricRegistrySchema, {
  schema: FORENSIC_METRIC_REGISTRY_VERSION,
  registryRef: "registry.forensic.metrics.test.v1",
  definitions,
  revisionDigest: forensicSha256Digest(definitions),
  frozenAt: "2026-08-01T12:00:01.000Z",
});

const metricContext = {
  benchmarkRevisionDigest: digest("1"),
  datasetSplit: "development" as const,
  armRef: "arm.coldcard.complete-vulnerable",
  repetition: 1,
  targetDigest: digest("2"),
  sourceBundleDigest: digest("3"),
  promptDigest: digest("4"),
  modelDigest: digest("5"),
  modelParametersDigest: digest("6"),
  workerImageDigest: digest("7"),
  workerProfileDigest: digest("8"),
  sandboxRef: "sandbox.forensic.test.1",
  resourceGeneration: 1,
  evaluatorRevisionDigest,
};

const event = (
  sequence: number,
  kind: "analysis_started" | "turn_started" | "finding_submitted" | "run_settled",
  observedAt: string,
) =>
  strictDecode(ForensicRunEventSchema, {
    schema: FORENSIC_RUN_EVENT_VERSION,
    eventRef: `event.run.test.${sequence}`,
    runRef: "run.forensic.test.1",
    sequence,
    kind,
    actorRef: "actor.forensic.driver",
    metricContext,
    relatedRefs: kind === "finding_submitted" ? ["finding.test.1"] : [],
    detailRefs: [],
    clock: "control_plane_server",
    observedAt,
  });

const analysisEvent = event(1, "analysis_started", "2026-08-01T12:00:00.000Z");
const turnEvent = event(2, "turn_started", "2026-08-01T12:00:01.000Z");
const findingEvent = event(3, "finding_submitted", "2026-08-01T12:02:00.000Z");
const settledEvent = event(4, "run_settled", "2026-08-01T12:03:00.000Z");

const usageReceipt = strictDecode(ForensicProviderUsageReceiptSchema, {
  schema: FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION,
  receiptRef: "receipt.provider.test.1",
  runRef: "run.forensic.test.1",
  turnRef: "turn.discovery.test.1",
  role: "discovery",
  attempt: 1,
  startEventRef: turnEvent.eventRef,
  startEventSequence: turnEvent.sequence,
  settledEventSequence: settledEvent.sequence,
  abandoned: false,
  losingParallelArm: false,
  usage: {
    inputTokens: 600,
    cachedInputTokens: 100,
    outputTokens: 200,
    reasoningTokens: 50,
    totalTokens: 900,
    exactness: "exact",
    providerUsageRef: "provider.usage.test.1",
  },
  costMicros: 120000,
  recordedAt: "2026-08-01T12:03:01.000Z",
});

const adjudication = strictDecode(ForensicEvaluatorAdjudicationSchema, {
  schema: FORENSIC_EVALUATOR_ADJUDICATION_VERSION,
  adjudicationRef: "adjudication.test.1",
  runRef: "run.forensic.test.1",
  findingRef: "finding.test.1",
  findingEventRef: findingEvent.eventRef,
  findingEventDigest: forensicSha256Digest(findingEvent),
  evaluatorRevisionDigest,
  outcome: "qualified",
  vulnerabilityIdentityDigest: digest("b"),
  requiredCausalLinks: 6,
  supportedCausalLinks: 6,
  submittedSourceRefs: 3,
  validSourceRefs: 3,
  reasonRefs: ["reason.frozen.oracle.match"],
  evaluatedAt: "2026-08-01T13:00:00.000Z",
});

const baseRun: ForensicScorecardRunInput = {
  runRef: "run.forensic.test.1",
  runDigest: digest("c"),
  armRef: metricContext.armRef,
  datasetSplit: "development",
  population: "vulnerable",
  coverageStatus: "complete",
  events: [analysisEvent, turnEvent, findingEvent, settledEvent],
  usageReceipts: [usageReceipt],
  adjudications: [adjudication],
  retainedReceiptDigests: [digest("d")],
  failureRefs: [],
};

const rebuild = (runs: ReadonlyArray<ForensicScorecardRunInput>) =>
  rebuildForensicScorecard({
    scorecardRef: "scorecard.forensic.test.v1",
    datasetRevisionDigest: digest("e"),
    evaluatorRevisionDigest,
    candidateDigest: digest("f"),
    registry,
    hardGates: [
      {
        gateRef: "gate.forensic.complete-input",
        passed: true,
        evidenceRefs: ["evidence.coverage.complete"],
        blockerRefs: [],
      },
    ],
    runs,
    generatedAt: "2026-08-01T14:00:00.000Z",
  });

describe("forensic metric evidence", () => {
  it("freezes the complete lifecycle, quality, efficiency, reliability, and Coldcard catalog", () => {
    const metricRefs = new Set(
      FROZEN_FORENSIC_METRIC_REGISTRY.definitions.map((item) => item.metricRef),
    );
    expect(FROZEN_FORENSIC_METRIC_REGISTRY.definitions.length).toBeGreaterThanOrEqual(45);
    expect(metricRefs.has("metric.coldcard.generator_vector_coverage.v1")).toBe(true);
    expect(metricRefs.has("metric.coldcard.entropy_sensitivity.v1")).toBe(true);
    expect(metricRefs.has("metric.coldcard.scan_throughput.v1")).toBe(true);
    expect(metricRefs.has("metric.coldcard.false_matches_per_million.v1")).toBe(true);
    expect(metricRefs.has("metric.coldcard.reconciliation_drift.v1")).toBe(true);
    expect(metricRefs.has("metric.correction_rejection_burden.v1")).toBe(true);
  });

  it("keeps T5 at the immutable finding timestamp and treats crossing-turn tokens as an upper bound", () => {
    const scorecard = rebuild([baseRun]);
    const run = scorecard.runs[0];
    expect(run?.qualifiedFindingObservedAt).toBe(findingEvent.observedAt);
    expect(adjudication.evaluatedAt).not.toBe(findingEvent.observedAt);
    expect(
      run?.values.find((value) => value.metricRef === ANALYSIS_TIME_TO_IDENTIFICATION_METRIC_REF)
        ?.numericValue,
    ).toBe(120000);
    expect(
      run?.values.find((value) => value.metricRef === TOKENS_TO_IDENTIFICATION_METRIC_REF),
    ).toMatchObject({ numericValue: 900, exactness: "upper_bound" });
  });

  it("rejects adjudication that tries to rewrite the finding event bytes", () => {
    const rewrittenFinding = { ...findingEvent, observedAt: "2026-08-01T12:00:02.000Z" };
    expect(() =>
      rebuild([
        {
          ...baseRun,
          events: [analysisEvent, turnEvent, rewrittenFinding, settledEvent],
        },
      ]),
    ).toThrow(/immutable finding event bytes/);
  });

  it("retains misses as right-censored observations with their spent token budget", () => {
    const scorecard = rebuild([
      {
        ...baseRun,
        adjudications: [],
        censorAtMilliseconds: 300000,
      },
    ]);
    const run = scorecard.runs[0];
    expect(run).toMatchObject({ outcome: "miss", miss: true, censored: true });
    expect(run?.censorAt?.milliseconds).toBe(300000);
    expect(run?.spentUsage).toMatchObject({ totalTokens: 900, exactness: "exact" });
    expect(scorecard).toMatchObject({ missCount: 1, censorCount: 1 });
  });

  it("never represents unavailable provider usage as numeric zero", () => {
    expect(() =>
      strictDecode(ForensicProviderUsageReceiptSchema, {
        ...usageReceipt,
        usage: {
          exactness: "unavailable",
          unavailableReasonRef: "unavailable.provider.did.not.report",
          inputTokens: 0,
          totalTokens: 0,
        },
      }),
    ).toThrow();

    const scorecard = rebuild([{ ...baseRun, usageReceipts: [] }]);
    expect(scorecard.cost).toMatchObject({ exactness: "unavailable" });
    expect(projectForensicScorecardPublicSafe(scorecard)).not.toHaveProperty("costMicros");
  });

  it("keeps control and incomplete populations out of identification-miss denominators", () => {
    const controlContext = {
      ...metricContext,
      datasetSplit: "clean_holdout" as const,
      armRef: "arm.clean.control",
    };
    const controlEvents = baseRun.events.map((item) => ({
      ...item,
      metricContext: controlContext,
    }));
    const scorecard = rebuild([
      {
        ...baseRun,
        armRef: controlContext.armRef,
        datasetSplit: "clean_holdout",
        population: "clean_control",
        events: controlEvents,
        adjudications: [],
      },
      {
        ...baseRun,
        runDigest: digest("0"),
        armRef: "arm.incomplete",
        population: "incomplete",
        coverageStatus: "incomplete",
        events: baseRun.events.map((item) => ({
          ...item,
          metricContext: { ...metricContext, armRef: "arm.incomplete" },
        })),
        adjudications: [],
      },
    ]);
    expect(scorecard.missCount).toBe(0);
    expect(scorecard.populationGroups).toHaveLength(2);
    expect(scorecard.runs.every((run) => !run.eligibleForIdentification)).toBe(true);
  });

  it("rejects hand-edited aggregate populations", () => {
    const scorecard = rebuild([baseRun]);
    expect(() =>
      strictDecode(ForensicScorecardSchema, {
        ...scorecard,
        populationGroups: [{ ...scorecard.populationGroups[0], missCount: 1 }],
      }),
    ).toThrow(/population/);
  });

  it("builds byte-identical scorecards from the same retained evidence", () => {
    expect(forensicCanonicalJson(rebuild([baseRun]))).toBe(
      forensicCanonicalJson(rebuild([baseRun])),
    );
  });
});

describe("forensic promotion and public metrics", () => {
  it("refuses admission when a faster candidate is quality-dominated", () => {
    const fixture = {
      schema: "openagents.forensic_prompt_promotion.v1",
      promotionRef: "promotion.test.1",
      candidatePromptDigest: digest("1"),
      candidateProducerRef: "actor.producer.1",
      evaluatorRef: "actor.evaluator.1",
      scorecardRef: "scorecard.candidate.1",
      baselineScorecardRef: "scorecard.baseline.1",
      releaseGateRef: "gate.release.1",
      hardGateRefs: ["gate.quality.1"],
      allHardGatesPassed: true,
      paretoStatus: "dominated",
      qualityRegressionMetricRefs: [QUALIFIED_HIT_METRIC_REF],
      efficiencyImprovementMetricRefs: [TOKENS_TO_IDENTIFICATION_METRIC_REF],
      holdoutDatasetDigest: digest("2"),
      cleanHoldoutDatasetDigest: digest("3"),
      holdoutEvidenceRefs: ["evidence.holdout.1"],
      operatorDecisionRef: "decision.operator.1",
      operatorRef: "actor.operator.1",
      decision: "admitted",
      priorActivePromptDigest: digest("4"),
      nextActivePromptDigest: digest("1"),
      rollbackPromptDigest: digest("4"),
      reasonRefs: ["reason.faster.but.less.accurate"],
      decidedAt: "2026-08-01T14:10:00.000Z",
    };
    expect(() => strictDecode(ForensicPromptPromotionSchema, fixture)).toThrow(/Pareto-safe/);
  });

  it("projects aggregate counts, durations, tokens, costs, and digests without private refs", () => {
    const scorecard = rebuild([baseRun]);
    const projection = projectForensicScorecardPublicSafe(scorecard);
    const encoded = forensicCanonicalJson(projection);
    expect(projection.populations[0]).toMatchObject({
      runCount: 1,
      hitCount: 1,
      identificationDurationTotalMilliseconds: 120000,
      identificationTokenTotal: 900,
    });
    expect(encoded).not.toContain(baseRun.runRef);
    expect(encoded).not.toContain(findingEvent.eventRef);
    expect(encoded).not.toContain(usageReceipt.receiptRef);
    expect(encoded).not.toContain("providerUsageRef");
  });
});
