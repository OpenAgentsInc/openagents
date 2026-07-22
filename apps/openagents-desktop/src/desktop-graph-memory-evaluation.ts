import { canonicalJson, sha256Hex } from "@openagentsinc/graph-corpus";
import { Schema } from "effect";

import {
  DESKTOP_GRAPH_MEMORY_EVALUATION_SCHEMA_VERSION,
  DesktopGraphMemoryEvaluationReceiptSchema,
  GraphMemoryEvaluationInputSchema,
  type GraphMemoryEvaluationArmRow,
  type GraphMemoryEvaluationArmSummary,
  type GraphMemoryEvaluationFixtureFile,
  type GraphMemoryEvaluationFixtureRow,
  type GraphMemoryEvaluationFractionMetric,
  type GraphMemoryEvaluationInput,
  type GraphMemoryEvaluationManifest,
  type GraphMemoryEvaluationOutcome,
  type GraphMemoryEvaluationPins,
  type GraphMemoryEvaluationQualityResult,
  type GraphMemoryEvaluationRefusal,
  type GraphMemoryEvaluationResult,
} from "./desktop-graph-memory-evaluation-contract.js";
import { desktopGraphMemoryRecallQueryFor } from "./desktop-graph-memory-workflow.js";

export const graphMemoryEvaluationDigest = (value: unknown): string =>
  sha256Hex(canonicalJson(value));

export const graphMemoryEvaluationPrivateDetailDigest = (
  historyOnlyRows: ReadonlyArray<GraphMemoryEvaluationArmRow>,
  graphAssistedRows: ReadonlyArray<GraphMemoryEvaluationArmRow>,
): string => graphMemoryEvaluationDigest({ historyOnlyRows, graphAssistedRows });

const decodeGraphMemoryEvaluationInput = Schema.decodeUnknownExit(GraphMemoryEvaluationInputSchema);
const decodeDesktopGraphMemoryEvaluationReceipt = Schema.decodeUnknownSync(
  DesktopGraphMemoryEvaluationReceiptSchema,
);

const refusal = (
  reason: GraphMemoryEvaluationRefusal["reason"],
  detailSafe: string,
): GraphMemoryEvaluationRefusal => ({ ok: false, reason, detailSafe });

const setOf = (values: ReadonlyArray<string>): ReadonlySet<string> => new Set(values);
const intersectionCount = (left: ReadonlySet<string>, right: ReadonlySet<string>): number => {
  let count = 0;
  for (const value of left) if (right.has(value)) count += 1;
  return count;
};
const unique = <T extends string>(values: ReadonlyArray<T>): ReadonlyArray<T> =>
  [...new Set(values)].toSorted();
const pairKey = (pair: readonly [string, string]): string => [...pair].toSorted().join("\u0000");

const fraction = (
  numerator: number,
  denominator: number,
  unsupportedReason: string,
): GraphMemoryEvaluationFractionMetric =>
  denominator === 0
    ? {
        status: "unsupported",
        numerator,
        denominator,
        value: null,
        reason: unsupportedReason,
      }
    : {
        status: "supported",
        numerator,
        denominator,
        value: numerator / denominator,
        reason: null,
      };

const percentile = (samples: ReadonlyArray<number>, proportion: number): number | null => {
  if (samples.length === 0) return null;
  const sorted = [...samples].toSorted((left, right) => left - right);
  return sorted[Math.floor((sorted.length - 1) * proportion)] ?? null;
};

const allSourceRefs = (fixture: GraphMemoryEvaluationFixtureFile): ReadonlySet<string> =>
  new Set(fixture.rows.flatMap((row) => row.sources.map((source) => source.sourceRef)));

const fixtureSemanticsAreValid = (row: GraphMemoryEvaluationFixtureRow): boolean => {
  const sources = setOf(row.sources.map((source) => source.sourceRef));
  const facts = setOf(row.expectedAnswerFactRefs);
  const entities = setOf(row.goldEntityRefs);
  if (
    row.expectedFactSupport.length !== facts.size ||
    row.expectedFactSupport.some(
      (support) =>
        !facts.has(support.factRef) ||
        support.supportingSourceRefs.some((sourceRef) => !sources.has(sourceRef)),
    ) ||
    row.entityAliases.length !== entities.size ||
    row.entityAliases.some((entity) => !entities.has(entity.entityRef)) ||
    row.scenario.steps.some((step) => step.sourceRef !== null && !sources.has(step.sourceRef))
  )
    return false;
  const challenges = setOf(row.challengeClasses);
  if (
    (challenges.has("revoked_source") &&
      !row.scenario.steps.some((step) => step.operation === "revoke")) ||
    (challenges.has("partial_extraction") &&
      !row.scenario.steps.some((step) => step.operation === "extract_partial")) ||
    (challenges.has("stale_graph") &&
      !row.scenario.steps.some((step) => step.operation === "advance_graph"))
  )
    return false;
  return true;
};

export type GraphMemoryEvaluationDatasetValidation =
  | Readonly<{
      ok: true;
      developmentDigest: string;
      holdoutDigest: string;
      datasetRevisionDigest: string;
      challengeClasses: ReadonlyArray<GraphMemoryEvaluationFixtureRow["challengeClasses"][number]>;
    }>
  | GraphMemoryEvaluationRefusal;

/** Validate the physical split and every digest before an evaluation row is scored. */
export const validateGraphMemoryEvaluationDataset = (
  development: GraphMemoryEvaluationFixtureFile,
  holdout: GraphMemoryEvaluationFixtureFile,
  manifest: GraphMemoryEvaluationManifest,
): GraphMemoryEvaluationDatasetValidation => {
  if (development.split !== "development" || holdout.split !== "holdout") {
    return refusal("split_mismatch", "The fixture files do not have their required split labels.");
  }
  const developmentIds = setOf(development.rows.map((row) => row.rowId));
  const holdoutIds = setOf(holdout.rows.map((row) => row.rowId));
  const sourceOverlap = intersectionCount(allSourceRefs(development), allSourceRefs(holdout));
  if (intersectionCount(developmentIds, holdoutIds) > 0 || sourceOverlap > 0) {
    return refusal(
      "split_identity_overlap",
      "Development and holdout fixture identities must be disjoint.",
    );
  }
  if ([...development.rows, ...holdout.rows].some((row) => !fixtureSemanticsAreValid(row))) {
    return refusal(
      "fixture_semantics_invalid",
      "A fixture fact, entity alias, or scenario step is not source-addressed.",
    );
  }
  const developmentDigest = graphMemoryEvaluationDigest(development);
  const holdoutDigest = graphMemoryEvaluationDigest(holdout);
  const datasetRevisionDigest = graphMemoryEvaluationDigest({
    developmentDigest,
    holdoutDigest,
  });
  if (
    developmentDigest !== manifest.developmentDigest ||
    holdoutDigest !== manifest.holdoutDigest ||
    datasetRevisionDigest !== manifest.datasetRevisionDigest
  ) {
    return refusal(
      "split_digest_mismatch",
      "The fixture bytes do not match the manifest split digests.",
    );
  }
  if (graphMemoryEvaluationDigest(manifest.qualityPolicy) !== manifest.qualityPolicyDigest) {
    return refusal(
      "split_digest_mismatch",
      "The quality policy does not match its predeclared digest.",
    );
  }
  const challengeClasses = unique(holdout.rows.flatMap((row) => row.challengeClasses));
  if (manifest.requiredChallengeClasses.some((value) => !challengeClasses.includes(value))) {
    return refusal(
      "challenge_coverage_missing",
      "The holdout does not contain each required challenge class.",
    );
  }
  return {
    ok: true,
    developmentDigest,
    holdoutDigest,
    datasetRevisionDigest,
    challengeClasses,
  };
};

const outcomeCounts = (rows: ReadonlyArray<GraphMemoryEvaluationArmRow>) => {
  const counts: Record<GraphMemoryEvaluationOutcome, number> = {
    complete: 0,
    partial: 0,
    refused: 0,
    failed: 0,
    inconclusive: 0,
  };
  for (const row of rows) counts[row.outcome] += 1;
  return counts;
};

const summarizeUsage = (rows: ReadonlyArray<GraphMemoryEvaluationArmRow>) => {
  const exact = rows.filter((row) => row.tokens._tag === "Exact");
  const unavailableRows = rows.length - exact.length;
  if (unavailableRows > 0) {
    return {
      truth: "unavailable" as const,
      exactRows: exact.length,
      unavailableRows,
      inputTokens: null,
      outputTokens: null,
      totalTokens: null,
    };
  }
  return {
    truth: "exact" as const,
    exactRows: exact.length,
    unavailableRows: 0,
    inputTokens: exact.reduce(
      (total, row) => total + (row.tokens._tag === "Exact" ? row.tokens.inputTokens : 0),
      0,
    ),
    outputTokens: exact.reduce(
      (total, row) => total + (row.tokens._tag === "Exact" ? row.tokens.outputTokens : 0),
      0,
    ),
    totalTokens: exact.reduce(
      (total, row) => total + (row.tokens._tag === "Exact" ? row.tokens.totalTokens : 0),
      0,
    ),
  };
};

/** Score one arm against the reviewed holdout without consulting the other arm. */
export const summarizeGraphMemoryEvaluationArm = (
  arm: GraphMemoryEvaluationArmRow["arm"],
  fixtures: ReadonlyArray<GraphMemoryEvaluationFixtureRow>,
  rows: ReadonlyArray<GraphMemoryEvaluationArmRow>,
): GraphMemoryEvaluationArmSummary => {
  const fixtureById = new Map(fixtures.map((fixture) => [fixture.rowId, fixture]));
  let emittedCitations = 0;
  let validCitations = 0;
  let emittedAnswerFacts = 0;
  let supportedAnswerFacts = 0;
  let falseMerges = 0;
  let distinctPairOpportunities = 0;
  let falseMergeEvidenceUnavailable = false;
  let missedEntities = 0;
  let goldEntities = 0;
  let entityEvidenceUnavailable = false;
  let retrieved = 0;
  let relevantRetrieved = 0;
  let relevantExpected = 0;

  for (const row of rows) {
    const fixture = fixtureById.get(row.rowId);
    if (fixture === undefined) continue;
    const emittedCitationRefs = setOf(row.emittedCitationRefs);
    const validCitationRefs = setOf(row.validCitationRefs);
    emittedCitations += emittedCitationRefs.size;
    validCitations += intersectionCount(emittedCitationRefs, validCitationRefs);

    const emittedFacts = setOf(row.emittedAnswerFactRefs);
    const expectedFacts = setOf(fixture.expectedAnswerFactRefs);
    emittedAnswerFacts += expectedFacts.size;
    supportedAnswerFacts += intersectionCount(emittedFacts, expectedFacts);

    const falseMergePairs = setOf(fixture.goldDistinctEntityPairs.map(pairKey));
    const observedEntities = setOf(row.observedEntityRefs);
    const expectedEntities = setOf(fixture.goldEntityRefs);
    if (row.extractionEvidence.graphStateDigest === null) {
      falseMergeEvidenceUnavailable ||= falseMergePairs.size > 0;
      entityEvidenceUnavailable ||= expectedEntities.size > 0;
    } else {
      distinctPairOpportunities += falseMergePairs.size;
      falseMerges += Math.min(row.extractionEvidence.mergeCount, falseMergePairs.size);
      goldEntities += expectedEntities.size;
      missedEntities += expectedEntities.size - intersectionCount(expectedEntities, observedEntities);
    }

    const retrievedRefs = setOf(row.retrievedSourceRefs);
    const relevantRefs = setOf(
      fixture.expectedFactSupport.flatMap((fact) => fact.supportingSourceRefs),
    );
    retrieved += retrievedRefs.size;
    relevantRetrieved += intersectionCount(retrievedRefs, relevantRefs);
    relevantExpected += relevantRefs.size;
  }

  return {
    arm,
    rows: rows.length,
    outcomes: outcomeCounts(rows),
    citationValidity: fraction(validCitations, emittedCitations, "no_citations_emitted"),
    answerSupport: fraction(supportedAnswerFacts, emittedAnswerFacts, "no_expected_answer_facts"),
    falseMergeRate: falseMergeEvidenceUnavailable
      ? fraction(falseMerges, 0, "graph_state_unavailable")
      : fraction(falseMerges, distinctPairOpportunities, "no_distinct_entity_pairs"),
    missedEntityRate: entityEvidenceUnavailable
      ? fraction(missedEntities, 0, "graph_state_unavailable")
      : fraction(missedEntities, goldEntities, "no_gold_entities"),
    retrievalPrecision: fraction(relevantRetrieved, retrieved, "no_elements_retrieved"),
    retrievalRecall: fraction(relevantRetrieved, relevantExpected, "no_relevant_elements_defined"),
    latency: {
      samples: rows.flatMap((row) => row.recallLatencySamplesMs).length,
      p50Ms: percentile(
        rows.flatMap((row) => row.recallLatencySamplesMs),
        0.5,
      ),
      p95Ms: percentile(
        rows.flatMap((row) => row.recallLatencySamplesMs),
        0.95,
      ),
    },
    usage: summarizeUsage(rows),
    truncation: {
      rows: rows.filter((row) => row.truncated).length,
      hitCaps: unique(rows.flatMap((row) => row.hitCaps)),
    },
  };
};

const comparableMetrics = (
  summary: GraphMemoryEvaluationArmSummary,
): ReadonlyArray<GraphMemoryEvaluationFractionMetric> => [
  summary.citationValidity,
  summary.answerSupport,
  summary.retrievalPrecision,
  summary.retrievalRecall,
];

const supportedValue = (metric: GraphMemoryEvaluationFractionMetric): number | null =>
  metric.status === "supported" ? metric.value : null;

const qualityComparison = (
  pins: GraphMemoryEvaluationPins,
  policy: GraphMemoryEvaluationManifest["qualityPolicy"],
  history: GraphMemoryEvaluationArmSummary,
  graph: GraphMemoryEvaluationArmSummary,
): Readonly<{ quality: GraphMemoryEvaluationQualityResult; reasons: ReadonlyArray<string> }> => {
  const reasons: string[] = [];
  if (pins.model._tag === "Unavailable") reasons.push("model_pin_unavailable");
  if (history.usage.truth === "unavailable" || graph.usage.truth === "unavailable") {
    reasons.push("token_usage_unavailable");
  }
  if (
    [
      ...comparableMetrics(history),
      ...comparableMetrics(graph),
      graph.falseMergeRate,
      graph.missedEntityRate,
    ].some((metric) => metric.status === "unsupported")
  ) {
    reasons.push("required_metric_unsupported");
  }
  if (history.outcomes.complete !== history.rows || graph.outcomes.complete !== graph.rows) {
    reasons.push("non_complete_rows_present");
  }
  if (reasons.length > 0) return { quality: "inconclusive", reasons };

  const historyCitation = supportedValue(history.citationValidity) ?? 0;
  const graphCitation = supportedValue(graph.citationValidity) ?? 0;
  const historySupport = supportedValue(history.answerSupport) ?? 0;
  const graphSupport = supportedValue(graph.answerSupport) ?? 0;
  const historyPrecision = supportedValue(history.retrievalPrecision) ?? 0;
  const graphPrecision = supportedValue(graph.retrievalPrecision) ?? 0;
  const historyRecall = supportedValue(history.retrievalRecall) ?? 0;
  const graphRecall = supportedValue(graph.retrievalRecall) ?? 0;
  const graphFalseMerge = supportedValue(graph.falseMergeRate) ?? 1;
  const answerDelta = graphSupport - historySupport;
  const precisionDelta = graphPrecision - historyPrecision;
  const recallDelta = graphRecall - historyRecall;
  const citationDelta = graphCitation - historyCitation;
  const tolerance = policy.tieTolerance;

  if (
    citationDelta < -(policy.maximumCitationValidityRegression + tolerance) ||
    answerDelta < -(policy.maximumAnswerSupportRegression + tolerance) ||
    precisionDelta < -(policy.maximumRetrievalPrecisionRegression + tolerance) ||
    recallDelta < -(policy.maximumRetrievalRecallRegression + tolerance) ||
    graphFalseMerge > policy.maximumFalseMergeRate + tolerance
  ) {
    return { quality: "regressed", reasons: ["graph_arm_has_a_quality_regression"] };
  }
  if (
    answerDelta + tolerance >= policy.minimumAnswerSupportImprovement ||
    recallDelta + tolerance >= policy.minimumRetrievalRecallImprovement
  ) {
    return { quality: "improved", reasons: ["graph_arm_improves_supported_quality_metrics"] };
  }
  return { quality: "neutral", reasons: ["supported_quality_metrics_are_equal"] };
};

const requiredSdkPackages = [
  "@openagentsinc/ai",
  "@openagentsinc/rlm",
  "@openagentsinc/history-corpus",
  "@openagentsinc/agent-harness-contract",
  "@openagentsinc/agent-runtime-schema",
  "@openagentsinc/dse",
  "@openagentsinc/graph-corpus",
  "@openagentsinc/conformance-kit",
] as const;

const pinsAreComplete = (pins: GraphMemoryEvaluationPins): boolean => {
  const packages = setOf(pins.sdkPackages.map((item) => item.package));
  return (
    packages.size === requiredSdkPackages.length &&
    requiredSdkPackages.every((name) => packages.has(name))
  );
};

const rowIdentity = (fixture: GraphMemoryEvaluationFixtureRow) => ({
  // Both recall arms consume the same post-policy corpus. Revoked rows remain
  // in the reviewed fixture and scenario ledger, but they cannot enter either
  // arm's recall input.
  inputDigest: graphMemoryEvaluationDigest({
    rowId: fixture.rowId,
    query: desktopGraphMemoryRecallQueryFor(fixture.query),
    sources: fixture.sources.filter((source) => !source.revoked),
    scenario: fixture.scenario,
  }),
  corpusDigest: graphMemoryEvaluationDigest(fixture.sources.filter((source) => !source.revoked)),
  queryDigest: graphMemoryEvaluationDigest(desktopGraphMemoryRecallQueryFor(fixture.query)),
});

const rowEvidenceIsValid = (
  fixture: GraphMemoryEvaluationFixtureRow,
  history: GraphMemoryEvaluationArmRow,
  graph: GraphMemoryEvaluationArmRow,
  pins: GraphMemoryEvaluationPins,
): boolean => {
  const expected = rowIdentity(fixture);
  const sharedFields = [
    "inputDigest",
    "corpusDigest",
    "queryDigest",
    "policyDigest",
    "budgetDigest",
  ] as const;
  if (sharedFields.some((field) => history[field] !== graph[field])) return false;
  if (
    history.inputDigest !== expected.inputDigest ||
    history.corpusDigest !== expected.corpusDigest ||
    history.queryDigest !== expected.queryDigest ||
    history.policyDigest !== pins.policyDigest ||
    history.budgetDigest !== pins.budgetDigest
  )
    return false;
  const modelPin = pins.model;
  if (
    modelPin._tag === "NotUsed" &&
    [history, graph].some(
      (row) =>
        row.modelCalls !== modelPin.requiredModelCalls ||
        row.tokens._tag !== "Exact" ||
        row.tokens.inputTokens !== modelPin.requiredInputTokens ||
        row.tokens.outputTokens !== modelPin.requiredOutputTokens ||
        row.tokens.totalTokens !== 0,
    )
  )
    return false;
  for (const row of [history, graph]) {
    const activeSourceRefs = setOf(
      fixture.sources.filter((source) => !source.revoked).map((source) => source.sourceRef),
    );
    const mappedAliases = setOf(
      row.retrievalEvidence.mappings.map((mapping) => mapping.oracleElementAlias),
    );
    if (
      row.retrievedElementAliases.some((alias) => !mappedAliases.has(alias)) ||
      row.retrievalEvidence.mappingDigest !==
        graphMemoryEvaluationDigest(row.retrievalEvidence.mappings)
    )
      return false;
    const emitted = setOf(row.emittedCitationRefs);
    const valid = setOf(row.validCitationRefs);
    if (
      [...valid, ...row.retrievedSourceRefs].some((sourceRef) => !activeSourceRefs.has(sourceRef))
    )
      return false;
    if (row.citationEvidence.invalidCount !== emitted.size - intersectionCount(emitted, valid)) {
      return false;
    }
    const validationDigest =
      emitted.size === 0
        ? null
        : graphMemoryEvaluationDigest({
            emitted: [...emitted].toSorted(),
            valid: [...valid].toSorted(),
          });
    if (row.citationEvidence.validationDigest !== validationDigest) return false;
    if (
      row.extractionEvidence.status === "complete" &&
      (row.extractionEvidence.receiptDigest === null ||
        row.extractionEvidence.inputCorpusDigest === null ||
        row.extractionEvidence.budgetDigest === null ||
        row.extractionEvidence.graphStateDigest === null)
    )
      return false;
  }
  if (fixture.scenario.expectedCaps.some((cap) => !graph.hitCaps.includes(cap))) return false;
  return true;
};

const rowsMatchFixtures = (
  arm: GraphMemoryEvaluationArmRow["arm"],
  fixtures: ReadonlyArray<GraphMemoryEvaluationFixtureRow>,
  rows: ReadonlyArray<GraphMemoryEvaluationArmRow>,
): boolean => {
  const expected = fixtures.map((row) => row.rowId).toSorted();
  const observed = rows
    .filter((row) => row.arm === arm)
    .map((row) => row.rowId)
    .toSorted();
  return (
    canonicalJson(expected) === canonicalJson(observed) && rows.every((row) => row.arm === arm)
  );
};

const decodeInput = (input: unknown): GraphMemoryEvaluationInput | null => {
  const decoded = decodeGraphMemoryEvaluationInput(input);
  return decoded._tag === "Success" ? decoded.value : null;
};

/**
 * Compare history-only recall with graph-assisted recall.
 *
 * This function is pure. Provider, store, and clock effects stay in the runner
 * that supplies the observed row results.
 */
export const evaluateDesktopGraphMemoryComparison = (
  unknownInput: unknown,
): GraphMemoryEvaluationResult => {
  const input = decodeInput(unknownInput);
  if (input === null) return refusal("invalid_input", "The evaluation input is invalid.");
  if (canonicalJson(input.expectedPins) !== canonicalJson(input.observedPins)) {
    return refusal("pin_mismatch", "Observed evaluation pins do not match the expected pins.");
  }
  if (!pinsAreComplete(input.observedPins)) {
    return refusal("pin_mismatch", "The exact SDK package integrity set is incomplete.");
  }
  const dataset = validateGraphMemoryEvaluationDataset(
    input.development,
    input.holdout,
    input.manifest,
  );
  if (!dataset.ok) return dataset;
  if (
    input.observedPins.developmentSplitDigest !== dataset.developmentDigest ||
    input.observedPins.holdoutSplitDigest !== dataset.holdoutDigest ||
    input.observedPins.datasetRevisionDigest !== dataset.datasetRevisionDigest
  ) {
    return refusal("split_digest_mismatch", "The runtime pins do not match the fixture manifest.");
  }
  if (input.observedPins.qualityPolicyDigest !== input.manifest.qualityPolicyDigest) {
    return refusal("pin_mismatch", "The runtime quality policy pin does not match the manifest.");
  }
  if (
    !rowsMatchFixtures("history_only", input.holdout.rows, input.historyOnlyRows) ||
    !rowsMatchFixtures("graph_assisted", input.holdout.rows, input.graphAssistedRows)
  ) {
    return refusal(
      "row_result_mismatch",
      "Each holdout row must have exactly one result in each evaluation arm.",
    );
  }
  const historyById = new Map(input.historyOnlyRows.map((row) => [row.rowId, row]));
  const graphById = new Map(input.graphAssistedRows.map((row) => [row.rowId, row]));
  if (
    input.holdout.rows.some((fixture) => {
      const history = historyById.get(fixture.rowId);
      const graph = graphById.get(fixture.rowId);
      return (
        history === undefined ||
        graph === undefined ||
        !rowEvidenceIsValid(fixture, history, graph, input.observedPins)
      );
    })
  ) {
    return refusal(
      "row_result_mismatch",
      "Arm results do not share the pinned input or evidence identity.",
    );
  }

  const historyOnly = summarizeGraphMemoryEvaluationArm(
    "history_only",
    input.holdout.rows,
    input.historyOnlyRows,
  );
  const graphAssisted = summarizeGraphMemoryEvaluationArm(
    "graph_assisted",
    input.holdout.rows,
    input.graphAssistedRows,
  );
  const comparison = qualityComparison(
    input.observedPins,
    input.manifest.qualityPolicy,
    historyOnly,
    graphAssisted,
  );
  const nonCompleteRows =
    historyOnly.rows -
    historyOnly.outcomes.complete +
    graphAssisted.rows -
    graphAssisted.outcomes.complete;
  const evidence =
    nonCompleteRows === 0 && comparison.quality !== "inconclusive"
      ? "present"
      : nonCompleteRows < historyOnly.rows + graphAssisted.rows
        ? "partial"
        : "absent";

  const receipt = decodeDesktopGraphMemoryEvaluationReceipt({
    schemaVersion: DESKTOP_GRAPH_MEMORY_EVALUATION_SCHEMA_VERSION,
    issue: "OA-GMEM-04",
    evaluatedAt: input.evaluatedAt,
    pins: input.observedPins,
    dataset: {
      datasetRef: input.manifest.datasetRef,
      reviewState: input.manifest.reviewState,
      developmentRows: input.development.rows.length,
      holdoutRows: input.holdout.rows.length,
      physicalHoldoutIsolation: true,
      splitIdentityOverlap: 0,
      challengeClasses: dataset.challengeClasses,
    },
    rowEvidence: input.holdout.rows.map((fixture) => {
      const history = historyById.get(fixture.rowId);
      const graph = graphById.get(fixture.rowId);
      if (history === undefined || graph === undefined) {
        throw new Error("Validated row evidence is unavailable.");
      }
      return {
        rowId: fixture.rowId,
        inputDigest: history.inputDigest,
        corpusDigest: history.corpusDigest,
        queryDigest: history.queryDigest,
        policyDigest: history.policyDigest,
        budgetDigest: history.budgetDigest,
        historyOnly: {
          outcome: history.outcome,
          modelCalls: history.modelCalls,
          extractionEvidence: history.extractionEvidence,
          citationEvidence: history.citationEvidence,
        },
        graphAssisted: {
          outcome: graph.outcome,
          modelCalls: graph.modelCalls,
          extractionEvidence: graph.extractionEvidence,
          citationEvidence: graph.citationEvidence,
        },
      };
    }),
    privateDetailReceiptDigest: graphMemoryEvaluationPrivateDetailDigest(
      input.historyOnlyRows,
      input.graphAssistedRows,
    ),
    historyOnly,
    graphAssisted,
    comparison,
    disposition: {
      implementation: "implemented",
      evidence,
      quality: comparison.quality,
      ownerReview: "unreviewed",
      release: "not_released",
      publicClaim: "not_authorized",
    },
  });
  return { ok: true, receipt };
};
