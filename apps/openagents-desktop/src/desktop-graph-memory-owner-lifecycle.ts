import { createHash } from "node:crypto";
import { existsSync, unlinkSync, writeFileSync } from "node:fs";

import {
  GraphMemoryBinding,
  GraphMemoryOperationRef,
  GraphMemoryStore,
  graphMemoryScopeRefFor,
  ownerScopeId,
  projectScopeId,
  type GraphMemoryArtifactAccounting,
  type GraphMemoryScope,
  type GraphMemoryStoredGraph,
  type GraphMemoryStoreInterface,
  type GraphMemoryStoreError,
} from "@openagentsinc/agent-experience-memory";
import {
  buildGraphCorpus,
  canonicalJson,
  graphDigest,
  makeCanonicalEntity,
  makeEmbeddingProjectionDescriptor,
  makeGraphAdapterCapabilities,
  makeInMemoryGraphSnapshotHandle,
  makeGraphMention,
  makeGraphRelation,
  sha256Hex,
} from "@openagentsinc/graph-corpus";
import {
  GraphArchiveSummaryRecord,
  GraphArchiveVectorRecord,
  importGraphCorpusArchive,
} from "@openagentsinc/graph-corpus/archive";
import {
  makeGraphArtifactInventory,
  planGraphSourceDeletion,
} from "@openagentsinc/graph-corpus/deletion";
import {
  makeGraphFeedbackObservation,
  makeGraphRankingSnapshot,
  rankingArtifactsFromSnapshot,
} from "@openagentsinc/graph-corpus/ranking";
import {
  makeGraphRlmClassificationProjection,
  makeGraphRlmProjection,
} from "@openagentsinc/graph-corpus/rlm";
import { GraphDerivation, GraphSourceMembership } from "@openagentsinc/graph-corpus/schemas";
import { buildInlineCorpusInput, makeInlineCorpusHandle } from "@openagentsinc/rlm";
import { Effect, Schema as S } from "effect";

import { openDesktopGraphMemoryStore } from "./desktop-graph-memory-store.js";
import type { SafeStorageLike } from "./desktop-session-vault.js";

export const GRAPH_MEMORY_OWNER_LIFECYCLE_PROOF_SCHEMA_ID =
  "openagents.desktop.graph_memory_owner_lifecycle_proof.v1" as const;

export type GraphMemoryOwnerLifecycleCustodyRung =
  | "test_fake_safe_storage"
  | "standalone_proof_process_wrapping_key"
  | "electron_safe_storage";

export interface GraphMemoryOwnerLifecycleProofInput {
  readonly databasePath: string;
  readonly archivePath: string;
  readonly safeStorage: SafeStorageLike;
  readonly custodyRung: GraphMemoryOwnerLifecycleCustodyRung;
  readonly ownerRef?: string;
  readonly projectRef?: string;
}

export interface GraphMemoryOwnerLifecycleCounts {
  readonly mentions: number;
  readonly entities: number;
  readonly relations: number;
  readonly merges: number;
  readonly vectors: number;
  readonly summaries: number;
  readonly rankingRefs: number;
  readonly rankingSnapshots: number;
  readonly archives: number;
}

export interface GraphMemoryOwnerLifecycleProof {
  readonly schemaId: typeof GRAPH_MEMORY_OWNER_LIFECYCLE_PROOF_SCHEMA_ID;
  readonly custodyRung: GraphMemoryOwnerLifecycleCustodyRung;
  readonly scope: Readonly<{ owner: string; project: string }>;
  readonly put: Readonly<{
    receiptRef: string;
    graphDigest: string;
    graphManifestDigest: string;
    counts: GraphMemoryOwnerLifecycleCounts;
  }>;
  readonly before: Readonly<{
    revision: number;
    graphDigest: string;
    graphManifestDigest: string;
    rankingSnapshotDigests: ReadonlyArray<string>;
    provenanceRefs: ReadonlyArray<string>;
    provenanceDigest: string;
    sourceMembershipRefs: ReadonlyArray<string>;
    sourceMembershipDigest: string;
    pendingOperationRef: null;
    counts: GraphMemoryOwnerLifecycleCounts;
  }>;
  readonly incompletePlanRefusal: Readonly<{
    receiptRef: string;
    status: "refused";
    unresolvedCount: number;
    graphDigestUnchanged: true;
  }>;
  readonly exported: Readonly<{
    receiptRef: string;
    archiveRef: string;
    contentDigest: string;
    manifestDigest: string;
    graphDigest: string;
    graphManifestDigest: string;
    encodedBytes: number;
    counts: GraphMemoryOwnerLifecycleCounts;
    exactSdkValidation: true;
  }>;
  readonly forgotten: Readonly<{
    receiptRef: string;
    receiptDigest: string;
    before: GraphMemoryOwnerLifecycleCounts;
    after: GraphMemoryOwnerLifecycleCounts;
  }>;
  readonly after: Readonly<{
    revision: number;
    currentAbsent: true;
    counts: GraphMemoryOwnerLifecycleCounts;
  }>;
  readonly repeated: Readonly<{
    receiptRef: string;
    receiptDigest: string;
    sameReceipt: true;
    before: GraphMemoryOwnerLifecycleCounts;
    after: GraphMemoryOwnerLifecycleCounts;
  }>;
  readonly archiveCleanup: Readonly<{
    retainedThroughForget: true;
    removed: true;
  }>;
  readonly reopenCount: 3;
}

export class GraphMemoryOwnerLifecycleProofError extends S.TaggedErrorClass<GraphMemoryOwnerLifecycleProofError>()(
  "DesktopGraphMemory.OwnerLifecycleProofError",
  {
    operation: S.String,
    reason: S.Literals([
      "adapter_unavailable",
      "graph_invalid",
      "archive_invalid",
      "archive_io",
      "receipt_mismatch",
    ]),
    detailSafe: S.String.check(S.isMaxLength(512)),
  },
) {}

const proofError = (
  operation: string,
  reason: GraphMemoryOwnerLifecycleProofError["reason"],
  detailSafe: string,
) => new GraphMemoryOwnerLifecycleProofError({ operation, reason, detailSafe });

const digest = (value: unknown) => graphDigest(sha256Hex(canonicalJson(value)));
const operationRef = S.decodeUnknownSync(GraphMemoryOperationRef);
const counts = (value: GraphMemoryArtifactAccounting): GraphMemoryOwnerLifecycleCounts => ({
  mentions: value.mentions,
  entities: value.entities,
  relations: value.relations,
  merges: value.merges,
  vectors: value.vectors,
  summaries: value.summaries,
  rankingRefs: value.rankingRefs,
  rankingSnapshots: value.rankingSnapshots,
  archives: value.archives,
});
const storedCounts = (current: GraphMemoryStoredGraph): GraphMemoryOwnerLifecycleCounts => ({
  mentions: current.built.snapshot.mentions.length,
  entities: current.built.snapshot.entities.length,
  relations: current.built.snapshot.relations.length,
  merges: current.built.snapshot.merges.length,
  vectors: current.artifactInventory.vectors.length,
  summaries: current.artifactInventory.summaries.length,
  rankingRefs: current.artifactInventory.rankingRefs.length,
  rankingSnapshots: current.rankingSnapshots.length,
  archives: current.archiveRefs.length,
});

const storeFailure = (operation: string) =>
  Effect.mapError((_error: GraphMemoryStoreError) =>
    proofError(operation, "adapter_unavailable", "The encrypted graph-memory store failed."),
  );

const withStore = <A>(
  input: GraphMemoryOwnerLifecycleProofInput,
  use: (store: GraphMemoryStoreInterface) => Effect.Effect<A, GraphMemoryOwnerLifecycleProofError>,
): Effect.Effect<A, GraphMemoryOwnerLifecycleProofError> =>
  Effect.acquireUseRelease(
    Effect.try({
      try: () =>
        openDesktopGraphMemoryStore({
          enabled: true,
          databasePath: input.databasePath,
          safeStorage: input.safeStorage,
        }),
      catch: () =>
        proofError(
          "open",
          "adapter_unavailable",
          "The encrypted graph-memory adapter cannot open.",
        ),
    }),
    (desktop) =>
      Effect.gen(function* () {
        const store = yield* GraphMemoryStore;
        return yield* use(store);
      }).pipe(Effect.provide(desktop.layer)),
    (desktop) => Effect.sync(() => desktop.close()),
  );

const proofFixture = Effect.fn("DesktopGraphMemory.OwnerLifecycle.fixture")(function* (
  scope: GraphMemoryScope,
) {
  const makeSource = Effect.fn("DesktopGraphMemory.OwnerLifecycle.source")(function* (
    suffix: string,
  ) {
    const handle = yield* makeInlineCorpusHandle(
      buildInlineCorpusInput({
        corpusRef: `corpus.owner-lifecycle.${suffix}`,
        scopeRef: graphMemoryScopeRefFor(scope),
        policy: { includeVisibilities: ["private"], includeRedactionClasses: ["redacted"] },
        entries: [
          {
            entryRef: `entry.${suffix}`,
            scopeRef: graphMemoryScopeRefFor(scope),
            sourcePlane: "repository",
            sourceKind: "evaluated-owner-memory",
            sourceAddress: {
              addressSchemaId: "openagents.desktop.owner_lifecycle_source.v1",
              encodedAddress: `owner-memory://evaluated/${suffix}`,
            },
            text: `PRIVATE_SENTINEL_SOURCE_${suffix.toUpperCase()}`,
            visibility: "private",
            redactionClass: "redacted",
          },
        ],
      }),
    ).pipe(
      Effect.mapError(() =>
        proofError("build_source", "graph_invalid", "An evaluated source corpus is invalid."),
      ),
    );
    const entry = (yield* handle
      .materializeAll()
      .pipe(
        Effect.mapError(() =>
          proofError("build_source", "graph_invalid", "An evaluated source entry is unavailable."),
        ),
      ))[0];
    if (entry === undefined) {
      return yield* proofError(
        "build_source",
        "graph_invalid",
        "An evaluated source corpus is empty.",
      );
    }
    const source = S.decodeUnknownSync(GraphSourceMembership)({
      source: {
        sourcePlane: entry.sourcePlane,
        sourceKind: entry.sourceKind,
        sourceAddress: entry.sourceAddress,
        corpusRef: handle.identity.corpusRef,
        contentDigest: handle.identity.contentDigest,
        entryRef: entry.entryRef,
      },
    }).source;
    return { handle, source };
  });
  const leafA = yield* makeSource("private-a");
  const leafB = yield* makeSource("private-b");
  const sourceA = leafA.source;
  const sourceB = leafB.source;
  const derivation = S.decodeUnknownSync(GraphDerivation)({
    _tag: "Deterministic",
    parserRef: "parser.owner-lifecycle-proof",
    parserVersion: "version.1",
  });
  const mentionA = makeGraphMention({
    identityNamespace: "owner-memory-proof",
    canonicalKey: "PRIVATE_SENTINEL_ALPHA",
    identityScopeRef: graphMemoryScopeRefFor(scope),
    source: sourceA,
    derivation,
  });
  const mentionB = makeGraphMention({
    identityNamespace: "owner-memory-proof",
    canonicalKey: "PRIVATE_SENTINEL_BETA",
    identityScopeRef: graphMemoryScopeRefFor(scope),
    source: sourceB,
    derivation,
  });
  const entityA = makeCanonicalEntity({
    identityNamespace: "owner-memory-proof",
    canonicalKey: "PRIVATE_SENTINEL_ENTITY_ALPHA",
    identityScopeRef: graphMemoryScopeRefFor(scope),
    mentions: [mentionA],
    derivation,
  });
  const entityB = makeCanonicalEntity({
    identityNamespace: "owner-memory-proof",
    canonicalKey: "PRIVATE_SENTINEL_ENTITY_BETA",
    identityScopeRef: graphMemoryScopeRefFor(scope),
    mentions: [mentionB],
    derivation,
  });
  const relation = makeGraphRelation({
    identityNamespace: "owner-memory-proof",
    canonicalKey: "PRIVATE_SENTINEL_RELATION",
    identityScopeRef: graphMemoryScopeRefFor(scope),
    relationKind: "evaluated_link",
    from: entityA,
    to: entityB,
    memberships: [{ source: sourceA }, { source: sourceB }],
    derivation,
  });
  const policy = {
    includeVisibilities: ["private"],
    includeRedactionClasses: ["redacted"],
  } as const;
  const descriptor = makeEmbeddingProjectionDescriptor({
    projectionSchemaId: "projection.owner-lifecycle.float32.v1",
    elementKinds: ["mention"],
    embeddableFields: ["identity.canonicalKey"],
    dimensions: 2,
  });
  const built = yield* buildGraphCorpus({
    graphRef: "graph.owner-lifecycle.evaluated",
    scopeRef: graphMemoryScopeRefFor(scope),
    policy,
    mentions: [mentionA, mentionB],
    entities: [entityA, entityB],
    relations: [relation],
    embeddingProjections: [descriptor],
  }).pipe(
    Effect.mapError(() =>
      proofError("build_graph", "graph_invalid", "The evaluated proof graph is invalid."),
    ),
  );
  const graphHandle = yield* makeInMemoryGraphSnapshotHandle(built).pipe(
    Effect.mapError(() =>
      proofError("build_graph", "graph_invalid", "The evaluated graph handle is invalid."),
    ),
  );
  const classifications = [
    ...built.snapshot.mentions,
    ...built.snapshot.entities,
    ...built.snapshot.relations,
  ].map((element) => ({
    elementRef: element.elementRef,
    visibility: "private" as const,
    redactionClass: "redacted" as const,
  }));
  const classification = makeGraphRlmClassificationProjection(graphHandle, classifications, [
    leafA.handle,
    leafB.handle,
  ]);
  const projection = yield* makeGraphRlmProjection({
    handle: graphHandle,
    capabilities: makeGraphAdapterCapabilities(["graph_read", "rlm_v2_projection"]),
    classification,
    corpusRef: "corpus.owner-lifecycle.graph-rlm",
    supportingCorpora: [leafA.handle, leafB.handle],
  }).pipe(
    Effect.mapError(() =>
      proofError("evaluate_graph", "graph_invalid", "The graph RLM projection is invalid."),
    ),
  );
  const result = yield* projection.operators
    .lookup(mentionA.elementRef, {
      maxDepth: 0,
      maxVisitedElements: 8,
      maxReturnedElements: 8,
      maxSourceAddresses: 8,
      maxCharactersPerResult: 2_048,
      maxObservationCharacters: 4_096,
    })
    .pipe(
      Effect.mapError(() =>
        proofError("evaluate_graph", "graph_invalid", "The graph evaluation result is invalid."),
      ),
    );
  if (result._tag !== "Complete") {
    return yield* proofError(
      "evaluate_graph",
      "graph_invalid",
      "The graph evaluation result is incomplete.",
    );
  }
  const rankingBinding = {
    schemaId: "openagents.ai.graph_ranking_operation_binding.v1" as const,
    _tag: "Lookup" as const,
    elementRef: mentionA.elementRef,
  };
  const feedback = yield* makeGraphFeedbackObservation({
    built,
    projection,
    result,
    expectedOperationDigest: result.operationDigest,
    binding: rankingBinding,
    elementRef: mentionA.elementRef,
    feedbackWeightMicros: 500_000,
    evidenceRef: "evidence.owner-lifecycle.evaluated",
  }).pipe(
    Effect.mapError(() =>
      proofError("evaluate_graph", "graph_invalid", "The graph feedback is invalid."),
    ),
  );
  const rankingSnapshot = yield* makeGraphRankingSnapshot({
    built,
    projection,
    result,
    expectedOperationDigest: result.operationDigest,
    binding: rankingBinding,
    feedbackObservations: [feedback],
  }).pipe(
    Effect.mapError(() =>
      proofError("evaluate_graph", "graph_invalid", "The graph ranking snapshot is invalid."),
    ),
  );
  const rankingRefs = yield* rankingArtifactsFromSnapshot(rankingSnapshot, {
    built,
    projection,
    result,
    expectedOperationDigest: result.operationDigest,
    binding: rankingBinding,
  }).pipe(
    Effect.mapError(() =>
      proofError("evaluate_graph", "graph_invalid", "The graph ranking refs are invalid."),
    ),
  );
  const vectorBytes = new Uint8Array(8);
  const vectorView = new DataView(vectorBytes.buffer);
  vectorView.setFloat32(0, 0.25, true);
  vectorView.setFloat32(4, 0.75, true);
  const vectorDigest = graphDigest(createHash("sha256").update(vectorBytes).digest("hex"));
  const vectorRecord = S.decodeUnknownSync(GraphArchiveVectorRecord)({
    artifact: {
      artifactKind: "vector",
      artifactRef: "vector.owner-lifecycle.mention-a",
      artifactDigest: vectorDigest,
      ownerElementRef: mentionA.elementRef,
    },
    descriptorRef: descriptor.descriptorRef,
    dimensions: 2,
    encoding: "float32-le-base64",
    payloadBase64: Buffer.from(vectorBytes).toString("base64"),
    payloadDigest: vectorDigest,
    visibility: "private",
    redactionClass: "redacted",
  });
  const summaryBytes = Buffer.from("PRIVATE_SENTINEL_SUMMARY", "utf8");
  const summaryDigest = graphDigest(createHash("sha256").update(summaryBytes).digest("hex"));
  const summaryRecord = S.decodeUnknownSync(GraphArchiveSummaryRecord)({
    artifact: {
      artifactKind: "summary",
      artifactRef: "summary.owner-lifecycle.entity-a",
      artifactDigest: summaryDigest,
      ownerElementRef: entityA.elementRef,
    },
    summarySchemaId: "summary.owner-lifecycle.v1",
    encoding: "utf8-base64",
    payloadBase64: summaryBytes.toString("base64"),
    payloadDigest: summaryDigest,
    visibility: "private",
    redactionClass: "redacted",
  });
  const artifactInventory = makeGraphArtifactInventory({
    built,
    vectors: [vectorRecord.artifact],
    summaries: [summaryRecord.artifact],
    rankingRefs,
    coverage: {
      vectors: { _tag: "Complete" },
      summaries: { _tag: "Complete" },
      rankingRefs: { _tag: "Complete" },
    },
  });
  const binding = S.decodeUnknownSync(GraphMemoryBinding)({
    owner: scope.owner,
    project: scope.project,
    graphScopeRef: built.snapshot.scopeRef,
    sourceBindings: [
      { corpusRef: sourceA.corpusRef, contentDigest: sourceA.contentDigest },
      { corpusRef: sourceB.corpusRef, contentDigest: sourceB.contentDigest },
    ],
    graphRef: built.snapshot.graphRef,
    graphDigest: built.snapshot.graphDigest,
    manifestDigest: built.manifest.manifestDigest,
    policyDigest: digest(policy),
    generation: 1,
  });
  const incompletePlan = yield* planGraphSourceDeletion(built, sourceA, artifactInventory).pipe(
    Effect.mapError(() =>
      proofError("plan_delete", "graph_invalid", "The incomplete delete plan is invalid."),
    ),
  );
  if (incompletePlan._tag !== "Incomplete") {
    return yield* proofError(
      "plan_delete",
      "graph_invalid",
      "The retained relation endpoint did not produce an incomplete plan.",
    );
  }
  return {
    built,
    artifactInventory,
    binding,
    rankingSnapshot,
    vectorRecord,
    summaryRecord,
    incompletePlan,
  };
});

/**
 * Prove the owner-local lifecycle through the encrypted Desktop adapter. Raw
 * archive bytes remain at the caller-provided private path until both forget
 * checks finish. The returned record contains aggregate refs and counts only.
 */
export const runGraphMemoryOwnerLifecycleProof = Effect.fn("DesktopGraphMemory.OwnerLifecycle.run")(
  function* (input: GraphMemoryOwnerLifecycleProofInput) {
    const scope: GraphMemoryScope = {
      owner: ownerScopeId(input.ownerRef ?? "owner.lifecycle-proof"),
      project: projectScopeId(input.projectRef ?? "project.lifecycle-proof"),
    };
    const fixture = yield* proofFixture(scope);
    const put = yield* withStore(input, (store) =>
      store
        .put({
          operationRef: operationRef("operation.owner-lifecycle.put"),
          binding: fixture.binding,
          admission: {
            consent: "granted",
            consentRef: "consent.owner-lifecycle",
            policyRef: "policy.owner-lifecycle",
            redactionState: "already_redacted",
          },
          built: fixture.built,
          artifactInventory: fixture.artifactInventory,
          rankingSnapshots: [fixture.rankingSnapshot],
          vectorRecords: [fixture.vectorRecord],
          summaryRecords: [fixture.summaryRecord],
        })
        .pipe(storeFailure("put")),
    );

    const exportedPhase = yield* withStore(input, (store) =>
      Effect.gen(function* () {
        const before = yield* store.inspect(scope).pipe(storeFailure("inspect_before"));
        if (before.current === null || before.pendingOperationRef !== null) {
          return yield* proofError(
            "inspect_before",
            "receipt_mismatch",
            "The reopened store has no current graph.",
          );
        }
        const incompletePlanRefusal = yield* store
          .applyDeletePlan({
            operationRef: operationRef("operation.owner-lifecycle.incomplete-plan"),
            scope,
            expectedGeneration: before.current.binding.generation,
            plan: fixture.incompletePlan,
          })
          .pipe(storeFailure("refuse_incomplete_plan"));
        if (
          incompletePlanRefusal.status !== "refused" ||
          incompletePlanRefusal.graphDigestAfter !== before.current.binding.graphDigest
        ) {
          return yield* proofError(
            "refuse_incomplete_plan",
            "receipt_mismatch",
            "The incomplete delete plan did not fail closed.",
          );
        }
        const exported = yield* store
          .exportArchive(scope, operationRef("operation.owner-lifecycle.export"))
          .pipe(storeFailure("export"));
        const imported = yield* importGraphCorpusArchive(exported.bytes).pipe(
          Effect.mapError(() =>
            proofError("validate_archive", "archive_invalid", "The SDK rejected its archive."),
          ),
        );
        if (
          imported.archive.manifest.archiveRef !== exported.archiveRef ||
          imported.archive.manifest.contentDigest !== exported.contentDigest ||
          imported.archive.manifest.manifestDigest !== exported.manifestDigest ||
          imported.built.snapshot.graphDigest !== before.current.built.snapshot.graphDigest ||
          imported.built.manifest.manifestDigest !== before.current.built.manifest.manifestDigest ||
          imported.artifactInventory?.inventoryDigest !==
            before.current.artifactInventory.inventoryDigest ||
          canonicalJson(imported.rankingSnapshots) !==
            canonicalJson(before.current.rankingSnapshots)
        ) {
          return yield* proofError(
            "validate_archive",
            "archive_invalid",
            "The archive refs or exact SDK digests do not match the stored graph.",
          );
        }
        yield* Effect.try({
          try: () => writeFileSync(input.archivePath, exported.bytes, { mode: 0o600 }),
          catch: () =>
            proofError("retain_archive", "archive_io", "The private archive cannot be retained."),
        });
        return { before, incompletePlanRefusal, exported, imported };
      }),
    );

    const forgotten = yield* withStore(input, (store) =>
      store
        .forget(scope, operationRef("operation.owner-lifecycle.forget"))
        .pipe(storeFailure("forget")),
    );
    if (!existsSync(input.archivePath)) {
      return yield* proofError(
        "forget",
        "archive_io",
        "The caller-held archive did not remain available through forget.",
      );
    }

    const finalPhase = yield* withStore(input, (store) =>
      Effect.gen(function* () {
        const after = yield* store.inspect(scope).pipe(storeFailure("inspect_after"));
        const repeated = yield* store
          .forget(scope, operationRef("operation.owner-lifecycle.forget"))
          .pipe(storeFailure("repeat_forget"));
        if (after.current !== null || canonicalJson(repeated) !== canonicalJson(forgotten)) {
          return yield* proofError(
            "repeat_forget",
            "receipt_mismatch",
            "Forget did not remain complete and idempotent after restart.",
          );
        }
        return { after, repeated };
      }),
    );

    yield* Effect.try({
      try: () => unlinkSync(input.archivePath),
      catch: () =>
        proofError(
          "cleanup_archive",
          "archive_io",
          "The temporary private archive was not removed.",
        ),
    });
    if (existsSync(input.archivePath)) {
      return yield* proofError(
        "cleanup_archive",
        "archive_io",
        "The temporary private archive remains after cleanup.",
      );
    }

    const beforeCurrent = exportedPhase.before.current;
    if (beforeCurrent === null) {
      return yield* proofError(
        "aggregate",
        "receipt_mismatch",
        "The before-state graph is unavailable.",
      );
    }
    return {
      schemaId: GRAPH_MEMORY_OWNER_LIFECYCLE_PROOF_SCHEMA_ID,
      custodyRung: input.custodyRung,
      scope: { owner: scope.owner, project: scope.project },
      put: {
        receiptRef: put.receiptRef,
        graphDigest: fixture.built.snapshot.graphDigest,
        graphManifestDigest: fixture.built.manifest.manifestDigest,
        counts: counts(put.after),
      },
      before: {
        revision: exportedPhase.before.revision,
        graphDigest: beforeCurrent.built.snapshot.graphDigest,
        graphManifestDigest: beforeCurrent.built.manifest.manifestDigest,
        rankingSnapshotDigests: beforeCurrent.rankingSnapshots.map(
          (snapshot) => snapshot.snapshotDigest,
        ),
        provenanceRefs: exportedPhase.imported.archive.manifest.provenanceRefs,
        provenanceDigest: exportedPhase.imported.archive.sections.provenance.sectionDigest,
        sourceMembershipRefs: exportedPhase.imported.archive.sections.sourceMemberships.entries.map(
          (entry) => entry.elementRef,
        ),
        sourceMembershipDigest:
          exportedPhase.imported.archive.sections.sourceMemberships.sectionDigest,
        pendingOperationRef: null,
        counts: storedCounts(beforeCurrent),
      },
      incompletePlanRefusal: {
        receiptRef: exportedPhase.incompletePlanRefusal.receiptRef,
        status: "refused",
        unresolvedCount: exportedPhase.incompletePlanRefusal.unresolved.length,
        graphDigestUnchanged: true,
      },
      exported: {
        receiptRef: exportedPhase.exported.receipt.receiptRef,
        archiveRef: exportedPhase.exported.archiveRef,
        contentDigest: exportedPhase.exported.contentDigest,
        manifestDigest: exportedPhase.exported.manifestDigest,
        graphDigest: exportedPhase.imported.built.snapshot.graphDigest,
        graphManifestDigest: exportedPhase.imported.built.manifest.manifestDigest,
        encodedBytes: exportedPhase.exported.bytes.byteLength,
        counts: counts(exportedPhase.exported.receipt.after),
        exactSdkValidation: true,
      },
      forgotten: {
        receiptRef: forgotten.receiptRef,
        receiptDigest: forgotten.receiptDigest,
        before: counts(forgotten.before),
        after: counts(forgotten.after),
      },
      after: {
        revision: finalPhase.after.revision,
        currentAbsent: true,
        counts: counts(finalPhase.repeated.after),
      },
      repeated: {
        receiptRef: finalPhase.repeated.receiptRef,
        receiptDigest: finalPhase.repeated.receiptDigest,
        sameReceipt: true,
        before: counts(finalPhase.repeated.before),
        after: counts(finalPhase.repeated.after),
      },
      archiveCleanup: { retainedThroughForget: true, removed: true },
      reopenCount: 3,
    } satisfies GraphMemoryOwnerLifecycleProof;
  },
);
