import { readFileSync } from "node:fs";
import path from "node:path";

import { Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  GraphMemoryEvaluationFixtureFileSchema,
  GraphMemoryEvaluationManifestSchema,
  type GraphMemoryEvaluationArmRow,
  type GraphMemoryEvaluationFixtureFile,
  type GraphMemoryEvaluationFixtureRow,
  type GraphMemoryEvaluationPins,
} from "./desktop-graph-memory-evaluation-contract.js";
import {
  evaluateDesktopGraphMemoryComparison,
  graphMemoryEvaluationDigest,
  graphMemoryEvaluationPrivateDetailDigest,
  summarizeGraphMemoryEvaluationArm,
  validateGraphMemoryEvaluationDataset,
} from "./desktop-graph-memory-evaluation.js";
import { desktopGraphMemoryRecallQueryFor } from "./desktop-graph-memory-workflow.js";

const fixtureRoot = path.resolve(
  import.meta.dirname,
  "../tests/fixtures/graph-memory-evaluation/v1",
);
const readJson = (name: string): unknown =>
  JSON.parse(readFileSync(path.join(fixtureRoot, name), "utf8"));
const development = Schema.decodeUnknownSync(GraphMemoryEvaluationFixtureFileSchema)(
  readJson("development.json"),
);
const holdout = Schema.decodeUnknownSync(GraphMemoryEvaluationFixtureFileSchema)(
  readJson("holdout.json"),
);
const manifest = Schema.decodeUnknownSync(GraphMemoryEvaluationManifestSchema)(
  readJson("manifest.json"),
);

const requiredAt = <T>(values: ReadonlyArray<T>, index: number): T => {
  const value = values[index];
  if (value === undefined) throw new Error(`Missing fixture index ${index}.`);
  return value;
};

const sdkPackages = (): GraphMemoryEvaluationPins["sdkPackages"] => [
  {
    package: "@openagentsinc/ai",
    version: "0.2.1-rc.2",
    integrity:
      "sha512-mQN7iOA0EbXL8zwFxJr2hHy23dMivLAhGeJRd8TepjzcBRECKAVQ1fTFOsvr8Ik2g+Bvg2Wxw3mtoX+N7SCC7Q==",
  },
  {
    package: "@openagentsinc/rlm",
    version: "0.2.1-rc.2",
    integrity:
      "sha512-M3JX7BJDvTBbjSltmLD7u5PRl7K1Ytzjvy5HdD/e25Ywixa1fJk0tX3i2DFUBLyJjnNPv4YO4QwosPnu7ILMMQ==",
  },
  {
    package: "@openagentsinc/history-corpus",
    version: "0.2.1-rc.2",
    integrity:
      "sha512-2V9CW86+DMlxmcu9GjUUJj4bnTvj8OP99j64tOM4BJzj76Ci1Z+jT4b291UJyp8l9sYcQpNVcC/ID1pUVCiYcw==",
  },
  {
    package: "@openagentsinc/agent-harness-contract",
    version: "0.2.1-rc.2",
    integrity:
      "sha512-3vIhogeE/Bbg5LfWKjQ8msCs7T7BB/ivE0Zd1VUVAA5Pz6WCsier6VLFapAhW6NBRnS0brvDhaerPiDBfkRqNQ==",
  },
  {
    package: "@openagentsinc/agent-runtime-schema",
    version: "0.2.1-rc.2",
    integrity:
      "sha512-xSC/laUZcFaF8AUOEshQy01Q1QuOdrYAEx48PPUjRjPO/dP4IUuTQLkeio0EfyYKqu+VMxax9ttwleeEPnQKJA==",
  },
  {
    package: "@openagentsinc/dse",
    version: "0.2.1-rc.2",
    integrity:
      "sha512-CFsEPBkJbQi/ZL/OQKOGqxahI4GNy9Es2b8p/uXD4xQaIFKm48DS7EtiEbJbLm3l5qXHNpR7D7HEFf6R4ljdHg==",
  },
  {
    package: "@openagentsinc/graph-corpus",
    version: "0.2.1-rc.2",
    integrity:
      "sha512-8LFMS/WGvRKYmcst0GpqpLrjRMpWRoQ9aSshJqKPHBTxWdbQI3360y/VBa5YroBz7fEndPnmgJStdyEhDM2C3Q==",
  },
  {
    package: "@openagentsinc/conformance-kit",
    version: "0.2.1-rc.2",
    integrity:
      "sha512-Hm2JMz1FKTuUNIxr9rEXKZ5RQuVLPZF8/UI2WdTTutt/dkbECjb8g1lpGK6tMJitw/NV0MbEbHK3z8PIqWXHKQ==",
  },
];

const pins = (): GraphMemoryEvaluationPins => ({
  sdkTrain: "0.2.1-rc.2",
  sdkPackages: sdkPackages(),
  lockDigest: "9".repeat(64),
  openAgentsCommit: "99c84710b5c13833c91c62a8941ebe7aa9359d4f",
  sourceState: "clean",
  desktopBuildRef: "apps/openagents-desktop/dist",
  desktopBuildDigest: "a".repeat(64),
  desktopBuildArtifacts: [{ ref: "dist/main.js", digest: "1".repeat(64) }],
  productWiringSmokeDigest: "2".repeat(64),
  runnerRef: "apps/openagents-desktop/scripts/graph-memory-evaluation.ts",
  runnerDigest: "7".repeat(64),
  oracleRef: "apps/openagents-desktop/src/desktop-graph-memory-evaluation.ts",
  oracleDigest: "8".repeat(64),
  runtime: { node: "v24.13.1", platform: "darwin", architecture: "arm64" },
  timingRef: "node:perf_hooks.performance.now",
  model: {
    _tag: "NotUsed",
    reason: "deterministic_evaluation",
    requiredModelCalls: 0,
    requiredInputTokens: 0,
    requiredOutputTokens: 0,
  },
  parserRef: "parser.graph-memory.fixture",
  parserVersion: "1",
  parserArtifactDigest: "c".repeat(64),
  datasetRevisionDigest: manifest.datasetRevisionDigest,
  developmentSplitDigest: manifest.developmentDigest,
  holdoutSplitDigest: manifest.holdoutDigest,
  corpusDigest: "d".repeat(64),
  policyDigest: "e".repeat(64),
  budgetDigest: "f".repeat(64),
  qualityPolicyDigest: manifest.qualityPolicyDigest,
});

const resultRow = (
  arm: GraphMemoryEvaluationArmRow["arm"],
  fixture: GraphMemoryEvaluationFixtureRow,
  overrides: Partial<GraphMemoryEvaluationArmRow> = {},
): GraphMemoryEvaluationArmRow => ({
  rowId: fixture.rowId,
  arm,
  outcome: "complete",
  inputDigest: graphMemoryEvaluationDigest({
    rowId: fixture.rowId,
    query: desktopGraphMemoryRecallQueryFor(fixture.query),
    sources: fixture.sources.filter((source) => !source.revoked),
    scenario: fixture.scenario,
  }),
  corpusDigest: graphMemoryEvaluationDigest(fixture.sources.filter((source) => !source.revoked)),
  queryDigest: graphMemoryEvaluationDigest(desktopGraphMemoryRecallQueryFor(fixture.query)),
  policyDigest: pins().policyDigest,
  budgetDigest: pins().budgetDigest,
  modelCalls: 0,
  extractionEvidence:
    arm === "graph_assisted"
      ? {
          status: "complete",
          receiptDigest: graphMemoryEvaluationDigest({
            rowId: fixture.rowId,
            extraction: "complete",
          }),
          usageTruth: "exact",
          inputCorpusDigest: graphMemoryEvaluationDigest(fixture.sources),
          budgetDigest: graphMemoryEvaluationDigest({ fixture: fixture.rowId, budget: "graph" }),
          graphStateDigest: graphMemoryEvaluationDigest({ fixture: fixture.rowId, graph: "state" }),
          entityCount: fixture.goldEntityRefs.length,
          mergeCount: 0,
        }
      : {
          status: "not_run",
          receiptDigest: null,
          usageTruth: "not_run",
          inputCorpusDigest: null,
          budgetDigest: null,
          graphStateDigest: null,
          entityCount: 0,
          mergeCount: 0,
        },
  emittedAnswerFactRefs: fixture.expectedAnswerFactRefs,
  emittedCitationRefs: [fixture.expectedFactSupport[0]!.supportingSourceRefs[0]!],
  validCitationRefs: [fixture.expectedFactSupport[0]!.supportingSourceRefs[0]!],
  citationEvidence: {
    validationDigest: graphMemoryEvaluationDigest({
      emitted: [fixture.expectedFactSupport[0]!.supportingSourceRefs[0]!],
      valid: [fixture.expectedFactSupport[0]!.supportingSourceRefs[0]!],
    }),
    invalidCount: 0,
  },
  mergedEntityPairs: [],
  observedEntityRefs: fixture.goldEntityRefs,
  retrievedSourceRefs: fixture.expectedFactSupport.flatMap((fact) => fact.supportingSourceRefs),
  retrievedElementAliases: fixture.relevantElementAliases,
  retrievalEvidence: {
    mappings: fixture.relevantElementAliases.map((oracleElementAlias) => ({
      observedElementDigest: graphMemoryEvaluationDigest({
        rowId: fixture.rowId,
        oracleElementAlias,
      }),
      oracleElementAlias,
    })),
    mappingDigest: graphMemoryEvaluationDigest(
      fixture.relevantElementAliases.map((oracleElementAlias) => ({
        observedElementDigest: graphMemoryEvaluationDigest({
          rowId: fixture.rowId,
          oracleElementAlias,
        }),
        oracleElementAlias,
      })),
    ),
  },
  recallLatencySamplesMs: [9, 10, 11],
  setupLatencyMs: arm === "graph_assisted" ? 5 : null,
  tokens: { _tag: "Exact", inputTokens: 0, outputTokens: 0, totalTokens: 0 },
  truncated: false,
  hitCaps: arm === "graph_assisted" ? fixture.scenario.expectedCaps : [],
  ...overrides,
});

const rows = (
  arm: GraphMemoryEvaluationArmRow["arm"],
  transform: (
    row: GraphMemoryEvaluationArmRow,
    fixture: GraphMemoryEvaluationFixtureRow,
    index: number,
  ) => GraphMemoryEvaluationArmRow = (row) => row,
): ReadonlyArray<GraphMemoryEvaluationArmRow> =>
  holdout.rows.map((fixture, index) => transform(resultRow(arm, fixture), fixture, index));

const input = (
  historyOnlyRows: ReadonlyArray<GraphMemoryEvaluationArmRow> = rows("history_only"),
  graphAssistedRows: ReadonlyArray<GraphMemoryEvaluationArmRow> = rows("graph_assisted"),
  selectedPins: GraphMemoryEvaluationPins = pins(),
) => ({
  evaluatedAt: "2026-07-22T12:00:00.000Z",
  expectedPins: selectedPins,
  observedPins: selectedPins,
  development,
  holdout,
  manifest,
  historyOnlyRows,
  graphAssistedRows,
});

describe("graph memory evaluation fixtures", () => {
  test("pins physically isolated development and holdout fixture digests", () => {
    const validated = validateGraphMemoryEvaluationDataset(development, holdout, manifest);
    expect(validated).toMatchObject({
      ok: true,
      developmentDigest: manifest.developmentDigest,
      holdoutDigest: manifest.holdoutDigest,
      datasetRevisionDigest: manifest.datasetRevisionDigest,
    });
    expect(graphMemoryEvaluationDigest(development)).toBe(manifest.developmentDigest);
    expect(graphMemoryEvaluationDigest(holdout)).toBe(manifest.holdoutDigest);
    const holdoutIds = new Set(holdout.rows.map((row) => row.rowId));
    expect(development.rows.filter((row) => holdoutIds.has(row.rowId))).toEqual([]);
  });

  test("covers every required challenge in the holdout", () => {
    const covered = new Set(holdout.rows.flatMap((row) => row.challengeClasses));
    expect(manifest.requiredChallengeClasses.every((value) => covered.has(value))).toBe(true);
    expect(holdout.rows.some((row) => row.sources.some((source) => source.revoked))).toBe(true);
  });

  test("refuses split identity overlap and changed bytes", () => {
    const overlap: GraphMemoryEvaluationFixtureFile = {
      ...holdout,
      rows: [
        { ...requiredAt(holdout.rows, 0), rowId: requiredAt(development.rows, 0).rowId },
        ...holdout.rows.slice(1),
      ],
    };
    expect(validateGraphMemoryEvaluationDataset(development, overlap, manifest)).toMatchObject({
      ok: false,
      reason: "split_identity_overlap",
    });
    const changed: GraphMemoryEvaluationFixtureFile = {
      ...holdout,
      rows: [
        { ...requiredAt(holdout.rows, 0), query: "Changed after review." },
        ...holdout.rows.slice(1),
      ],
    };
    expect(validateGraphMemoryEvaluationDataset(development, changed, manifest)).toMatchObject({
      ok: false,
      reason: "split_digest_mismatch",
    });
  });
});

describe("graph memory evaluation scoring", () => {
  test("reports an evidence-backed graph improvement without changing review or release state", () => {
    const history = rows("history_only", (row, fixture, index) => ({
      ...row,
      emittedAnswerFactRefs: index % 2 === 0 ? [] : fixture.expectedAnswerFactRefs,
      retrievedSourceRefs: fixture.sources
        .filter((source) => !source.revoked)
        .map((source) => source.sourceRef),
      recallLatencySamplesMs: [5, 6, 7],
    }));
    const evaluated = evaluateDesktopGraphMemoryComparison(input(history));
    expect(evaluated.ok).toBe(true);
    if (!evaluated.ok) return;
    expect(evaluated.receipt.comparison.quality).toBe("improved");
    expect(evaluated.receipt.historyOnly.answerSupport.value).toBeLessThan(1);
    expect(evaluated.receipt.graphAssisted.answerSupport.value).toBe(1);
    expect(evaluated.receipt.historyOnly.retrievalPrecision.value).toBeLessThan(1);
    expect(evaluated.receipt.graphAssisted.retrievalPrecision.value).toBe(1);
    expect(evaluated.receipt.privateDetailReceiptDigest).toBe(
      graphMemoryEvaluationPrivateDetailDigest(history, rows("graph_assisted")),
    );
    expect(evaluated.receipt.disposition).toEqual({
      implementation: "implemented",
      evidence: "present",
      quality: "improved",
      ownerReview: "unreviewed",
      release: "not_released",
      publicClaim: "not_authorized",
    });
  });

  test("zero citations are unsupported and a missing answer has zero support", () => {
    const emptyEvidence = rows("history_only", (row) => ({
      ...row,
      emittedAnswerFactRefs: [],
      emittedCitationRefs: [],
      validCitationRefs: [],
      citationEvidence: { validationDigest: null, invalidCount: 0 },
    }));
    const summary = summarizeGraphMemoryEvaluationArm("history_only", holdout.rows, emptyEvidence);
    expect(summary.citationValidity).toEqual({
      status: "unsupported",
      numerator: 0,
      denominator: 0,
      value: null,
      reason: "no_citations_emitted",
    });
    expect(summary.answerSupport).toMatchObject({ status: "supported", numerator: 0, value: 0 });
    const evaluated = evaluateDesktopGraphMemoryComparison(input(emptyEvidence));
    expect(evaluated.ok).toBe(true);
    if (evaluated.ok) {
      expect(evaluated.receipt.comparison.quality).toBe("inconclusive");
      expect(evaluated.receipt.disposition.evidence).toBe("partial");
    }
  });

  test("does not score an unavailable graph state as an empty graph", () => {
    const graph = rows("graph_assisted", (row, fixture) =>
      fixture.challengeClasses.includes("same_name_entities")
        ? {
            ...row,
            outcome: "failed",
            extractionEvidence: {
              ...row.extractionEvidence,
              status: "failed",
              graphStateDigest: null,
              entityCount: 0,
              mergeCount: 0,
            },
            observedEntityRefs: [],
          }
        : row,
    );
    const summary = summarizeGraphMemoryEvaluationArm("graph_assisted", holdout.rows, graph);
    expect(summary.falseMergeRate).toEqual({
      status: "unsupported",
      numerator: 0,
      denominator: 0,
      value: null,
      reason: "graph_state_unavailable",
    });
    expect(summary.missedEntityRate).toMatchObject({
      status: "unsupported",
      denominator: 0,
      value: null,
      reason: "graph_state_unavailable",
    });
    const evaluated = evaluateDesktopGraphMemoryComparison(input(rows("history_only"), graph));
    expect(evaluated.ok).toBe(true);
    if (evaluated.ok) {
      expect(evaluated.receipt.comparison).toEqual({
        quality: "inconclusive",
        reasons: ["required_metric_unsupported", "non_complete_rows_present"],
      });
    }
  });

  test("keeps every terminal row outcome separate and makes partial evidence inconclusive", () => {
    const outcomes = ["complete", "partial", "refused", "failed", "inconclusive"] as const;
    const history = rows("history_only", (row, _fixture, index) => ({
      ...row,
      outcome: requiredAt(outcomes, index % outcomes.length),
    }));
    const evaluated = evaluateDesktopGraphMemoryComparison(input(history));
    expect(evaluated.ok).toBe(true);
    if (!evaluated.ok) return;
    expect(evaluated.receipt.historyOnly.outcomes).toEqual({
      complete: 2,
      partial: 2,
      refused: 1,
      failed: 1,
      inconclusive: 1,
    });
    expect(evaluated.receipt.comparison.quality).toBe("inconclusive");
    expect(evaluated.receipt.disposition.evidence).toBe("partial");
  });

  test("keeps unavailable tokens distinct from exact zero and refuses a quality verdict", () => {
    const history = rows("history_only", (row, _fixture, index) =>
      index === 0 ? { ...row, tokens: { _tag: "Unavailable", reason: "not_reported" } } : row,
    );
    const availablePins: GraphMemoryEvaluationPins = {
      ...pins(),
      model: {
        _tag: "Available",
        provider: "fixture-provider",
        model: "fixture-model-v1",
        modelArtifactDigest: "b".repeat(64),
      },
    };
    const evaluated = evaluateDesktopGraphMemoryComparison(
      input(history, rows("graph_assisted"), availablePins),
    );
    expect(evaluated.ok).toBe(true);
    if (!evaluated.ok) return;
    expect(evaluated.receipt.historyOnly.usage).toMatchObject({
      truth: "unavailable",
      exactRows: 6,
      unavailableRows: 1,
      inputTokens: null,
      outputTokens: null,
      totalTokens: null,
    });
    expect(evaluated.receipt.comparison).toMatchObject({
      quality: "inconclusive",
      reasons: expect.arrayContaining(["token_usage_unavailable"]),
    });
  });

  test("records false merges, missed entities, retrieval metrics, latency, and truncation", () => {
    const graph = rows("graph_assisted", (row, fixture, index) => ({
      ...row,
      extractionEvidence: {
        ...row.extractionEvidence,
        mergeCount: index === 0 ? 1 : 0,
      },
      observedEntityRefs: fixture.goldEntityRefs.slice(0, -1),
      retrievedSourceRefs:
        index === 5 ? row.retrievedSourceRefs.slice(0, 1) : row.retrievedSourceRefs,
      recallLatencySamplesMs: [index + 1, index + 2, index + 3],
      truncated: index === 5,
      hitCaps: index === 5 ? [...row.hitCaps, "max_returned_elements"] : row.hitCaps,
    }));
    const summary = summarizeGraphMemoryEvaluationArm("graph_assisted", holdout.rows, graph);
    expect(summary.falseMergeRate).toMatchObject({
      status: "supported",
      numerator: 1,
      denominator: 3,
    });
    expect(summary.missedEntityRate).toMatchObject({
      status: "supported",
      numerator: 7,
      denominator: 14,
      value: 0.5,
    });
    expect(summary.retrievalRecall.value).toBe(1);
    expect(summary.latency).toEqual({ samples: 21, p50Ms: 5, p95Ms: 8 });
    expect(summary.truncation).toEqual({
      rows: 1,
      hitCaps: [
        "max_returned_elements",
        "partial_extraction_reported",
        "prompt_injection_treated_as_data",
        "revoked_source_excluded",
        "stale_graph_failed_closed",
      ],
    });
    const evaluated = evaluateDesktopGraphMemoryComparison(input(rows("history_only"), graph));
    expect(evaluated.ok).toBe(true);
    if (evaluated.ok) expect(evaluated.receipt.comparison.quality).toBe("regressed");
  });

  test("refuses changed pins, split pins, duplicate or missing arm rows", () => {
    const changedPins = input();
    changedPins.observedPins = { ...changedPins.observedPins, policyDigest: "0".repeat(64) };
    expect(evaluateDesktopGraphMemoryComparison(changedPins)).toMatchObject({
      ok: false,
      reason: "pin_mismatch",
    });

    const changedSplit = input();
    const changed = { ...changedSplit.expectedPins, holdoutSplitDigest: "1".repeat(64) };
    changedSplit.expectedPins = changed;
    changedSplit.observedPins = changed;
    expect(evaluateDesktopGraphMemoryComparison(changedSplit)).toMatchObject({
      ok: false,
      reason: "split_digest_mismatch",
    });

    const missing = input(rows("history_only").slice(1));
    expect(evaluateDesktopGraphMemoryComparison(missing)).toMatchObject({
      ok: false,
      reason: "row_result_mismatch",
    });
    const duplicatedRows = rows("history_only");
    const duplicate = input([...duplicatedRows, requiredAt(duplicatedRows, 0)]);
    expect(evaluateDesktopGraphMemoryComparison(duplicate)).toMatchObject({
      ok: false,
      reason: "row_result_mismatch",
    });
  });

  test("an unavailable model pin cannot produce a quality claim", () => {
    const unavailable = input();
    const unavailablePins: GraphMemoryEvaluationPins = {
      ...unavailable.expectedPins,
      model: { _tag: "Unavailable", reason: "not_observed" },
    };
    unavailable.expectedPins = unavailablePins;
    unavailable.observedPins = unavailablePins;
    const evaluated = evaluateDesktopGraphMemoryComparison(unavailable);
    expect(evaluated.ok).toBe(true);
    if (evaluated.ok) {
      expect(evaluated.receipt.comparison).toMatchObject({
        quality: "inconclusive",
        reasons: expect.arrayContaining(["model_pin_unavailable"]),
      });
      expect(evaluated.receipt.disposition.publicClaim).toBe("not_authorized");
    }
  });
});
