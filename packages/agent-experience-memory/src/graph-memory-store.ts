import {
  buildGraphCorpus,
  canonicalJson,
  graphDigest,
  makeGraphAdapterCapabilities,
  sha256Hex,
  verifyBuiltGraphCorpus,
  type BuiltGraphCorpus,
  GraphDigest,
} from "@openagentsinc/graph-corpus";
import {
  encodeGraphCorpusArchive,
  GraphArchiveRef,
  GraphArchiveSummaryRecord,
  GraphArchiveVectorRecord,
  importGraphCorpusArchive,
} from "@openagentsinc/graph-corpus/archive";
import {
  GraphArtifactInventory,
  makeCompleteGraphDeleteExecutionResult,
  makeGraphArtifactInventory,
  makeGraphDeleteReceipt,
  requireExecutableGraphDeletePlan,
  type GraphCompleteDeletePlan,
  type GraphDeletePlan,
  type GraphDerivedArtifactAction,
} from "@openagentsinc/graph-corpus/deletion";
import {
  GraphRankingSnapshot,
  validateGraphRankingSnapshotIntegrity,
} from "@openagentsinc/graph-corpus/ranking";
import { GraphManifest, GraphScopeRef, GraphSnapshot } from "@openagentsinc/graph-corpus/schemas";
import { Context, Effect, Layer, Ref, Result, Schema as S } from "effect";

import { ProjectScopeId, OwnerScopeId } from "./contract/refs.js";
import { guardMemoryText } from "./redaction.js";

export const GRAPH_MEMORY_STORE_SCHEMA_ID = "openagents.graph_memory_store.v1" as const;
export const GRAPH_MEMORY_RECEIPT_SCHEMA_ID = "openagents.graph_memory_receipt.v1" as const;
export const GRAPH_MEMORY_STATE_SCHEMA_ID = "openagents.graph_memory_state.v1" as const;
export const GRAPH_MEMORY_RECEIPT_LIMIT = 10_000;
export const GRAPH_MEMORY_SECTION_ITEM_LIMIT = 10_000;
export const GRAPH_MEMORY_GRAPH_ELEMENT_LIMIT = 10_000;
export const GRAPH_MEMORY_SOURCE_MEMBERSHIP_LIMIT = 10_000;
export const GRAPH_MEMORY_ARTIFACT_LIMIT = 10_000;
export const GRAPH_MEMORY_UNRESOLVED_LIMIT = 10_000;
/**
 * Accepted delete intended facts are at most 2G+A+R+E (50,000). Retained facts
 * are at most G+X+E (30,000). Forget intended facts are at most
 * 1+A+R+X+E (40,001). The 12-unit ceiling also bounds refused external plans.
 */
export const GRAPH_MEMORY_LIFECYCLE_FACT_LIMIT =
  12 * GRAPH_MEMORY_SECTION_ITEM_LIMIT;
export const GRAPH_MEMORY_CAS_ATTEMPT_LIMIT = 8;

const boundedRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(512),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:/-]*$/),
);
const NonNegativeInteger = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0));
const PositiveInteger = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1));

export const GraphMemoryOperationRef = boundedRef.pipe(S.brand("GraphMemoryOperationRef"));
export type GraphMemoryOperationRef = typeof GraphMemoryOperationRef.Type;

export const GraphMemoryScope = S.Struct({
  owner: OwnerScopeId,
  project: ProjectScopeId,
});
export interface GraphMemoryScope extends S.Schema.Type<typeof GraphMemoryScope> {}

export const GraphMemorySourceBinding = S.Struct({
  corpusRef: boundedRef,
  contentDigest: GraphDigest,
});
export interface GraphMemorySourceBinding
  extends S.Schema.Type<typeof GraphMemorySourceBinding> {}

export const GraphMemoryBinding = S.Struct({
  owner: OwnerScopeId,
  project: ProjectScopeId,
  graphScopeRef: GraphScopeRef,
  sourceBindings: S.Array(GraphMemorySourceBinding).check(S.isMaxLength(10_000)),
  graphRef: GraphSnapshot.fields.graphRef,
  graphDigest: GraphDigest,
  manifestDigest: GraphDigest,
  policyDigest: GraphDigest,
  generation: PositiveInteger,
});
export interface GraphMemoryBinding extends S.Schema.Type<typeof GraphMemoryBinding> {}

export const GraphMemoryAdmission = S.Struct({
  consent: S.Literals(["granted", "withheld"]),
  consentRef: boundedRef,
  policyRef: boundedRef,
  redactionState: S.Literals(["already_redacted", "unreviewed"]),
});
export interface GraphMemoryAdmission extends S.Schema.Type<typeof GraphMemoryAdmission> {}

const GraphMemoryBuiltGraph = S.Struct({
  snapshot: GraphSnapshot,
  manifest: GraphManifest,
});

export const GraphMemoryStoredGraph = S.Struct({
  schemaId: S.Literal(GRAPH_MEMORY_STORE_SCHEMA_ID),
  binding: GraphMemoryBinding,
  admission: GraphMemoryAdmission,
  built: GraphMemoryBuiltGraph,
  artifactInventory: GraphArtifactInventory,
  rankingSnapshots: S.Array(GraphRankingSnapshot).check(S.isMaxLength(GRAPH_MEMORY_SECTION_ITEM_LIMIT)),
  vectorRecords: S.Array(GraphArchiveVectorRecord).check(S.isMaxLength(GRAPH_MEMORY_SECTION_ITEM_LIMIT)),
  summaryRecords: S.Array(GraphArchiveSummaryRecord).check(S.isMaxLength(GRAPH_MEMORY_SECTION_ITEM_LIMIT)),
  archiveRefs: S.Array(GraphArchiveRef).check(S.isMaxLength(GRAPH_MEMORY_SECTION_ITEM_LIMIT)),
});
export interface GraphMemoryStoredGraph extends S.Schema.Type<typeof GraphMemoryStoredGraph> {}

export const GraphMemoryLifecycleFact = S.Struct({
  plane: S.Literals([
    "graph",
    "source_membership",
    "vector",
    "summary",
    "ranking",
    "archive",
    "state",
  ]),
  targetRef: boundedRef,
  actionRef: S.optionalKey(boundedRef),
  reason: boundedRef,
});
export interface GraphMemoryLifecycleFact
  extends S.Schema.Type<typeof GraphMemoryLifecycleFact> {}

export const GraphMemoryArtifactAccounting = S.Struct({
  mentions: NonNegativeInteger,
  entities: NonNegativeInteger,
  relations: NonNegativeInteger,
  merges: NonNegativeInteger,
  vectors: NonNegativeInteger,
  summaries: NonNegativeInteger,
  rankingRefs: NonNegativeInteger,
  rankingSnapshots: NonNegativeInteger,
  archives: NonNegativeInteger,
});
export interface GraphMemoryArtifactAccounting
  extends S.Schema.Type<typeof GraphMemoryArtifactAccounting> {}

export const GraphMemoryOperationReceipt = S.Struct({
  schemaId: S.Literal(GRAPH_MEMORY_RECEIPT_SCHEMA_ID),
  receiptRef: boundedRef,
  receiptDigest: GraphDigest,
  requestDigest: GraphDigest,
  operationRef: GraphMemoryOperationRef,
  operation: S.Literals(["put", "archive_export", "archive_import", "delete_source", "forget"]),
  status: S.Literals(["complete", "refused", "disabled"]),
  owner: OwnerScopeId,
  project: ProjectScopeId,
  graphDigestBefore: S.optionalKey(GraphDigest),
  graphDigestAfter: S.optionalKey(GraphDigest),
  manifestDigestBefore: S.optionalKey(GraphDigest),
  manifestDigestAfter: S.optionalKey(GraphDigest),
  sdkReceiptRef: S.optionalKey(boundedRef),
  intended: S.Array(GraphMemoryLifecycleFact).check(S.isMaxLength(GRAPH_MEMORY_LIFECYCLE_FACT_LIMIT)),
  applied: S.Array(GraphMemoryLifecycleFact).check(S.isMaxLength(GRAPH_MEMORY_LIFECYCLE_FACT_LIMIT)),
  retainedShared: S.Array(GraphMemoryLifecycleFact).check(S.isMaxLength(GRAPH_MEMORY_LIFECYCLE_FACT_LIMIT)),
  unresolved: S.Array(GraphMemoryLifecycleFact).check(S.isMaxLength(GRAPH_MEMORY_LIFECYCLE_FACT_LIMIT)),
  failed: S.Array(GraphMemoryLifecycleFact).check(S.isMaxLength(GRAPH_MEMORY_LIFECYCLE_FACT_LIMIT)),
  before: GraphMemoryArtifactAccounting,
  after: GraphMemoryArtifactAccounting,
});
export interface GraphMemoryOperationReceipt
  extends S.Schema.Type<typeof GraphMemoryOperationReceipt> {}

export const GraphMemoryArchiveExportRecord = S.Struct({
  operationRef: GraphMemoryOperationRef,
  archivalStateDigest: GraphDigest,
  generation: PositiveInteger,
  bytesBase64: S.String.check(S.isBase64()),
  archiveRef: GraphArchiveRef,
  contentDigest: GraphDigest,
  manifestDigest: GraphDigest,
  receipt: GraphMemoryOperationReceipt,
});
export interface GraphMemoryArchiveExportRecord
  extends S.Schema.Type<typeof GraphMemoryArchiveExportRecord> {}

const GraphMemoryPendingMutation = S.Struct({
  operationRef: GraphMemoryOperationRef,
  nextCurrent: S.optionalKey(GraphMemoryStoredGraph),
  archiveExport: S.optionalKey(GraphMemoryArchiveExportRecord),
  clearArchiveExports: S.optionalKey(S.Boolean),
  receipt: GraphMemoryOperationReceipt,
});

export const GraphMemoryPersistedEnvelope = S.Struct({
  schemaId: S.Literal(GRAPH_MEMORY_STATE_SCHEMA_ID),
  scope: GraphMemoryScope,
  revision: NonNegativeInteger,
  current: S.optionalKey(GraphMemoryStoredGraph),
  pending: S.optionalKey(GraphMemoryPendingMutation),
  receipts: S.Array(GraphMemoryOperationReceipt).check(S.isMaxLength(GRAPH_MEMORY_RECEIPT_LIMIT)),
  receiptHistoryDigest: S.optionalKey(GraphDigest),
  receiptHistoryCount: S.optionalKey(NonNegativeInteger),
  archiveExports: S.optionalKey(
    S.Array(GraphMemoryArchiveExportRecord).check(S.isMaxLength(GRAPH_MEMORY_RECEIPT_LIMIT)),
  ),
});
export interface GraphMemoryPersistedEnvelope
  extends S.Schema.Type<typeof GraphMemoryPersistedEnvelope> {}

export class GraphMemoryPersistenceError extends S.TaggedErrorClass<GraphMemoryPersistenceError>()(
  "GraphMemory.PersistenceError",
  {
    operation: S.String,
    reason: S.Literals(["unavailable", "invalid_state", "conflict"]),
    detailSafe: S.optionalKey(S.String.check(S.isMaxLength(512))),
  },
) {}

export class GraphMemoryStoreError extends S.TaggedErrorClass<GraphMemoryStoreError>()(
  "GraphMemory.StoreError",
  {
    operation: S.String,
    reason: S.Literals([
      "scope_violation",
      "consent_required",
      "unredacted_input",
      "policy_mismatch",
      "invalid_graph",
      "invalid_inventory",
      "invalid_ranking",
      "invalid_archive",
      "stale_generation",
      "state_conflict",
      "persistence_unavailable",
    ]),
    detailSafe: S.optionalKey(S.String.check(S.isMaxLength(512))),
  },
) {}

/**
 * The platform persistence port. A Desktop adapter can serialize and encrypt
 * the opaque envelope in SQLite. Compare-and-set must be one atomic transaction.
 */
export interface GraphMemoryStateStore {
  readonly enabled: boolean;
  readonly load: (
    scope: GraphMemoryScope,
  ) => Effect.Effect<unknown | null, GraphMemoryPersistenceError>;
  readonly compareAndSet: (
    scope: GraphMemoryScope,
    expectedRevision: number | null,
    next: unknown,
  ) => Effect.Effect<boolean, GraphMemoryPersistenceError>;
  readonly reads: Effect.Effect<number>;
  readonly writes: Effect.Effect<number>;
}

export interface PutGraphMemoryInput {
  readonly operationRef: GraphMemoryOperationRef;
  readonly binding: GraphMemoryBinding;
  readonly admission: GraphMemoryAdmission;
  readonly built: BuiltGraphCorpus;
  readonly artifactInventory: GraphArtifactInventory;
  readonly rankingSnapshots?: ReadonlyArray<GraphRankingSnapshot>;
  readonly vectorRecords?: ReadonlyArray<GraphArchiveVectorRecord>;
  readonly summaryRecords?: ReadonlyArray<GraphArchiveSummaryRecord>;
}

export interface ImportGraphMemoryArchiveInput {
  readonly operationRef: GraphMemoryOperationRef;
  readonly scope: GraphMemoryScope;
  readonly generation: number;
  readonly admission: GraphMemoryAdmission;
  readonly bytes: Uint8Array;
}

export interface ApplyGraphMemoryDeletePlanInput {
  readonly operationRef: GraphMemoryOperationRef;
  readonly scope: GraphMemoryScope;
  readonly expectedGeneration: number;
  readonly plan: GraphDeletePlan;
}

export interface GraphMemoryArchiveExport {
  readonly bytes: Uint8Array;
  readonly archiveRef: GraphArchiveRef;
  readonly contentDigest: GraphDigest;
  readonly manifestDigest: GraphDigest;
  readonly receipt: GraphMemoryOperationReceipt;
}

export interface GraphMemoryInspection {
  readonly enabled: boolean;
  readonly scope: GraphMemoryScope;
  readonly revision: number;
  readonly current: GraphMemoryStoredGraph | null;
  readonly receipts: ReadonlyArray<GraphMemoryOperationReceipt>;
  readonly pendingOperationRef: GraphMemoryOperationRef | null;
}

export interface GraphMemoryStoreInterface {
  readonly enabled: boolean;
  readonly put: (
    input: PutGraphMemoryInput,
  ) => Effect.Effect<GraphMemoryOperationReceipt, GraphMemoryStoreError>;
  readonly inspect: (
    scope: GraphMemoryScope,
  ) => Effect.Effect<GraphMemoryInspection, GraphMemoryStoreError>;
  readonly exportArchive: (
    scope: GraphMemoryScope,
    operationRef: GraphMemoryOperationRef,
  ) => Effect.Effect<GraphMemoryArchiveExport, GraphMemoryStoreError>;
  readonly importArchive: (
    input: ImportGraphMemoryArchiveInput,
  ) => Effect.Effect<GraphMemoryOperationReceipt, GraphMemoryStoreError>;
  readonly applyDeletePlan: (
    input: ApplyGraphMemoryDeletePlanInput,
  ) => Effect.Effect<GraphMemoryOperationReceipt, GraphMemoryStoreError>;
  readonly forget: (
    scope: GraphMemoryScope,
    operationRef: GraphMemoryOperationRef,
  ) => Effect.Effect<GraphMemoryOperationReceipt, GraphMemoryStoreError>;
  readonly recover: (
    scope: GraphMemoryScope,
  ) => Effect.Effect<ReadonlyArray<GraphMemoryOperationReceipt>, GraphMemoryStoreError>;
}

export class GraphMemoryStore extends Context.Service<GraphMemoryStore, GraphMemoryStoreInterface>()(
  "@openagentsinc/agent-experience-memory/GraphMemoryStore",
) {}

const compareText = (left: string, right: string): number =>
  left < right ? -1 : left > right ? 1 : 0;
const scopeKey = (scope: GraphMemoryScope): string => `${scope.owner}\u0000${scope.project}`;
const sameScope = (left: GraphMemoryScope, right: GraphMemoryScope): boolean =>
  left.owner === right.owner && left.project === right.project;
const digest = (value: unknown): GraphDigest => graphDigest(sha256Hex(canonicalJson(value)));
const requestFingerprint = (operation: GraphMemoryOperationReceipt["operation"], value: unknown) =>
  digest({ operation, value });
const bytesToBase64 = (bytes: Uint8Array): string => {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
};
const bytesFromBase64 = (value: string): Uint8Array =>
  Uint8Array.from(atob(value), (character) => character.charCodeAt(0));

/** Deterministically binds the SDK graph scope to one owner/project envelope. */
export const graphMemoryScopeRefFor = (scope: GraphMemoryScope): GraphScopeRef =>
  S.decodeUnknownSync(GraphScopeRef)(
    `graph-memory.scope.${sha256Hex(canonicalJson({
      schemaId: "openagents.graph_memory_scope.v1",
      owner: scope.owner,
      project: scope.project,
    }))}`,
  );

const emptyAccounting = (): GraphMemoryArtifactAccounting => ({
  mentions: 0,
  entities: 0,
  relations: 0,
  merges: 0,
  vectors: 0,
  summaries: 0,
  rankingRefs: 0,
  rankingSnapshots: 0,
  archives: 0,
});

const accounting = (current: GraphMemoryStoredGraph | undefined): GraphMemoryArtifactAccounting =>
  current === undefined
    ? emptyAccounting()
    : {
        mentions: current.built.snapshot.mentions.length,
        entities: current.built.snapshot.entities.length,
        relations: current.built.snapshot.relations.length,
        merges: current.built.snapshot.merges.length,
        vectors: current.artifactInventory.vectors.length,
        summaries: current.artifactInventory.summaries.length,
        rankingRefs: current.artifactInventory.rankingRefs.length,
        rankingSnapshots: current.rankingSnapshots.length,
        archives: current.archiveRefs.length,
      };

const archivalStateDigestFor = (current: GraphMemoryStoredGraph): GraphDigest =>
  digest({
    generation: current.binding.generation,
    graphDigest: current.binding.graphDigest,
    manifestDigest: current.binding.manifestDigest,
    policyDigest: current.binding.policyDigest,
    sourceBindings: current.binding.sourceBindings,
    artifactInventory: current.artifactInventory,
    rankingSnapshots: current.rankingSnapshots,
    vectorRecords: current.vectorRecords,
    summaryRecords: current.summaryRecords,
  });

const fact = (
  plane: GraphMemoryLifecycleFact["plane"],
  targetRef: string,
  reason: string,
  actionRef?: string,
): GraphMemoryLifecycleFact => ({
  plane,
  targetRef,
  reason,
  ...(actionRef === undefined ? {} : { actionRef }),
});

const deleteActionFacts = (plan: GraphDeletePlan): ReadonlyArray<GraphMemoryLifecycleFact> => [
  ...plan.actions.sourceMembershipRemovals.map((item) =>
    fact("source_membership", item.elementRef, "remove_source_membership", item.actionRef),
  ),
  ...plan.actions.removableElements.map((item) =>
    fact("graph", item.elementRef, "remove_graph_element", item.actionRef),
  ),
  ...plan.actions.entityRekeys.map((item) =>
    fact("graph", item.oldElementRef, "rekey_shared_entity", item.actionRef),
  ),
  ...plan.actions.relationRekeys.map((item) =>
    fact("graph", item.oldElementRef, "rekey_shared_relation", item.actionRef),
  ),
  ...plan.actions.removableMerges.map((item) =>
    fact("graph", item.mergeRef, "remove_merge", item.actionRef),
  ),
  ...plan.actions.mergeRekeys.map((item) =>
    fact("graph", item.oldMergeRef, "rekey_shared_merge", item.actionRef),
  ),
  ...plan.actions.vectorActions.map((item) =>
    fact("vector", item.artifactRef, item._tag === "Remove" ? "remove_artifact" : "rebuild_artifact", item.actionRef),
  ),
  ...plan.actions.summaryActions.map((item) =>
    fact("summary", item.artifactRef, item._tag === "Remove" ? "remove_artifact" : "rebuild_artifact", item.actionRef),
  ),
  ...plan.actions.rankingRefActions.map((item) =>
    fact("ranking", item.artifactRef, item._tag === "Remove" ? "remove_artifact" : "rebuild_artifact", item.actionRef),
  ),
];

const makeReceipt = (
  input: Omit<GraphMemoryOperationReceipt, "schemaId" | "receiptRef" | "receiptDigest">,
): GraphMemoryOperationReceipt => {
  const content = { schemaId: GRAPH_MEMORY_RECEIPT_SCHEMA_ID, ...input };
  const receiptDigest = digest(content);
  return S.decodeUnknownSync(GraphMemoryOperationReceipt)({
    ...content,
    receiptDigest,
    receiptRef: `graph-memory-receipt.${receiptDigest}`,
  });
};

const sourceBindingsFor = (built: BuiltGraphCorpus): ReadonlyArray<GraphMemorySourceBinding> => {
  const sources = [
    ...built.snapshot.mentions.flatMap((item) => item.memberships),
    ...built.snapshot.entities.flatMap((item) => item.memberships),
    ...built.snapshot.relations.flatMap((item) => item.memberships),
    ...built.snapshot.merges.flatMap((item) => item.memberships),
  ].map(({ source }) =>
    S.decodeUnknownSync(GraphMemorySourceBinding)({
      corpusRef: source.corpusRef,
      contentDigest: source.contentDigest,
    }),
  );
  return [...new Map(sources.map((item) => [`${item.corpusRef}\u0000${item.contentDigest}`, item])).values()]
    .sort((left, right) =>
      compareText(`${left.corpusRef}\u0000${left.contentDigest}`, `${right.corpusRef}\u0000${right.contentDigest}`),
    );
};

const policyDigestFor = (built: BuiltGraphCorpus): GraphDigest => digest(built.snapshot.policy);

const storeError = (
  operation: string,
  reason: GraphMemoryStoreError["reason"],
  detailSafe: string,
): GraphMemoryStoreError => new GraphMemoryStoreError({ operation, reason, detailSafe });

const mapPersistence = (operation: string) =>
  Effect.mapError(
    (error: GraphMemoryPersistenceError) =>
      storeError(operation, "persistence_unavailable", error.detailSafe ?? error.reason),
  );

const decodeEnvelope = (
  scope: GraphMemoryScope,
  value: unknown | null,
): Effect.Effect<GraphMemoryPersistedEnvelope | undefined, GraphMemoryStoreError> => {
  if (value === null) return Effect.map(Effect.void, (): undefined => undefined);
  const decoded = S.decodeUnknownResult(GraphMemoryPersistedEnvelope)(value);
  if (Result.isFailure(decoded) || !sameScope(decoded.success.scope, scope)) {
    return Effect.fail(storeError("load", "persistence_unavailable", "Stored graph memory is invalid or has another scope."));
  }
  return Effect.succeed(decoded.success);
};

const validateStoredGraph = Effect.fn("GraphMemory.validateStoredGraph")(function* (
  input: Omit<PutGraphMemoryInput, "operationRef">,
) {
  if (input.admission.consent !== "granted") {
    return yield* storeError("put", "consent_required", "Graph memory consent is not granted.");
  }
  if (input.admission.redactionState !== "already_redacted") {
    return yield* storeError("put", "unredacted_input", "Graph memory input is not declared already redacted.");
  }
  if (input.built.snapshot.policy.includeRedactionClasses.includes("secret")) {
    return yield* storeError("put", "policy_mismatch", "Graph memory policy cannot admit secret source content.");
  }
  const graphElementCount =
    input.built.snapshot.mentions.length +
    input.built.snapshot.entities.length +
    input.built.snapshot.relations.length +
    input.built.snapshot.merges.length;
  if (graphElementCount > GRAPH_MEMORY_GRAPH_ELEMENT_LIMIT) {
    return yield* storeError("put", "invalid_graph", "The graph exceeds the aggregate element limit.");
  }
  const sourceMembershipCount = [
    ...input.built.snapshot.mentions,
    ...input.built.snapshot.entities,
    ...input.built.snapshot.relations,
    ...input.built.snapshot.merges,
  ].reduce((count, item) => count + item.memberships.length, 0);
  if (sourceMembershipCount > GRAPH_MEMORY_SOURCE_MEMBERSHIP_LIMIT) {
    return yield* storeError("put", "invalid_graph", "The graph exceeds the aggregate source membership limit.");
  }
  const artifactCount =
    input.artifactInventory.vectors.length +
    input.artifactInventory.summaries.length +
    input.artifactInventory.rankingRefs.length;
  if (artifactCount > GRAPH_MEMORY_ARTIFACT_LIMIT) {
    return yield* storeError("put", "invalid_inventory", "The artifact inventory exceeds the aggregate limit.");
  }
  const unresolvedCount = Object.values(input.artifactInventory.coverage).reduce(
    (count, coverage) => count + (coverage._tag === "Incomplete" ? coverage.gaps.length : 0),
    0,
  );
  if (unresolvedCount > GRAPH_MEMORY_UNRESOLVED_LIMIT) {
    return yield* storeError("put", "invalid_inventory", "The inventory gaps exceed the aggregate limit.");
  }
  if ((input.rankingSnapshots?.length ?? 0) > GRAPH_MEMORY_SECTION_ITEM_LIMIT) {
    return yield* storeError("put", "invalid_ranking", "The ranking snapshots exceed the admitted limit.");
  }
  const dataBearingStrings = [
    ...input.built.snapshot.mentions.map((item) => item.identity.canonicalKey),
    ...input.built.snapshot.entities.map((item) => item.identity.canonicalKey),
    ...input.built.snapshot.relations.flatMap((item) => [item.identity.canonicalKey, item.relationKind]),
  ];
  if (dataBearingStrings.some((value) => !guardMemoryText(value).clean)) {
    return yield* storeError("put", "unredacted_input", "A graph identity value still contains redactable content.");
  }
  yield* verifyBuiltGraphCorpus(input.built).pipe(
    Effect.mapError(() => storeError("put", "invalid_graph", "The SDK graph snapshot or manifest is invalid.")),
  );
  const expectedSources = sourceBindingsFor(input.built);
  const expectedScopeRef = graphMemoryScopeRefFor({
    owner: input.binding.owner,
    project: input.binding.project,
  });
  if (
    input.binding.owner.length === 0 ||
    input.binding.project.length === 0 ||
    input.binding.graphScopeRef !== expectedScopeRef ||
    input.built.snapshot.scopeRef !== expectedScopeRef ||
    input.binding.graphRef !== input.built.snapshot.graphRef ||
    input.binding.graphDigest !== input.built.snapshot.graphDigest ||
    input.binding.manifestDigest !== input.built.manifest.manifestDigest ||
    input.binding.policyDigest !== policyDigestFor(input.built) ||
    canonicalJson(input.binding.sourceBindings) !== canonicalJson(expectedSources)
  ) {
    return yield* storeError("put", "scope_violation", "The owner, project, source, graph, manifest, or policy binding is not exact.");
  }
  for (const snapshot of input.rankingSnapshots ?? []) {
    yield* validateGraphRankingSnapshotIntegrity(snapshot).pipe(
      Effect.mapError(() => storeError("put", "invalid_ranking", "A ranking snapshot is invalid.")),
    );
    if (
      snapshot.graphRef !== input.built.snapshot.graphRef ||
      snapshot.scopeRef !== input.built.snapshot.scopeRef ||
      snapshot.graphDigest !== input.built.snapshot.graphDigest ||
      snapshot.manifestDigest !== input.built.manifest.manifestDigest
    ) {
      return yield* storeError("put", "invalid_ranking", "A ranking snapshot has another graph identity.");
    }
  }
  for (const record of input.summaryRecords ?? []) {
    const text = yield* Effect.try({
      try: () => new TextDecoder("utf-8", { fatal: true }).decode(bytesFromBase64(record.payloadBase64)),
      catch: () => storeError("put", "unredacted_input", "A summary payload is not valid UTF-8."),
    });
    if (!guardMemoryText(text).clean) {
      return yield* storeError("put", "unredacted_input", "A summary payload contains redactable content.");
    }
  }
  const archiveCheck = yield* Effect.result(
    encodeGraphCorpusArchive({
      built: input.built,
      capabilities: makeGraphAdapterCapabilities(["snapshot_export"]),
      artifactInventory: input.artifactInventory,
      vectorRecords: input.vectorRecords ?? [],
      summaryRecords: input.summaryRecords ?? [],
      rankingSnapshots: input.rankingSnapshots ?? [],
    }),
  );
  if (Result.isFailure(archiveCheck)) {
    return yield* storeError("put", "invalid_inventory", "The SDK rejected the artifact inventory or archive records.");
  }
  return S.decodeUnknownSync(GraphMemoryStoredGraph)({
    schemaId: GRAPH_MEMORY_STORE_SCHEMA_ID,
    binding: input.binding,
    admission: input.admission,
    built: input.built,
    artifactInventory: input.artifactInventory,
    rankingSnapshots: input.rankingSnapshots ?? [],
    vectorRecords: input.vectorRecords ?? [],
    summaryRecords: input.summaryRecords ?? [],
    archiveRefs: [],
  }) as GraphMemoryStoredGraph;
});

const receiptForDisabled = (
  operation: GraphMemoryOperationReceipt["operation"],
  scope: GraphMemoryScope,
  operationRef: GraphMemoryOperationRef,
): GraphMemoryOperationReceipt =>
  makeReceipt({
    requestDigest: requestFingerprint(operation, scope),
    operationRef,
    operation,
    status: "disabled",
    owner: scope.owner,
    project: scope.project,
    intended: [],
    applied: [],
    retainedShared: [],
    unresolved: [],
    failed: [fact("state", operationRef, "memory_disabled")],
    before: emptyAccounting(),
    after: emptyAccounting(),
  });

const disabledInterface = (): GraphMemoryStoreInterface => ({
  enabled: false,
  put: (input) => Effect.succeed(receiptForDisabled("put", input.binding, input.operationRef)),
  inspect: (scope) =>
    Effect.succeed({ enabled: false, scope, revision: 0, current: null, receipts: [], pendingOperationRef: null }),
  exportArchive: (scope, operationRef) =>
    Effect.fail(storeError("exportArchive", "persistence_unavailable", `Memory is disabled (${operationRef}).`)),
  importArchive: (input) => Effect.succeed(receiptForDisabled("archive_import", input.scope, input.operationRef)),
  applyDeletePlan: (input) => Effect.succeed(receiptForDisabled("delete_source", input.scope, input.operationRef)),
  forget: (scope, operationRef) => Effect.succeed(receiptForDisabled("forget", scope, operationRef)),
  recover: () => Effect.succeed([]),
});

export const makeGraphMemoryStore = Effect.fn("GraphMemory.makeStore")(function* (
  stateStore: GraphMemoryStateStore,
) {
  if (!stateStore.enabled) return disabledInterface();

  const load = Effect.fn("GraphMemory.load")(function* (scope: GraphMemoryScope) {
    const raw = yield* stateStore.load(scope).pipe(mapPersistence("load"));
    return yield* decodeEnvelope(scope, raw);
  });

  const finalizePending = Effect.fn("GraphMemory.finalizePending")(function* (
    envelope: GraphMemoryPersistedEnvelope,
  ) {
    if (envelope.pending === undefined) return envelope;
    const next = S.decodeUnknownSync(GraphMemoryPersistedEnvelope)({
      schemaId: GRAPH_MEMORY_STATE_SCHEMA_ID,
      scope: envelope.scope,
      revision: envelope.revision + 1,
      ...(envelope.pending.nextCurrent === undefined ? {} : { current: envelope.pending.nextCurrent }),
      receipts: [...envelope.receipts, envelope.pending.receipt],
      ...(envelope.receiptHistoryDigest === undefined
        ? {}
        : { receiptHistoryDigest: envelope.receiptHistoryDigest }),
      ...(envelope.receiptHistoryCount === undefined
        ? {}
        : { receiptHistoryCount: envelope.receiptHistoryCount }),
      archiveExports: [
        ...(envelope.pending.clearArchiveExports === true ? [] : (envelope.archiveExports ?? [])),
        ...(envelope.pending.archiveExport === undefined ? [] : [envelope.pending.archiveExport]),
      ],
    });
    const committed = yield* stateStore
      .compareAndSet(envelope.scope, envelope.revision, next)
      .pipe(mapPersistence("recover"));
    return committed ? next : yield* storeError("recover", "state_conflict", "The pending graph mutation changed concurrently.");
  });

  const recover = Effect.fn("GraphMemory.recover")(function* (scope: GraphMemoryScope) {
    for (let attempt = 0; attempt < GRAPH_MEMORY_CAS_ATTEMPT_LIMIT; attempt += 1) {
      const envelope = yield* load(scope);
      if (envelope === undefined || envelope.pending === undefined) return [];
      const receipt = envelope.pending.receipt;
      const completed = yield* Effect.result(finalizePending(envelope));
      if (Result.isSuccess(completed)) return [receipt];
    }
    return yield* storeError("recover", "state_conflict", "Pending graph mutation recovery exceeded its retry limit.");
  });

  const mutate = Effect.fn("GraphMemory.mutate")(function* (
    scope: GraphMemoryScope,
    operationRef: GraphMemoryOperationRef,
    requestDigest: GraphDigest,
    compute: (
      current: GraphMemoryStoredGraph | undefined,
      archiveExports: ReadonlyArray<GraphMemoryArchiveExportRecord>,
    ) => Effect.Effect<Readonly<{
      nextCurrent?: GraphMemoryStoredGraph;
      archiveExport?: GraphMemoryArchiveExportRecord;
      clearArchiveExports?: boolean;
      receipt: GraphMemoryOperationReceipt;
    }>, GraphMemoryStoreError>,
  ) {
    yield* recover(scope);
    for (let attempt = 0; attempt < GRAPH_MEMORY_CAS_ATTEMPT_LIMIT; attempt += 1) {
      const envelope = yield* load(scope);
      const prior = envelope?.receipts.find((receipt) => receipt.operationRef === operationRef);
      if (prior !== undefined) {
        if (prior.requestDigest !== requestDigest) {
          return yield* storeError("mutate", "state_conflict", "The operation reference is bound to another request.");
        }
        return prior;
      }
      if (envelope?.pending !== undefined) {
        yield* recover(scope);
        continue;
      }
      const computed = yield* compute(envelope?.current, envelope?.archiveExports ?? []);
      let receipts = envelope?.receipts ?? [];
      let receiptHistoryDigest = envelope?.receiptHistoryDigest;
      let receiptHistoryCount = envelope?.receiptHistoryCount ?? 0;
      if (receipts.length >= GRAPH_MEMORY_RECEIPT_LIMIT) {
        if (computed.receipt.operation !== "forget") {
          return yield* storeError("mutate", "state_conflict", "The graph memory receipt limit is reached.");
        }
        receiptHistoryDigest = digest({
          priorDigest: receiptHistoryDigest,
          priorCount: receiptHistoryCount,
          receipts,
        });
        receiptHistoryCount += receipts.length;
        receipts = [];
      }
      const prepared = S.decodeUnknownSync(GraphMemoryPersistedEnvelope)({
        schemaId: GRAPH_MEMORY_STATE_SCHEMA_ID,
        scope,
        revision: (envelope?.revision ?? -1) + 1,
        ...(envelope?.current === undefined ? {} : { current: envelope.current }),
        pending: {
          operationRef,
          ...(computed.nextCurrent === undefined ? {} : { nextCurrent: computed.nextCurrent }),
          ...(computed.archiveExport === undefined ? {} : { archiveExport: computed.archiveExport }),
          ...(computed.clearArchiveExports === undefined
            ? {}
            : { clearArchiveExports: computed.clearArchiveExports }),
          receipt: computed.receipt,
        },
        receipts,
        ...(receiptHistoryDigest === undefined ? {} : { receiptHistoryDigest }),
        ...(receiptHistoryCount === 0 ? {} : { receiptHistoryCount }),
        archiveExports: envelope?.archiveExports ?? [],
      });
      const staged = yield* stateStore
        .compareAndSet(scope, envelope?.revision ?? null, prepared)
        .pipe(mapPersistence("prepare"));
      if (!staged) continue;
      const finalized = yield* Effect.result(finalizePending(prepared));
      if (Result.isSuccess(finalized)) return computed.receipt;
      return yield* finalized.failure;
    }
    return yield* storeError("mutate", "state_conflict", "Graph memory mutation exceeded its retry limit.");
  });

  const put = Effect.fn("GraphMemory.put")(function* (input: PutGraphMemoryInput) {
    const scope: GraphMemoryScope = { owner: input.binding.owner, project: input.binding.project };
    const putRequestDigest = requestFingerprint("put", input);
    const stored = yield* validateStoredGraph(input);
    return yield* mutate(scope, input.operationRef, putRequestDigest, (current) => {
      if (current !== undefined && input.binding.generation <= current.binding.generation) {
        return Effect.fail(storeError("put", "stale_generation", "Graph generation must increase."));
      }
      const before = accounting(current);
      const after = accounting(stored);
      return Effect.succeed({
        nextCurrent: stored,
        receipt: makeReceipt({
          requestDigest: putRequestDigest,
          operationRef: input.operationRef,
          operation: "put",
          status: "complete",
          owner: scope.owner,
          project: scope.project,
          ...(current === undefined ? {} : {
            graphDigestBefore: current.binding.graphDigest,
            manifestDigestBefore: current.binding.manifestDigest,
          }),
          graphDigestAfter: stored.binding.graphDigest,
          manifestDigestAfter: stored.binding.manifestDigest,
          intended: [fact("graph", stored.binding.graphRef, "store_exact_graph")],
          applied: [fact("graph", stored.binding.graphRef, "stored_exact_graph")],
          retainedShared: [],
          unresolved: [],
          failed: [],
          before,
          after,
        }),
      });
    });
  });

  const inspect = Effect.fn("GraphMemory.inspect")(function* (scope: GraphMemoryScope) {
    yield* recover(scope);
    const envelope = yield* load(scope);
    return {
      enabled: true,
      scope,
      revision: envelope?.revision ?? 0,
      current: envelope?.current ?? null,
      receipts: envelope?.receipts ?? [],
      pendingOperationRef: envelope?.pending?.operationRef ?? null,
    } satisfies GraphMemoryInspection;
  });

  const exportArchive = Effect.fn("GraphMemory.exportArchive")(function* (
    scope: GraphMemoryScope,
    operationRef: GraphMemoryOperationRef,
  ) {
    yield* recover(scope);
    const envelope = yield* load(scope);
    const priorExport = envelope?.archiveExports?.find((item) => item.operationRef === operationRef);
    if (priorExport !== undefined) {
      return {
        bytes: bytesFromBase64(priorExport.bytesBase64),
        archiveRef: priorExport.archiveRef,
        contentDigest: priorExport.contentDigest,
        manifestDigest: priorExport.manifestDigest,
        receipt: priorExport.receipt,
      } satisfies GraphMemoryArchiveExport;
    }
    if (envelope?.receipts.some((item) => item.operationRef === operationRef)) {
      return yield* storeError(
        "exportArchive",
        "state_conflict",
        "The prior export replay payload is no longer available.",
      );
    }
    const current = envelope?.current;
    if (current === undefined) {
      return yield* storeError("exportArchive", "invalid_archive", "No graph exists in this scope.");
    }
    const imported = yield* encodeGraphCorpusArchive({
      built: current.built,
      capabilities: makeGraphAdapterCapabilities(["snapshot_export"]),
      artifactInventory: current.artifactInventory,
      vectorRecords: current.vectorRecords,
      summaryRecords: current.summaryRecords,
      rankingSnapshots: current.rankingSnapshots,
    }).pipe(
      Effect.mapError(() => storeError("exportArchive", "invalid_archive", "The SDK refused graph archive export.")),
    );
    const verified = yield* importGraphCorpusArchive(imported).pipe(
      Effect.mapError(() => storeError("exportArchive", "invalid_archive", "The exported archive did not round-trip.")),
    );
    const archiveRef = verified.archive.manifest.archiveRef;
    const archivalStateDigest = archivalStateDigestFor(current);
    const exportRequestDigest = requestFingerprint("archive_export", scope);
    yield* mutate(scope, operationRef, exportRequestDigest, (latest) => {
      if (latest === undefined || archivalStateDigestFor(latest) !== archivalStateDigest) {
        return Effect.fail(storeError("exportArchive", "stale_generation", "The archival state changed during export."));
      }
      const next = S.decodeUnknownSync(GraphMemoryStoredGraph)({
        ...latest,
        archiveRefs: [...new Set([...latest.archiveRefs, archiveRef])].sort(compareText),
      }) as GraphMemoryStoredGraph;
      const receipt = makeReceipt({
          requestDigest: exportRequestDigest,
          operationRef,
          operation: "archive_export",
          status: "complete",
          owner: scope.owner,
          project: scope.project,
          graphDigestBefore: latest.binding.graphDigest,
          graphDigestAfter: latest.binding.graphDigest,
          manifestDigestBefore: latest.binding.manifestDigest,
          manifestDigestAfter: latest.binding.manifestDigest,
          intended: [fact("archive", archiveRef, "export_versioned_archive")],
          applied: [fact("archive", archiveRef, "archive_digest_round_trip_verified")],
          retainedShared: [],
          unresolved: [],
          failed: [],
          before: accounting(latest),
          after: accounting(next),
        });
      const archiveExport = S.decodeUnknownSync(GraphMemoryArchiveExportRecord)({
        operationRef,
        archivalStateDigest,
        generation: latest.binding.generation,
        bytesBase64: bytesToBase64(imported),
        archiveRef,
        contentDigest: verified.archive.manifest.contentDigest,
        manifestDigest: verified.archive.manifest.manifestDigest,
        receipt,
      });
      return Effect.succeed({
        nextCurrent: next,
        archiveExport,
        receipt,
      });
    });
    const committed = yield* load(scope);
    const exact = committed?.archiveExports?.find((item) => item.operationRef === operationRef);
    if (exact === undefined) {
      return yield* storeError("exportArchive", "state_conflict", "The export result is not committed.");
    }
    return {
      bytes: bytesFromBase64(exact.bytesBase64),
      archiveRef: exact.archiveRef,
      contentDigest: exact.contentDigest,
      manifestDigest: exact.manifestDigest,
      receipt: exact.receipt,
    } satisfies GraphMemoryArchiveExport;
  });

  const importArchive = Effect.fn("GraphMemory.importArchive")(function* (
    input: ImportGraphMemoryArchiveInput,
  ) {
    const importRequestDigest = requestFingerprint("archive_import", {
      ...input,
      bytes: bytesToBase64(input.bytes),
    });
    if (input.admission.consent !== "granted" || input.admission.redactionState !== "already_redacted") {
      return yield* storeError("importArchive", "consent_required", "Archive import requires consent and an already-redacted declaration.");
    }
    const imported = yield* importGraphCorpusArchive(input.bytes).pipe(
      Effect.mapError(() => storeError("importArchive", "invalid_archive", "The SDK refused graph archive import.")),
    );
    if (imported.archive.sections.contentExtension !== undefined) {
      return yield* storeError("importArchive", "unredacted_input", "Portable graph memory does not import content extensions.");
    }
    const inventory = imported.artifactInventory ?? makeGraphArtifactInventory({
      built: imported.built,
      vectors: [],
      summaries: [],
      rankingRefs: [],
      coverage: {
        vectors: { _tag: "Complete" },
        summaries: { _tag: "Complete" },
        rankingRefs: { _tag: "Complete" },
      },
    });
    const binding: GraphMemoryBinding = {
      owner: input.scope.owner,
      project: input.scope.project,
      graphScopeRef: imported.built.snapshot.scopeRef,
      sourceBindings: sourceBindingsFor(imported.built),
      graphRef: imported.built.snapshot.graphRef,
      graphDigest: imported.built.snapshot.graphDigest,
      manifestDigest: imported.built.manifest.manifestDigest,
      policyDigest: policyDigestFor(imported.built),
      generation: input.generation,
    };
    const stored = yield* validateStoredGraph({
      binding,
      admission: input.admission,
      built: imported.built,
      artifactInventory: inventory,
      rankingSnapshots: imported.rankingSnapshots,
      vectorRecords: imported.archive.sections.vectors?.records ?? [],
      summaryRecords: imported.archive.sections.summaries?.records ?? [],
    });
    const withArchive = S.decodeUnknownSync(GraphMemoryStoredGraph)({
      ...stored,
      archiveRefs: [imported.archive.manifest.archiveRef],
    }) as GraphMemoryStoredGraph;
    return yield* mutate(input.scope, input.operationRef, importRequestDigest, (current) => {
      if (current !== undefined && input.generation <= current.binding.generation) {
        return Effect.fail(storeError("importArchive", "stale_generation", "Imported graph generation must increase."));
      }
      return Effect.succeed({
        nextCurrent: withArchive,
        receipt: makeReceipt({
          requestDigest: importRequestDigest,
          operationRef: input.operationRef,
          operation: "archive_import",
          status: "complete",
          owner: input.scope.owner,
          project: input.scope.project,
          ...(current === undefined ? {} : {
            graphDigestBefore: current.binding.graphDigest,
            manifestDigestBefore: current.binding.manifestDigest,
          }),
          graphDigestAfter: withArchive.binding.graphDigest,
          manifestDigestAfter: withArchive.binding.manifestDigest,
          intended: [fact("archive", imported.archive.manifest.archiveRef, "import_versioned_archive")],
          applied: [fact("graph", withArchive.binding.graphRef, "archive_digests_verified_and_imported")],
          retainedShared: [],
          unresolved: [],
          failed: [],
          before: accounting(current),
          after: accounting(withArchive),
        }),
      });
    });
  });

  const projectAfterDelete = Effect.fn("GraphMemory.projectAfterDelete")(function* (
    plan: GraphCompleteDeletePlan,
    current: GraphMemoryStoredGraph,
  ) {
    const before = current.built;
    const removedElements = new Set(plan.actions.removableElements.map((item) => item.elementRef));
    const entityRekeys = new Map(plan.actions.entityRekeys.map((item) => [item.oldElementRef, item]));
    const relationRekeys = new Map(plan.actions.relationRekeys.map((item) => [item.oldElementRef, item]));
    const removedMerges = new Set(plan.actions.removableMerges.map((item) => item.mergeRef));
    const mergeRekeys = new Map(plan.actions.mergeRekeys.map((item) => [item.oldMergeRef, item]));
    const after = yield* buildGraphCorpus({
      graphRef: before.snapshot.graphRef,
      scopeRef: before.snapshot.scopeRef,
      policy: before.snapshot.policy,
      mentions: before.snapshot.mentions.filter((item) => !removedElements.has(item.elementRef)),
      entities: before.snapshot.entities
        .filter((item) => !removedElements.has(item.elementRef))
        .map((item) => {
          const action = entityRekeys.get(item.elementRef);
          return action === undefined ? item : {
            ...item,
            elementRef: action.newElementRef,
            entityRef: action.newEntityRef,
            mentionRefs: action.retainedMentionRefs,
            memberships: action.retainedMemberships,
          };
        }),
      relations: before.snapshot.relations
        .filter((item) => !removedElements.has(item.elementRef))
        .map((item) => {
          const action = relationRekeys.get(item.elementRef);
          return action === undefined ? item : {
            ...item,
            elementRef: action.newElementRef,
            relationRef: action.newRelationRef,
            fromEntityRef: action.newFromEntityRef,
            toEntityRef: action.newToEntityRef,
            memberships: action.retainedMemberships,
          };
        }),
      merges: before.snapshot.merges
        .filter((item) => !removedMerges.has(item.mergeRef))
        .map((item) => {
          const action = mergeRekeys.get(item.mergeRef);
          return action === undefined ? item : {
            ...item,
            mergeRef: action.newMergeRef,
            entityRef: action.newEntityRef,
            mentionRefs: action.retainedMentionRefs,
            memberships: action.retainedMemberships,
          };
        }),
      embeddingProjections: before.snapshot.embeddingProjections,
    }).pipe(Effect.mapError(() => storeError("applyDeletePlan", "invalid_graph", "The SDK delete after-state is invalid.")));
    if (current.artifactInventory._tag !== "Complete") {
      return yield* storeError("applyDeletePlan", "invalid_inventory", "Exact deletion requires a complete artifact inventory.");
    }
    const removeArtifacts = <A extends { readonly artifactRef: string }>(
      artifacts: ReadonlyArray<A>,
      actions: ReadonlyArray<GraphDerivedArtifactAction>,
    ): ReadonlyArray<A> => {
      const targeted = new Set<string>(actions.map((action) => action.artifactRef));
      return artifacts.filter((artifact) => !targeted.has(artifact.artifactRef));
    };
    const afterInventory = makeGraphArtifactInventory({
      built: after,
      vectors: removeArtifacts(current.artifactInventory.vectors, plan.actions.vectorActions),
      summaries: removeArtifacts(current.artifactInventory.summaries, plan.actions.summaryActions),
      rankingRefs: removeArtifacts(current.artifactInventory.rankingRefs, plan.actions.rankingRefActions),
      coverage: {
        vectors: { _tag: "Complete" },
        summaries: { _tag: "Complete" },
        rankingRefs: { _tag: "Complete" },
      },
    });
    const result = yield* makeCompleteGraphDeleteExecutionResult(plan, {
      before,
      beforeInventory: current.artifactInventory,
      after,
      afterInventory,
    }).pipe(Effect.mapError(() => storeError("applyDeletePlan", "invalid_inventory", "The SDK refused the delete execution result.")));
    const sdkReceipt = yield* makeGraphDeleteReceipt(plan, result, {
      before,
      beforeInventory: current.artifactInventory,
      after,
      afterInventory,
    }).pipe(Effect.mapError(() => storeError("applyDeletePlan", "invalid_inventory", "The SDK refused the delete receipt.")));
    const vectorTargets = new Set(plan.actions.vectorActions.map((action) => action.artifactRef));
    const summaryTargets = new Set(plan.actions.summaryActions.map((action) => action.artifactRef));
    const next: GraphMemoryStoredGraph = {
      ...current,
      binding: {
        ...current.binding,
        sourceBindings: sourceBindingsFor(after),
        graphDigest: after.snapshot.graphDigest,
        manifestDigest: after.manifest.manifestDigest,
        policyDigest: policyDigestFor(after),
        generation: current.binding.generation + 1,
      },
      built: after,
      artifactInventory: afterInventory,
      rankingSnapshots: current.rankingSnapshots.filter(
        (snapshot) => snapshot.graphDigest === after.snapshot.graphDigest,
      ),
      vectorRecords: current.vectorRecords.filter((record) => !vectorTargets.has(record.artifact.artifactRef)),
      summaryRecords: current.summaryRecords.filter((record) => !summaryTargets.has(record.artifact.artifactRef)),
      archiveRefs: [],
    };
    return { next, sdkReceipt } as const;
  });

  const applyDeletePlan = Effect.fn("GraphMemory.applyDeletePlan")(function* (
    input: ApplyGraphMemoryDeletePlanInput,
  ) {
    const actionCount =
      input.plan.actions.sourceMembershipRemovals.length +
      input.plan.actions.removableElements.length +
      input.plan.actions.entityRekeys.length +
      input.plan.actions.relationRekeys.length +
      input.plan.actions.removableMerges.length +
      input.plan.actions.mergeRekeys.length +
      input.plan.actions.vectorActions.length +
      input.plan.actions.summaryActions.length +
      input.plan.actions.rankingRefActions.length;
    const unresolvedCount = input.plan._tag === "Incomplete" ? input.plan.unresolved.length : 0;
    if (actionCount + unresolvedCount > GRAPH_MEMORY_LIFECYCLE_FACT_LIMIT) {
      return yield* storeError(
        "applyDeletePlan",
        "invalid_inventory",
        "The delete plan exceeds the lifecycle fact limit.",
      );
    }
    const deleteRequestDigest = requestFingerprint("delete_source", input);
    return yield* mutate(input.scope, input.operationRef, deleteRequestDigest, (current, archiveExports) =>
      Effect.gen(function* () {
        if (current === undefined) {
          return {
            receipt: makeReceipt({
              requestDigest: deleteRequestDigest,
              operationRef: input.operationRef,
              operation: "delete_source",
              status: "refused",
              owner: input.scope.owner,
              project: input.scope.project,
              intended: [],
              applied: [],
              retainedShared: [],
              unresolved: [],
              failed: [fact("state", input.plan.idempotencyKey, "graph_not_found")],
              before: emptyAccounting(),
              after: emptyAccounting(),
            }),
          };
        }
        if (current.binding.generation !== input.expectedGeneration) {
          return {
            nextCurrent: current,
            receipt: makeReceipt({
              requestDigest: deleteRequestDigest,
              operationRef: input.operationRef,
              operation: "delete_source",
              status: "refused",
              owner: input.scope.owner,
              project: input.scope.project,
              graphDigestBefore: current.binding.graphDigest,
              graphDigestAfter: current.binding.graphDigest,
              manifestDigestBefore: current.binding.manifestDigest,
              manifestDigestAfter: current.binding.manifestDigest,
              intended: deleteActionFacts(input.plan),
              applied: [],
              retainedShared: [],
              unresolved: [],
              failed: [fact("state", input.plan.idempotencyKey, "stale_generation")],
              before: accounting(current),
              after: accounting(current),
            }),
          };
        }
        const executable = yield* Effect.result(
          requireExecutableGraphDeletePlan(input.plan, current.built, current.artifactInventory),
        );
        if (Result.isFailure(executable)) {
          const unresolved = input.plan._tag === "Incomplete"
            ? input.plan.unresolved.map((item) =>
                fact(
                  item.targetKind === "ranking_ref"
                    ? "ranking"
                    : item.targetKind === "vector"
                      ? "vector"
                      : item.targetKind === "summary"
                        ? "summary"
                        : "graph",
                  item.targetRef ?? item.unresolvedRef,
                  item.reason,
                  item.unresolvedRef,
                ),
              )
            : [];
          return {
            nextCurrent: current,
            receipt: makeReceipt({
              requestDigest: deleteRequestDigest,
              operationRef: input.operationRef,
              operation: "delete_source",
              status: "refused",
              owner: input.scope.owner,
              project: input.scope.project,
              graphDigestBefore: current.binding.graphDigest,
              graphDigestAfter: current.binding.graphDigest,
              manifestDigestBefore: current.binding.manifestDigest,
              manifestDigestAfter: current.binding.manifestDigest,
              intended: deleteActionFacts(input.plan),
              applied: [],
              retainedShared: [],
              unresolved,
              failed: [fact("state", input.plan.idempotencyKey, executable.failure.reason)],
              before: accounting(current),
              after: accounting(current),
            }),
          };
        }
        const { next, sdkReceipt } = yield* projectAfterDelete(executable.success, current);
        const removed = new Set<string>([
          ...executable.success.actions.removableElements.map((item) => item.elementRef),
          ...executable.success.actions.removableMerges.map((item) => item.mergeRef),
        ]);
        const retainedShared = executable.success.actions.sourceMembershipRemovals
          .filter((item) => !removed.has(item.elementRef))
          .map((item) => fact("source_membership", item.elementRef, "retained_shared_element", item.actionRef));
        const retainedSnapshotRefs = new Set(next.rankingSnapshots.map((item) => item.snapshotRef));
        const droppedSnapshotFacts = current.rankingSnapshots
          .filter((snapshot) => !retainedSnapshotRefs.has(snapshot.snapshotRef))
          .map((snapshot) => {
            const featureRefs = new Set(snapshot.features.map((item) => item.featureRef));
            const action = executable.success.actions.rankingRefActions.find((item) =>
              featureRefs.has(item.artifactRef),
            );
            return fact(
              "ranking",
              snapshot.snapshotRef,
              "remove_stale_ranking_snapshot",
              action?.actionRef ?? executable.success.idempotencyKey,
            );
          });
        const replayPurgeFacts = archiveExports.map((item) =>
          fact("archive", item.operationRef, "delete_internal_archive_replay", item.receipt.receiptRef),
        );
        const externalExportFacts = [...new Set([
          ...current.archiveRefs,
          ...archiveExports.map((item) => item.archiveRef),
        ])].sort(compareText).map((archiveRef) =>
          fact("archive", archiveRef, "owner_export_payload_not_stored"),
        );
        const actionFacts = [
          ...deleteActionFacts(executable.success),
          ...droppedSnapshotFacts,
          ...replayPurgeFacts,
        ];
        return {
          nextCurrent: next,
          clearArchiveExports: true,
          receipt: makeReceipt({
            requestDigest: deleteRequestDigest,
            operationRef: input.operationRef,
            operation: "delete_source",
            status: "complete",
            owner: input.scope.owner,
            project: input.scope.project,
            graphDigestBefore: current.binding.graphDigest,
            graphDigestAfter: next.binding.graphDigest,
            manifestDigestBefore: current.binding.manifestDigest,
            manifestDigestAfter: next.binding.manifestDigest,
            sdkReceiptRef: sdkReceipt.receiptRef,
            intended: actionFacts,
            applied: actionFacts,
            retainedShared: [...retainedShared, ...externalExportFacts],
            unresolved: [],
            failed: [],
            before: accounting(current),
            after: accounting(next),
          }),
        };
      }),
    );
  });

  const forget = Effect.fn("GraphMemory.forget")(function* (
    scope: GraphMemoryScope,
    operationRef: GraphMemoryOperationRef,
  ) {
    const forgetRequestDigest = requestFingerprint("forget", scope);
    return yield* mutate(scope, operationRef, forgetRequestDigest, (current, archiveExports) => {
      const before = accounting(current);
      const replayPurgeFacts = archiveExports.map((item) =>
        fact("archive", item.operationRef, "delete_internal_archive_replay", item.receipt.receiptRef),
      );
      const intended = current === undefined
        ? replayPurgeFacts
        : [
            fact("graph", current.binding.graphRef, "forget_graph"),
            ...current.artifactInventory.vectors.map((item) => fact("vector", item.artifactRef, "forget_vector")),
            ...current.artifactInventory.summaries.map((item) => fact("summary", item.artifactRef, "forget_summary")),
            ...current.artifactInventory.rankingRefs.map((item) => fact("ranking", item.artifactRef, "forget_ranking_ref")),
            ...current.rankingSnapshots.map((item) => fact("ranking", item.snapshotRef, "forget_ranking_snapshot")),
            ...current.archiveRefs.map((item) => fact("archive", item, "forget_archive_accounting")),
            ...replayPurgeFacts,
          ];
      const retainedOwnerExports = [...new Set([
        ...(current?.archiveRefs ?? []),
        ...archiveExports.map((item) => item.archiveRef),
      ])].sort(compareText).map((archiveRef) =>
        fact("archive", archiveRef, "owner_export_payload_not_stored"),
      );
      const applied = intended.filter((item) =>
        item.plane !== "archive" || item.reason === "delete_internal_archive_replay",
      );
      return Effect.succeed({
        clearArchiveExports: true,
        receipt: makeReceipt({
          requestDigest: forgetRequestDigest,
          operationRef,
          operation: "forget",
          status: "complete",
          owner: scope.owner,
          project: scope.project,
          ...(current === undefined ? {} : {
            graphDigestBefore: current.binding.graphDigest,
            manifestDigestBefore: current.binding.manifestDigest,
          }),
          intended,
          applied,
          retainedShared: retainedOwnerExports,
          unresolved: [],
          failed: [],
          before,
          after: emptyAccounting(),
        }),
      });
    });
  });

  return GraphMemoryStore.of({
    enabled: true,
    put,
    inspect,
    exportArchive,
    importArchive,
    applyDeletePlan,
    forget,
    recover,
  });
});

export const graphMemoryStoreLayer = (stateStore: GraphMemoryStateStore) =>
  Layer.effect(GraphMemoryStore, makeGraphMemoryStore(stateStore));

export const disabledGraphMemoryStoreLayer = Layer.succeed(
  GraphMemoryStore,
  GraphMemoryStore.of(disabledInterface()),
);

/** A counted, atomic in-memory driver for conformance and app-adapter tests. */
export const makeInMemoryGraphMemoryStateStore = Effect.fn(
  "GraphMemory.makeInMemoryStateStore",
)(function* () {
  const rows = yield* Ref.make(new Map<string, unknown>());
  const readCount = yield* Ref.make(0);
  const writeCount = yield* Ref.make(0);
  const load: GraphMemoryStateStore["load"] = (scope) =>
    Ref.update(readCount, (count) => count + 1).pipe(
      Effect.flatMap(() => Ref.get(rows)),
      Effect.map((state) => structuredClone(state.get(scopeKey(scope)) ?? null)),
    );
  const compareAndSet: GraphMemoryStateStore["compareAndSet"] = (scope, expectedRevision, next) =>
    Ref.update(writeCount, (count) => count + 1).pipe(
      Effect.flatMap(() =>
        Ref.modify(rows, (state) => {
          const existing = state.get(scopeKey(scope));
          const decoded = existing === undefined
            ? undefined
            : S.decodeUnknownResult(GraphMemoryPersistedEnvelope)(existing);
          const revision = decoded === undefined || Result.isFailure(decoded)
            ? null
            : decoded.success.revision;
          if (revision !== expectedRevision) return [false, state] as const;
          const updated = new Map(state);
          updated.set(scopeKey(scope), structuredClone(next));
          return [true, updated] as const;
        }),
      ),
    );
  return {
    enabled: true,
    load,
    compareAndSet,
    reads: Ref.get(readCount),
    writes: Ref.get(writeCount),
  } satisfies GraphMemoryStateStore;
});
