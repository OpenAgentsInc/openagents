import { execFileSync } from "node:child_process";
import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";

import {
  GraphMemoryStore,
  graphMemoryScopeRefFor,
  ownerScopeId,
  projectScopeId,
  type GraphMemoryScope,
} from "@openagentsinc/agent-experience-memory";
import {
  buildTextDelta,
  buildTurnFinished,
  buildTurnStarted,
  makeInMemoryEventLogStore,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract";
import {
  buildInlineCorpusInput,
  makeInlineCorpusHandle,
  type RlmCorpusHandle,
  type RlmTerminalResult,
} from "@openagentsinc/rlm";
import { Effect, Schema } from "effect";

import {
  DESKTOP_GRAPH_MEMORY_FIXTURE_SCHEMA_VERSION,
  DESKTOP_GRAPH_MEMORY_MANIFEST_SCHEMA_VERSION,
  DESKTOP_GRAPH_MEMORY_SDK_TRAIN,
  GraphMemoryEvaluationFixtureFileSchema,
  GraphMemoryEvaluationManifestSchema,
  type GraphMemoryEvaluationArmRow,
  type GraphMemoryEvaluationFixtureFile,
  type GraphMemoryEvaluationFixtureRow,
  type GraphMemoryEvaluationOutcome,
  type GraphMemoryEvaluationPins,
} from "../src/desktop-graph-memory-evaluation-contract.js";
import {
  evaluateDesktopGraphMemoryComparison,
  graphMemoryEvaluationDigest,
  graphMemoryEvaluationPrivateDetailDigest,
  validateGraphMemoryEvaluationDataset,
} from "../src/desktop-graph-memory-evaluation.js";
import { openDesktopGraphMemoryStore } from "../src/desktop-graph-memory-store.js";
import {
  runDesktopGraphMemoryTurn,
  type DesktopGraphMemoryTurnEvidence,
  type DesktopGraphMemoryTurnResult,
} from "../src/desktop-graph-memory-turn.js";
import {
  DESKTOP_GRAPH_MEMORY_EXTRACTION_LIMITS,
  DESKTOP_GRAPH_MEMORY_RECALL_LIMITS,
  desktopGraphMemoryDeterministicExtractor,
  desktopGraphMemoryRecallQueryFor,
  makeDesktopGraphMemoryWorkflow,
} from "../src/desktop-graph-memory-workflow.js";
import {
  citedSpansFromRlmResult,
  runDesktopRlmDeterministicGrep,
  type HistoryRecallHostSources,
} from "../src/history-recall-host.js";
import type { SafeStorageLike } from "../src/desktop-session-vault.js";

const RUNNER_REF = "apps/openagents-desktop/scripts/graph-memory-evaluation.ts";
const ORACLE_REF = "apps/openagents-desktop/src/desktop-graph-memory-evaluation.ts";
const PARSER_REF = "parser.desktop.foreground-history";
const PARSER_VERSION = "version.1";
const FIXTURE_ROOT = "apps/openagents-desktop/tests/fixtures/graph-memory-evaluation/v1";
const TIMING_REF = "node.performance.now.recall_only.warm.3_repetitions";
const SOURCE = { lane: "test_fixture" } as const;
const POLICY = {
  includeVisibilities: ["private"],
  includeRedactionClasses: ["none"],
} as const;
const RECALL_REPETITIONS = 3;

const EXPECTED_SDK_PACKAGES = [
  [
    "@openagentsinc/ai",
    "sha512-mQN7iOA0EbXL8zwFxJr2hHy23dMivLAhGeJRd8TepjzcBRECKAVQ1fTFOsvr8Ik2g+Bvg2Wxw3mtoX+N7SCC7Q==",
  ],
  [
    "@openagentsinc/rlm",
    "sha512-M3JX7BJDvTBbjSltmLD7u5PRl7K1Ytzjvy5HdD/e25Ywixa1fJk0tX3i2DFUBLyJjnNPv4YO4QwosPnu7ILMMQ==",
  ],
  [
    "@openagentsinc/history-corpus",
    "sha512-2V9CW86+DMlxmcu9GjUUJj4bnTvj8OP99j64tOM4BJzj76Ci1Z+jT4b291UJyp8l9sYcQpNVcC/ID1pUVCiYcw==",
  ],
  [
    "@openagentsinc/agent-harness-contract",
    "sha512-3vIhogeE/Bbg5LfWKjQ8msCs7T7BB/ivE0Zd1VUVAA5Pz6WCsier6VLFapAhW6NBRnS0brvDhaerPiDBfkRqNQ==",
  ],
  [
    "@openagentsinc/agent-runtime-schema",
    "sha512-xSC/laUZcFaF8AUOEshQy01Q1QuOdrYAEx48PPUjRjPO/dP4IUuTQLkeio0EfyYKqu+VMxax9ttwleeEPnQKJA==",
  ],
  [
    "@openagentsinc/dse",
    "sha512-CFsEPBkJbQi/ZL/OQKOGqxahI4GNy9Es2b8p/uXD4xQaIFKm48DS7EtiEbJbLm3l5qXHNpR7D7HEFf6R4ljdHg==",
  ],
  [
    "@openagentsinc/graph-corpus",
    "sha512-8LFMS/WGvRKYmcst0GpqpLrjRMpWRoQ9aSshJqKPHBTxWdbQI3360y/VBa5YroBz7fEndPnmgJStdyEhDM2C3Q==",
  ],
  [
    "@openagentsinc/conformance-kit",
    "sha512-Hm2JMz1FKTuUNIxr9rEXKZ5RQuVLPZF8/UI2WdTTutt/dkbECjb8g1lpGK6tMJitw/NV0MbEbHK3z8PIqWXHKQ==",
  ],
] as const;

const repoRoot = path.resolve(import.meta.dirname, "../../..");
const absolute = (ref: string): string => path.join(repoRoot, ref);
const readJson = (ref: string): unknown => JSON.parse(readFileSync(absolute(ref), "utf8"));
const shaFile = (ref: string): string =>
  graphMemoryEvaluationDigest(readFileSync(absolute(ref)).toString("base64"));
const unique = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(values)].toSorted();

const writeAggregate = (value: unknown): void => {
  const encoded = `${JSON.stringify(value, null, 2)}\n`;
  const outputArg = process.argv.find((argument) => argument.startsWith("--output="));
  if (outputArg === undefined) {
    process.stdout.write(encoded);
    return;
  }
  const outputRef = outputArg.slice("--output=".length);
  const allowedPrefix = "apps/openagents-desktop/benchmarks/graph-memory/";
  if (!outputRef.startsWith(allowedPrefix) || !outputRef.endsWith(".json")) {
    throw new Error("The evaluation output must be a graph-memory benchmark JSON file.");
  }
  const outputPath = absolute(outputRef);
  mkdirSync(path.dirname(outputPath), { recursive: true });
  writeFileSync(outputPath, encoded, { mode: 0o644 });
};

const makeProcessSafeStorage = (): Readonly<{
  safeStorage: SafeStorageLike;
  destroy: () => void;
}> => {
  const key = randomBytes(32);
  return {
    safeStorage: {
      isEncryptionAvailable: () => true,
      getSelectedStorageBackend: () => "standalone-proof-process-key",
      encryptString: (plainText) => {
        const nonce = randomBytes(12);
        const cipher = createCipheriv("aes-256-gcm", key, nonce);
        const payload = Buffer.concat([cipher.update(plainText, "utf8"), cipher.final()]);
        return Buffer.concat([nonce, cipher.getAuthTag(), payload]);
      },
      decryptString: (encrypted) => {
        const decipher = createDecipheriv("aes-256-gcm", key, encrypted.subarray(0, 12));
        decipher.setAuthTag(encrypted.subarray(12, 28));
        return Buffer.concat([decipher.update(encrypted.subarray(28)), decipher.final()]).toString(
          "utf8",
        );
      },
    },
    destroy: () => key.fill(0),
  };
};

const activeSources = (row: GraphMemoryEvaluationFixtureRow) =>
  row.sources.filter((source) => !source.revoked);

const scenarioPlan = (row: GraphMemoryEvaluationFixtureRow) => {
  const byRef = new Map(row.sources.map((source) => [source.sourceRef, source]));
  const current = new Map<string, (typeof row.sources)[number]>();
  let graphSnapshot: ReadonlyArray<(typeof row.sources)[number]> | null = null;
  let partialExtraction = false;
  let graphAdvanced = false;
  let revokedCount = 0;
  for (const step of row.scenario.steps) {
    const source = step.sourceRef === null ? undefined : byRef.get(step.sourceRef);
    switch (step.operation) {
      case "ingest":
        if (source === undefined) throw new Error("An ingest scenario source is absent.");
        current.set(source.sourceRef, source);
        break;
      case "revoke":
        if (source === undefined || !current.delete(source.sourceRef)) {
          throw new Error("A revoke scenario source is not active.");
        }
        revokedCount += 1;
        break;
      case "replace":
        if (source === undefined) throw new Error("A replacement scenario source is absent.");
        for (const candidate of row.sources) {
          if (candidate.revoked) current.delete(candidate.sourceRef);
        }
        current.set(source.sourceRef, source);
        break;
      case "extract_partial":
        partialExtraction = true;
        break;
      case "snapshot_graph":
        graphSnapshot = [...current.values()];
        break;
      case "advance_graph":
        graphAdvanced = true;
        break;
    }
  }
  const currentSources = [...current.values()];
  if (
    graphMemoryEvaluationDigest(currentSources) !== graphMemoryEvaluationDigest(activeSources(row))
  ) {
    throw new Error("The scenario result does not match the reviewed active sources.");
  }
  return {
    currentSources,
    setupSources: graphSnapshot ?? currentSources,
    partialExtraction,
    graphAdvanced,
    revokedCount,
  };
};

/*
 * This identity is the exact recall input. The reviewed natural-language
 * question remains in the fixture, while both arms receive the same shipped
 * deterministic query projection.
 */
const rowIdentity = (row: GraphMemoryEvaluationFixtureRow, pins: GraphMemoryEvaluationPins) => ({
  inputDigest: graphMemoryEvaluationDigest({
    rowId: row.rowId,
    query: desktopGraphMemoryRecallQueryFor(row.query),
    sources: activeSources(row),
    scenario: row.scenario,
  }),
  corpusDigest: graphMemoryEvaluationDigest(activeSources(row)),
  queryDigest: graphMemoryEvaluationDigest(desktopGraphMemoryRecallQueryFor(row.query)),
  policyDigest: pins.policyDigest,
  budgetDigest: pins.budgetDigest,
});

const retrievalProjection = (
  arm: GraphMemoryEvaluationArmRow["arm"],
  citedSourceRefs: ReadonlyArray<string>,
  observedElementRefs: ReadonlyArray<string>,
) => {
  const mappings = citedSourceRefs.flatMap((sourceRef, index) => {
    const observedRef = observedElementRefs[index] ?? observedElementRefs[0] ?? sourceRef;
    if (observedRef === "") return [];
    return [
      {
        observedElementDigest: graphMemoryEvaluationDigest({ arm, observedRef }),
        oracleElementAlias: sourceRef,
      },
    ];
  });
  return {
    retrievedElementAliases: mappings.map(({ oracleElementAlias }) => oracleElementAlias),
    retrievalEvidence: {
      mappings,
      mappingDigest: graphMemoryEvaluationDigest(mappings),
    },
  };
};

const citationEvidence = (emitted: ReadonlyArray<string>, valid: ReadonlyArray<string>) => {
  const emittedUnique = unique(emitted);
  const validUnique = unique(valid);
  return {
    validationDigest:
      emittedUnique.length === 0
        ? null
        : graphMemoryEvaluationDigest({ emitted: emittedUnique, valid: validUnique }),
    invalidCount: emittedUnique.filter((ref) => !validUnique.includes(ref)).length,
  };
};

const scriptedTurn = (
  turnId: string,
  threadId: string,
  text: string,
): ReadonlyArray<HarnessStreamEvent> => [
  buildTurnStarted({
    turnId,
    threadId,
    sequence: 0,
    source: SOURCE,
    observedAt: "2026-07-22T10:00:00.000Z",
  }),
  buildTextDelta({
    turnId,
    threadId,
    sequence: 1,
    source: SOURCE,
    observedAt: "2026-07-22T10:00:01.000Z",
    messageId: `message.${turnId}`,
    text,
  }),
  buildTurnFinished({
    turnId,
    threadId,
    sequence: 2,
    source: SOURCE,
    observedAt: "2026-07-22T10:00:02.000Z",
    finishReason: "stop",
  }),
];

const historySourcesFor = async (row: GraphMemoryEvaluationFixtureRow) => {
  const eventLog = makeInMemoryEventLogStore();
  const threadId = `thread.evaluation.${graphMemoryEvaluationDigest(row.rowId).slice(0, 24)}`;
  const turns = activeSources(row).map((source) => ({
    source,
    turnId: `turn.evaluation.${graphMemoryEvaluationDigest(source.sourceRef).slice(0, 24)}`,
  }));
  for (const { source, turnId } of turns) {
    for (const event of scriptedTurn(turnId, threadId, source.text)) {
      await Effect.runPromise(eventLog.append(event));
    }
  }
  const sources: HistoryRecallHostSources = {
    eventLog,
    turnIdsForThread: (candidate) =>
      candidate === threadId ? turns.map(({ turnId }) => turnId) : [],
    authorizeThread: (candidate) => candidate === threadId,
    builtAt: () => "2026-07-22T10:00:03.000Z",
    source: SOURCE,
  };
  return {
    sources,
    threadId,
    sourceRefByTurn: new Map(turns.map(({ source, turnId }) => [turnId, source.sourceRef])),
  };
};

const outcomeForRlm = (result: RlmTerminalResult): GraphMemoryEvaluationOutcome =>
  result._tag === "Completed" ? "complete" : result._tag === "Partial" ? "partial" : "refused";

const runHistoryRow = async (
  row: GraphMemoryEvaluationFixtureRow,
  pins: GraphMemoryEvaluationPins,
): Promise<GraphMemoryEvaluationArmRow> => {
  const scenario = scenarioPlan(row);
  const history = await historySourcesFor(row);
  const samples: number[] = [];
  let result: RlmTerminalResult | null = null;
  for (let index = 0; index < RECALL_REPETITIONS; index += 1) {
    const started = performance.now();
    result = await Effect.runPromise(
      runDesktopRlmDeterministicGrep(history.sources, {
        scope: { _tag: "Thread", threadId: history.threadId },
        pattern: desktopGraphMemoryRecallQueryFor(row.query),
        runRef: `run.evaluation.history.${row.rowId}.${index}`,
        maxSpans: DESKTOP_GRAPH_MEMORY_RECALL_LIMITS.maxReturnedElements,
      }),
    );
    samples.push(performance.now() - started);
  }
  if (result === null) throw new Error("History recall did not run.");
  const citedSourceRefs = unique(
    citedSpansFromRlmResult(result)
      .map((span) => history.sourceRefByTurn.get(span.turnId))
      .filter((ref): ref is string => ref !== undefined),
  );
  const active = new Set(scenario.currentSources.map((source) => source.sourceRef));
  const validatedCount = Math.min(result.honesty.citationValidated, citedSourceRefs.length);
  const validSourceRefs = citedSourceRefs
    .filter((sourceRef) => active.has(sourceRef))
    .slice(0, validatedCount);
  const retrieval = retrievalProjection("history_only", citedSourceRefs, citedSourceRefs);
  return {
    rowId: row.rowId,
    arm: "history_only",
    outcome: outcomeForRlm(result),
    ...rowIdentity(row, pins),
    modelCalls: result.usage.modelCalls,
    extractionEvidence: {
      status: "not_run",
      receiptDigest: null,
      usageTruth: "not_run",
      inputCorpusDigest: null,
      budgetDigest: null,
      graphStateDigest: null,
      entityCount: 0,
      mergeCount: 0,
    },
    citationEvidence: citationEvidence(citedSourceRefs, validSourceRefs),
    emittedAnswerFactRefs: [],
    emittedCitationRefs: citedSourceRefs,
    validCitationRefs: validSourceRefs,
    mergedEntityPairs: [],
    observedEntityRefs: [],
    retrievedSourceRefs: citedSourceRefs,
    ...retrieval,
    recallLatencySamplesMs: samples,
    setupLatencyMs: null,
    tokens: { _tag: "Exact", inputTokens: 0, outputTokens: 0, totalTokens: 0 },
    truncated: result._tag === "Partial",
    hitCaps: unique([
      ...result.honesty.capsHit,
      ...(scenario.revokedCount > 0 ? ["revoked_source_excluded"] : []),
    ]),
  };
};

const corpusHandles = async (
  row: GraphMemoryEvaluationFixtureRow,
  scope: GraphMemoryScope,
  sources: ReadonlyArray<GraphMemoryEvaluationFixtureRow["sources"][number]> = activeSources(row),
): Promise<ReadonlyArray<RlmCorpusHandle>> =>
  Promise.all(
    sources.map((source) =>
      Effect.runPromise(
        makeInlineCorpusHandle(
          buildInlineCorpusInput({
            corpusRef: `corpus.evaluation.${graphMemoryEvaluationDigest(source.sourceRef)}`,
            scopeRef: graphMemoryScopeRefFor(scope),
            policy: POLICY,
            entries: [
              {
                entryRef: source.sourceRef,
                scopeRef: graphMemoryScopeRefFor(scope),
                sourcePlane: "evidence_pack",
                sourceKind: "graph-memory-evaluation",
                sourceAddress: {
                  addressSchemaId: "openagents.desktop.graph_memory_evaluation_source.v1",
                  encodedAddress: `fixture:${source.sourceRef}`,
                },
                text: source.text,
                visibility: "private",
                redactionClass: "none",
              },
            ],
          }),
        ),
      ),
    ),
  );

const extractionStatus = (
  result: DesktopGraphMemoryTurnResult | null,
): GraphMemoryEvaluationArmRow["extractionEvidence"]["status"] => {
  if (result === null || result._tag !== "Completed") return "failed";
  switch (result.extraction._tag) {
    case "Stored":
    case "AlreadyStored":
      return "complete";
    case "Incomplete":
      return "partial";
    case "SpendRefused":
      return "refused";
    case "Disabled":
      return "not_run";
  }
};

const safeFailureRef = (phase: "setup" | "recall", error: unknown): string => {
  const record =
    typeof error === "object" && error !== null ? (error as Record<string, unknown>) : {};
  const tag = typeof record._tag === "string" ? record._tag : "unknown";
  const reason = typeof record.reason === "string" ? record.reason : "unknown";
  const safe = `${tag}.${reason}`.replaceAll(/[^A-Za-z0-9_.-]/gu, "_").slice(0, 160);
  return `graph_${phase}_failure.${safe}`;
};

const runGraphRow = async (
  row: GraphMemoryEvaluationFixtureRow,
  pins: GraphMemoryEvaluationPins,
  temporaryRoot: string,
  safeStorage: SafeStorageLike,
): Promise<GraphMemoryEvaluationArmRow> => {
  const scenario = scenarioPlan(row);
  const scope: GraphMemoryScope = {
    owner: ownerScopeId(`owner.evaluation.${graphMemoryEvaluationDigest(row.rowId).slice(0, 24)}`),
    project: projectScopeId(
      `project.evaluation.${graphMemoryEvaluationDigest(row.rowId).slice(0, 24)}`,
    ),
  };
  const currentHandles = await corpusHandles(row, scope, scenario.currentSources);
  const setupHandles = await corpusHandles(row, scope, scenario.setupSources);
  const extractionLimits = scenario.partialExtraction
    ? {
        ...DESKTOP_GRAPH_MEMORY_EXTRACTION_LIMITS,
        maxCharacters: 1,
        maxInputTokens: 1,
        maxCharactersPerBatch: 1,
        maxInputTokensPerBatch: 1,
      }
    : DESKTOP_GRAPH_MEMORY_EXTRACTION_LIMITS;
  const databasePath = path.join(temporaryRoot, `${graphMemoryEvaluationDigest(row.rowId)}.sqlite`);
  const evidence: DesktopGraphMemoryTurnEvidence[] = [];
  let store = openDesktopGraphMemoryStore({ enabled: true, databasePath, safeStorage });
  let setupResult: DesktopGraphMemoryTurnResult | null = null;
  let setupError = false;
  let setupFailureRef: string | null = null;
  let graphStateDigest: string | null = null;
  let graphEntityCount = 0;
  let graphMergeCount = 0;
  let observedEntityRefs: ReadonlyArray<string> = [];
  const setupStarted = performance.now();
  try {
    setupResult = await Effect.runPromise(
      runDesktopGraphMemoryTurn(
        {
          turnRef: `turn.evaluation.setup.${row.rowId}`,
          mode: "foreground",
          prompt: row.query,
          recallQuery: desktopGraphMemoryRecallQueryFor(row.query),
          scope,
          extractionEnabled: true,
          recallEnabled: false,
          admission: {
            consent: "granted",
            consentRef: "consent.graph-memory-evaluation.v1",
            policyRef: "policy.graph-memory-evaluation.v1",
            redactionState: "already_redacted",
          },
          policy: POLICY,
        },
        {
          resolveSources: () =>
            Effect.succeed(
              setupHandles.map((handle) => ({
                handle,
                redactionState: "already_redacted" as const,
              })),
            ),
          extraction: {
            _tag: "Deterministic",
            extractor: desktopGraphMemoryDeterministicExtractor,
          },
          extractionLimits,
          recallLimits: DESKTOP_GRAPH_MEMORY_RECALL_LIMITS,
          countTokens: (text) => text.length,
          monotonicMs: () => Math.floor(performance.now()),
          now: () => "2026-07-22T10:01:00.000Z",
          emitEvidence: (record) =>
            Effect.sync(() => {
              evidence.push(record);
            }),
        },
      ).pipe(Effect.provide(store.layer)),
    );
    const inspection = await Effect.runPromise(
      Effect.gen(function* () {
        const graphStore = yield* GraphMemoryStore;
        return yield* graphStore.inspect(scope);
      }).pipe(Effect.provide(store.layer)),
    );
    if (inspection.current !== null) {
      const graph = inspection.current.built.snapshot;
      graphEntityCount = graph.entities.length;
      graphMergeCount = graph.merges.length;
      graphStateDigest = graphMemoryEvaluationDigest({
        graphDigest: graph.graphDigest,
        entityRefs: graph.entities.map((entity) => entity.elementRef),
        mergeRefs: graph.merges.map((merge) => merge.mergeRef),
      });
      observedEntityRefs = row.entityAliases
        .filter((gold) =>
          graph.entities.some((entity) =>
            gold.aliases.some((alias) =>
              entity.identity.canonicalKey
                .toLocaleLowerCase("en-US")
                .includes(alias.toLocaleLowerCase("en-US")),
            ),
          ),
        )
        .map((gold) => gold.entityRef);
    }
  } catch (error) {
    setupError = true;
    setupFailureRef = safeFailureRef("setup", error);
  }
  const setupLatencyMs = performance.now() - setupStarted;
  store.close();
  store = openDesktopGraphMemoryStore({ enabled: true, databasePath, safeStorage });

  const samples: number[] = [];
  let recallResult: DesktopGraphMemoryTurnResult | null = null;
  let recallError = false;
  let recallFailureRef: string | null = null;
  for (let index = 0; index < RECALL_REPETITIONS; index += 1) {
    const started = performance.now();
    try {
      recallResult = await Effect.runPromise(
        runDesktopGraphMemoryTurn(
          {
            turnRef: `turn.evaluation.recall.${row.rowId}.${index}`,
            mode: "foreground",
            prompt: row.query,
            recallQuery: desktopGraphMemoryRecallQueryFor(row.query),
            scope,
            extractionEnabled: false,
            recallEnabled: true,
            admission: {
              consent: "granted",
              consentRef: "consent.graph-memory-evaluation.v1",
              policyRef: "policy.graph-memory-evaluation.v1",
              redactionState: "already_redacted",
            },
            policy: POLICY,
          },
          {
            resolveSources: () =>
              Effect.succeed(
                currentHandles.map((handle) => ({
                  handle,
                  redactionState: "already_redacted" as const,
                })),
              ),
            extraction: {
              _tag: "Deterministic",
              extractor: desktopGraphMemoryDeterministicExtractor,
            },
            extractionLimits: DESKTOP_GRAPH_MEMORY_EXTRACTION_LIMITS,
            recallLimits: DESKTOP_GRAPH_MEMORY_RECALL_LIMITS,
            countTokens: (text) => text.length,
            monotonicMs: () => Math.floor(performance.now()),
            now: () => "2026-07-22T10:02:00.000Z",
            emitEvidence: (record) =>
              Effect.sync(() => {
                evidence.push(record);
              }),
          },
        ).pipe(Effect.provide(store.layer)),
      );
    } catch (error) {
      recallError = true;
      recallFailureRef = safeFailureRef("recall", error);
    }
    samples.push(performance.now() - started);
  }
  store.close();

  const setupEvidence =
    evidence.findLast((record) => record.extractionReceiptDigest !== null) ?? null;
  const finalEvidence =
    evidence.findLast((record) => record.operationDigest !== null) ?? setupEvidence;
  const citedSourceRefs = unique(
    finalEvidence?.citations.map((citation) => citation.entryRef) ?? [],
  );
  const activeSourceRefs = new Set(scenario.currentSources.map((source) => source.sourceRef));
  const validSourceRefs = citedSourceRefs.filter((sourceRef) => activeSourceRefs.has(sourceRef));
  const observedElements = finalEvidence?.usedElementRefs ?? [];
  const retrieval = retrievalProjection("graph_assisted", citedSourceRefs, observedElements);
  const setupStatus = extractionStatus(setupResult);
  const outcome: GraphMemoryEvaluationOutcome = setupError
    ? "failed"
    : setupStatus === "partial"
      ? "partial"
      : setupStatus === "refused"
        ? "refused"
        : recallError
          ? "failed"
          : finalEvidence?.truncated === true
            ? "partial"
            : "complete";
  const scenarioCaps = [
    ...(scenario.revokedCount > 0 ? ["revoked_source_excluded"] : []),
    ...(scenario.partialExtraction && setupStatus === "partial"
      ? ["partial_extraction_reported"]
      : []),
    ...(row.challengeClasses.includes("prompt_injection") && !setupError
      ? ["prompt_injection_treated_as_data"]
      : []),
    ...(scenario.graphAdvanced && recallError ? ["stale_graph_failed_closed"] : []),
  ];
  return {
    rowId: row.rowId,
    arm: "graph_assisted",
    outcome,
    ...rowIdentity(row, pins),
    modelCalls: setupEvidence?.extractionModelCalls ?? 0,
    extractionEvidence: {
      status: setupStatus,
      receiptDigest: setupEvidence?.extractionReceiptDigest ?? null,
      usageTruth:
        setupEvidence?.extractionUsageTruth === "exact"
          ? "exact"
          : setupEvidence?.extractionUsageTruth === "unavailable"
            ? "unavailable"
            : "not_run",
      inputCorpusDigest: graphMemoryEvaluationDigest(scenario.setupSources),
      budgetDigest: graphMemoryEvaluationDigest(extractionLimits),
      graphStateDigest,
      entityCount: graphEntityCount,
      mergeCount: graphMergeCount,
    },
    citationEvidence: citationEvidence(citedSourceRefs, validSourceRefs),
    emittedAnswerFactRefs: [],
    emittedCitationRefs: citedSourceRefs,
    validCitationRefs: validSourceRefs,
    mergedEntityPairs: [],
    observedEntityRefs,
    retrievedSourceRefs: citedSourceRefs,
    ...retrieval,
    recallLatencySamplesMs: samples,
    setupLatencyMs,
    tokens: { _tag: "Exact", inputTokens: 0, outputTokens: 0, totalTokens: 0 },
    truncated: outcome === "partial" || finalEvidence?.truncated === true,
    hitCaps: unique([
      ...(finalEvidence?.hitCaps ?? []),
      ...scenarioCaps,
      ...(setupFailureRef === null ? [] : [setupFailureRef]),
      ...(recallFailureRef === null ? [] : [recallFailureRef]),
    ]),
  };
};

const listBuildArtifacts = (): ReadonlyArray<{ ref: string; digest: string }> => {
  const distRoot = absolute("apps/openagents-desktop/dist");
  const walk = (directory: string): string[] =>
    readdirSync(directory).flatMap((name) => {
      const candidate = path.join(directory, name);
      return statSync(candidate).isDirectory() ? walk(candidate) : [candidate];
    });
  return walk(distRoot)
    .map((file) => {
      const ref = path.relative(repoRoot, file).split(path.sep).join("/");
      return { ref, digest: shaFile(ref) };
    })
    .toSorted((left, right) => left.ref.localeCompare(right.ref));
};

const verifySdkPins = (): GraphMemoryEvaluationPins["sdkPackages"] => {
  const lock = readFileSync(absolute("pnpm-lock.yaml"), "utf8");
  for (const [packageName, integrity] of EXPECTED_SDK_PACKAGES) {
    if (
      !lock.includes(`'${packageName}@${DESKTOP_GRAPH_MEMORY_SDK_TRAIN}':`) ||
      !lock.includes(integrity)
    ) {
      throw new Error(`The lock does not contain the expected ${packageName} integrity.`);
    }
  }
  return EXPECTED_SDK_PACKAGES.map(([packageName, integrity]) => ({
    package: packageName,
    version: DESKTOP_GRAPH_MEMORY_SDK_TRAIN,
    integrity,
  }));
};

const productWiringSmoke = async (
  row: GraphMemoryEvaluationFixtureRow,
  temporaryRoot: string,
  safeStorage: SafeStorageLike,
): Promise<string> => {
  const databasePath = path.join(temporaryRoot, "product-wiring.sqlite");
  const store = openDesktopGraphMemoryStore({ enabled: true, databasePath, safeStorage });
  const evidence: DesktopGraphMemoryTurnEvidence[] = [];
  const workflow = makeDesktopGraphMemoryWorkflow({
    preferences: () => ({ graphExtractionEnabled: true, graphRecallEnabled: true }),
    ownerScope: () => "owner.graph-memory-evaluation",
    projectScope: () => "project.graph-memory-evaluation",
    openStore: async () => store,
    emitEvidence: async (record) => {
      evidence.push(record);
    },
    now: () => new Date("2026-07-22T10:03:00.000Z"),
  });
  const output = await workflow.beforeTurn({
    turnRef: "turn.evaluation.product-wiring",
    threadRef: "thread.evaluation.product-wiring",
    history: activeSources(row).map((source, index) => ({
      role: index % 2 === 0 ? ("user" as const) : ("assistant" as const),
      text: source.text,
    })),
    message: row.query,
  });
  store.close();
  const recall = evidence.findLast((record) => record.operationDigest !== null);
  if (
    recall === undefined ||
    recall.citations.length === 0 ||
    !output.message.includes("GRAPH MEMORY ADVISORY")
  ) {
    throw new Error("The product graph-memory workflow did not emit a cited advisory.");
  }
  return graphMemoryEvaluationDigest({
    schema: recall.schemaId,
    graphDigest: recall.graphDigest,
    operationDigest: recall.operationDigest,
    citationDigests: recall.citations.map((citation) => citation.citationDigest),
    profilePromotion: recall.profilePromotion,
  });
};

const runRows = async (
  rows: ReadonlyArray<GraphMemoryEvaluationFixtureRow>,
  pins: GraphMemoryEvaluationPins,
  temporaryRoot: string,
  safeStorage: SafeStorageLike,
) => {
  const historyOnlyRows: GraphMemoryEvaluationArmRow[] = [];
  const graphAssistedRows: GraphMemoryEvaluationArmRow[] = [];
  for (const row of rows) {
    historyOnlyRows.push(await runHistoryRow(row, pins));
    graphAssistedRows.push(await runGraphRow(row, pins, temporaryRoot, safeStorage));
  }
  return { historyOnlyRows, graphAssistedRows };
};

const main = async (): Promise<void> => {
  if (
    execFileSync("git", ["status", "--porcelain"], { cwd: repoRoot, encoding: "utf8" }).trim() !==
    ""
  ) {
    throw new Error("The graph-memory evaluation requires a clean source worktree.");
  }
  const development = Schema.decodeUnknownSync(GraphMemoryEvaluationFixtureFileSchema)(
    readJson(`${FIXTURE_ROOT}/development.json`),
  );
  const manifest = Schema.decodeUnknownSync(GraphMemoryEvaluationManifestSchema)(
    readJson(`${FIXTURE_ROOT}/manifest.json`),
  );
  if (
    development.schemaVersion !== DESKTOP_GRAPH_MEMORY_FIXTURE_SCHEMA_VERSION ||
    manifest.schemaVersion !== DESKTOP_GRAPH_MEMORY_MANIFEST_SCHEMA_VERSION
  )
    throw new Error("The evaluation fixture schema is not pinned.");

  const temporaryRoot = mkdtempSync(path.join(tmpdir(), "openagents-graph-memory-evaluation-"));
  const processStorage = makeProcessSafeStorage();
  try {
    const buildArtifacts = listBuildArtifacts();
    const smokeDigest = await productWiringSmoke(
      development.rows[0]!,
      temporaryRoot,
      processStorage.safeStorage,
    );
    const openAgentsCommit = execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: repoRoot,
      encoding: "utf8",
    }).trim();
    const basePins = {
      sdkTrain: DESKTOP_GRAPH_MEMORY_SDK_TRAIN,
      sdkPackages: verifySdkPins(),
      lockDigest: shaFile("pnpm-lock.yaml"),
      openAgentsCommit,
      sourceState: "clean" as const,
      desktopBuildRef: "apps/openagents-desktop/dist",
      desktopBuildDigest: graphMemoryEvaluationDigest(buildArtifacts),
      desktopBuildArtifacts: buildArtifacts,
      productWiringSmokeDigest: smokeDigest,
      runnerRef: RUNNER_REF,
      runnerDigest: shaFile(RUNNER_REF),
      oracleRef: ORACLE_REF,
      oracleDigest: shaFile(ORACLE_REF),
      runtime: { node: process.version, platform: process.platform, architecture: process.arch },
      timingRef: TIMING_REF,
      model: {
        _tag: "NotUsed" as const,
        reason: "deterministic_evaluation" as const,
        requiredModelCalls: 0 as const,
        requiredInputTokens: 0 as const,
        requiredOutputTokens: 0 as const,
      },
      parserRef: PARSER_REF,
      parserVersion: PARSER_VERSION,
      parserArtifactDigest: shaFile("apps/openagents-desktop/src/desktop-graph-memory-workflow.ts"),
      datasetRevisionDigest: manifest.datasetRevisionDigest,
      developmentSplitDigest: manifest.developmentDigest,
      holdoutSplitDigest: manifest.holdoutDigest,
      corpusDigest: graphMemoryEvaluationDigest(development.rows.flatMap(activeSources)),
      policyDigest: graphMemoryEvaluationDigest(POLICY),
      budgetDigest: graphMemoryEvaluationDigest({
        normalizedRecallResultCap: DESKTOP_GRAPH_MEMORY_RECALL_LIMITS.maxReturnedElements,
        queryProjection: "desktopGraphMemoryRecallQueryFor",
        repetitions: RECALL_REPETITIONS,
      }),
      qualityPolicyDigest: manifest.qualityPolicyDigest,
    } satisfies GraphMemoryEvaluationPins;

    // Run development before the holdout file is loaded. This freezes the
    // runner, parser, policy, budget, build, and quality pins first.
    await runRows(development.rows, basePins, temporaryRoot, processStorage.safeStorage);
    const holdout = Schema.decodeUnknownSync(GraphMemoryEvaluationFixtureFileSchema)(
      readJson(`${FIXTURE_ROOT}/holdout.json`),
    );
    const dataset = validateGraphMemoryEvaluationDataset(development, holdout, manifest);
    if (!dataset.ok) throw new Error(dataset.detailSafe);
    const pins: GraphMemoryEvaluationPins = {
      ...basePins,
      corpusDigest: graphMemoryEvaluationDigest([
        ...development.rows.flatMap(activeSources),
        ...holdout.rows.flatMap(activeSources),
      ]),
    };
    const holdoutRows = await runRows(
      holdout.rows,
      pins,
      temporaryRoot,
      processStorage.safeStorage,
    );
    const detailDigest = graphMemoryEvaluationPrivateDetailDigest(
      holdoutRows.historyOnlyRows,
      holdoutRows.graphAssistedRows,
    );
    const privateDetailPath = path.join(temporaryRoot, "owner-local-row-detail.json");
    writeFileSync(privateDetailPath, JSON.stringify(holdoutRows), { mode: 0o600 });
    if (
      graphMemoryEvaluationDigest(JSON.parse(readFileSync(privateDetailPath, "utf8"))) !==
      detailDigest
    ) {
      throw new Error("The private detail receipt digest does not match its bytes.");
    }
    const evaluated = evaluateDesktopGraphMemoryComparison({
      evaluatedAt: "2026-07-22T10:04:00.000Z",
      expectedPins: pins,
      observedPins: pins,
      development,
      holdout,
      manifest,
      ...holdoutRows,
    });
    if (!evaluated.ok) throw new Error(evaluated.detailSafe);
    if (evaluated.receipt.privateDetailReceiptDigest !== detailDigest) {
      throw new Error("The public receipt does not bind the private row detail.");
    }
    rmSync(privateDetailPath);
    writeAggregate(evaluated.receipt);
  } finally {
    processStorage.destroy();
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
};

await main();
