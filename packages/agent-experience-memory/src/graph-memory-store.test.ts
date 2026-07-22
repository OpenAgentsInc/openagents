import {
  buildGraphCorpus,
  canonicalJson,
  graphDigest,
  makeCanonicalEntity,
  makeEmbeddingProjectionDescriptor,
  makeGraphMention,
  makeMergeEvidence,
  sha256Hex,
  type BuiltGraphCorpus,
} from "@openagentsinc/graph-corpus";
import {
  GraphArchiveSummaryRecord,
  GraphArchiveVectorRecord,
} from "@openagentsinc/graph-corpus/archive";
import {
  GraphRankingArtifact,
  GraphSummaryArtifact,
  GraphVectorArtifact,
  makeGraphArtifactInventory,
  planGraphSourceDeletion,
  GraphDeleteRef,
  type GraphArtifactInventory,
} from "@openagentsinc/graph-corpus/deletion";
import {
  GraphRankingConfidence,
  GraphRankingFeature,
  GraphRankingSnapshot,
} from "@openagentsinc/graph-corpus/ranking";
import {
  GraphCorpusPolicy,
  GraphDerivation,
  GraphSourceMembership,
} from "@openagentsinc/graph-corpus/schemas";
import { Effect, Ref, Result, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { ownerScopeId, projectScopeId } from "./contract/index.js";
import {
  GraphMemoryBinding,
  GraphMemoryOperationRef,
  GraphMemoryPersistedEnvelope,
  GraphMemoryPersistenceError,
  GRAPH_MEMORY_RECEIPT_LIMIT,
  graphMemoryScopeRefFor,
  makeGraphMemoryStore,
  makeInMemoryGraphMemoryStateStore,
  type GraphMemoryAdmission,
  type GraphMemoryScope,
  type GraphMemoryStateStore,
} from "./graph-memory-store.js";

const scope: GraphMemoryScope = {
  owner: ownerScopeId("owner:graph-test"),
  project: projectScopeId("project:graph-test"),
};
const otherScope: GraphMemoryScope = {
  owner: ownerScopeId("owner:other"),
  project: scope.project,
};
const admission: GraphMemoryAdmission = {
  consent: "granted",
  consentRef: "consent.graph-test",
  policyRef: "policy.graph-test",
  redactionState: "already_redacted",
};
const operationRef = (value: string) => S.decodeUnknownSync(GraphMemoryOperationRef)(value);
const deleteRef = (value: string) => S.decodeUnknownSync(GraphDeleteRef)(value);
const derivation = S.decodeUnknownSync(GraphDerivation)({
  _tag: "Deterministic",
  parserRef: "parser.graph-test",
  parserVersion: "version.1",
});
const digest = (value: unknown) => graphDigest(sha256Hex(canonicalJson(value)));

const source = (suffix: string) =>
  S.decodeUnknownSync(GraphSourceMembership)({
    source: {
      sourcePlane: "repository",
      sourceKind: "graph-test",
      sourceAddress: {
        addressSchemaId: "openagents.test.repository_address.v1",
        encodedAddress: `memory://${suffix}`,
      },
      corpusRef: `corpus.${suffix}`,
      contentDigest: digest({ suffix }),
      entryRef: `entry.${suffix}`,
    },
  }).source;

const sourceA = source("a");
const sourceB = source("b");

const fixture = (
  targetScope: GraphMemoryScope = scope,
  includeSecret = false,
  includeArtifacts = false,
  artifactVariant: 1 | 2 | 3 = 1,
) =>
  Effect.gen(function* () {
    const mentionA = makeGraphMention({
      identityNamespace: "test",
      canonicalKey: "portable-a",
      source: sourceA,
      derivation,
    });
    const mentionB = makeGraphMention({
      identityNamespace: "test",
      canonicalKey: "portable-b",
      source: sourceB,
      derivation,
    });
    const mentionB2 = makeGraphMention({
      identityNamespace: "test",
      canonicalKey: "portable-b2",
      source: sourceB,
      derivation,
    });
    const mentionA2 = makeGraphMention({
      identityNamespace: "test",
      canonicalKey: "portable-a2",
      source: sourceA,
      derivation,
    });
    const mentionA3 = makeGraphMention({
      identityNamespace: "test",
      canonicalKey: "portable-a3",
      source: sourceA,
      derivation,
    });
    const entity = makeCanonicalEntity({
      identityNamespace: "test",
      canonicalKey: "portable-shared",
      mentions: [mentionA, mentionB, mentionB2],
      derivation,
    });
    const sourceAEntity = makeCanonicalEntity({
      identityNamespace: "test",
      canonicalKey: "portable-source-a",
      mentions: [mentionA2, mentionA3],
      derivation,
    });
    const sharedMerge = makeMergeEvidence({
      entity,
      mentions: [mentionA, mentionB, mentionB2],
      evidenceRef: "evidence.merge.shared",
    });
    const removableMerge = makeMergeEvidence({
      entity: sourceAEntity,
      mentions: [mentionA2, mentionA3],
      evidenceRef: "evidence.merge.removable",
    });
    const policy = S.decodeUnknownSync(GraphCorpusPolicy)({
      includeVisibilities: ["private"],
      includeRedactionClasses: includeSecret ? ["redacted", "secret"] : ["redacted"],
    });
    const descriptor = makeEmbeddingProjectionDescriptor({
      projectionSchemaId: "openagents.test.embedding.v1",
      elementKinds: ["mention"],
      embeddableFields: ["identity.canonicalKey"],
      dimensions: 1,
    });
    const built = yield* buildGraphCorpus({
      graphRef: "graph.graph-test",
      scopeRef: graphMemoryScopeRefFor(targetScope),
      policy,
      mentions: [mentionA, mentionB, mentionB2, mentionA2, mentionA3],
      entities: [entity, sourceAEntity],
      relations: [],
      merges: [sharedMerge, removableMerge],
      embeddingProjections: includeArtifacts ? [descriptor] : [],
    });
    const vectorDigest = graphDigest("df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119");
    const summaryDigests = {
      1: graphDigest("5abacc050045ac77f2cf27391427fee97844fa8e7fe1e94757fbd0ee5c81313f"),
      2: graphDigest("106fcc2c575166a06c1604a32f8e54054d04387d9d874e9ab9efe7bbdc40f86d"),
      3: graphDigest("f0ed4e9df6ab88ed3a64e4f8e8c731532e32c8cccb753d2d2e163659c64f23cb"),
    };
    const summaryPayloads = {
      1: "cG9ydGFibGUgc3VtbWFyeQ==",
      2: "Y2hhbmdlZCBwb3J0YWJsZSBzdW1tYXJ5",
      3: "dG9rZW4gc2stbGl2ZS1BQkNERUZHSDEyMzQ1Njc4aWprbG1ub3A=",
    };
    const summaryDigest = summaryDigests[artifactVariant];
    const vectors = includeArtifacts
      ? [S.decodeUnknownSync(GraphVectorArtifact)({
          artifactRef: "artifact.vector.1",
          artifactDigest: vectorDigest,
          artifactKind: "vector",
          ownerElementRef: mentionA.elementRef,
        })]
      : [];
    const summaries = includeArtifacts
      ? [S.decodeUnknownSync(GraphSummaryArtifact)({
          artifactRef: "artifact.summary.1",
          artifactDigest: summaryDigest,
          artifactKind: "summary",
          ownerElementRef: mentionA.elementRef,
        })]
      : [];
    const rankingIdentity = {
      graphRef: built.snapshot.graphRef,
      scopeRef: built.snapshot.scopeRef,
      graphDigest: built.snapshot.graphDigest,
      manifestDigest: built.manifest.manifestDigest,
      corpusRef: sourceA.corpusRef,
      contentDigest: sourceA.contentDigest,
      corpusManifestDigest: digest({ corpusManifest: 1 }),
      classificationDigest: digest({ classification: 1 }),
    };
    const confidence = S.decodeUnknownSync(GraphRankingConfidence)({
      ...rankingIdentity,
      elementRef: mentionA.elementRef,
      confidenceMicros: 750_000,
      evidenceRef: deleteRef("evidence.ranking-confidence.1"),
    });
    const featureContent = {
      ...rankingIdentity,
      elementRef: mentionA.elementRef,
      feedbackWeightMicros: 0,
      confidenceMicros: confidence.confidenceMicros,
      feedbackObservationRefs: [],
      confidenceEvidenceRef: confidence.evidenceRef,
    };
    const feature = S.decodeUnknownSync(GraphRankingFeature)({
      ...featureContent,
      featureRef: deleteRef(`ranking-feature.${digest(featureContent)}`),
      featureDigest: digest(featureContent),
    });
    const rankingContent = {
      schemaId: "openagents.ai.graph_ranking_snapshot.v1",
      ...rankingIdentity,
      algorithmVersion: "openagents.ai.graph_ranking.feedback-confidence-relevance-ref.v1",
      feedbackObservations: [],
      confidences: [confidence],
      features: [feature],
    };
    const rankingSnapshots = includeArtifacts
      ? [S.decodeUnknownSync(GraphRankingSnapshot)({
          ...rankingContent,
          snapshotRef: deleteRef(`ranking-snapshot.${digest(rankingContent)}`),
          snapshotDigest: digest(rankingContent),
        })]
      : [];
    const rankingRefs = includeArtifacts
      ? [S.decodeUnknownSync(GraphRankingArtifact)({
          artifactRef: feature.featureRef,
          artifactDigest: feature.featureDigest,
          artifactKind: "ranking_ref",
          ownerElementRef: feature.elementRef,
        })]
      : [];
    const inventory = makeGraphArtifactInventory({
      built,
      vectors,
      summaries,
      rankingRefs,
      coverage: {
        vectors: { _tag: "Complete" },
        summaries: { _tag: "Complete" },
        rankingRefs: { _tag: "Complete" },
      },
    });
    const binding = S.decodeUnknownSync(GraphMemoryBinding)({
      owner: targetScope.owner,
      project: targetScope.project,
      graphScopeRef: built.snapshot.scopeRef,
      sourceBindings: [sourceA, sourceB].map((item) => ({
        corpusRef: item.corpusRef,
        contentDigest: item.contentDigest,
      })),
      graphRef: built.snapshot.graphRef,
      graphDigest: built.snapshot.graphDigest,
      manifestDigest: built.manifest.manifestDigest,
      policyDigest: digest(built.snapshot.policy),
      generation: 1,
    });
    const vectorRecords = includeArtifacts
      ? [S.decodeUnknownSync(GraphArchiveVectorRecord)({
          artifact: vectors[0],
          descriptorRef: descriptor.descriptorRef,
          dimensions: 1,
          encoding: "float32-le-base64",
          payloadBase64: "AAAAAA==",
          payloadDigest: vectorDigest,
          visibility: "private",
          redactionClass: "redacted",
        })]
      : [];
    const summaryRecords = includeArtifacts
      ? [S.decodeUnknownSync(GraphArchiveSummaryRecord)({
          artifact: summaries[0],
          summarySchemaId: "openagents.test.summary.v1",
          encoding: "utf8-base64",
          payloadBase64: summaryPayloads[artifactVariant],
          payloadDigest: summaryDigest,
          visibility: "private",
          redactionClass: "redacted",
        })]
      : [];
    return { built, inventory, binding, vectorRecords, summaryRecords, rankingSnapshots };
  });

interface GraphFixture {
  readonly built: BuiltGraphCorpus;
  readonly inventory: GraphArtifactInventory;
  readonly binding: S.Schema.Type<typeof GraphMemoryBinding>;
  readonly vectorRecords: ReadonlyArray<S.Schema.Type<typeof GraphArchiveVectorRecord>>;
  readonly summaryRecords: ReadonlyArray<S.Schema.Type<typeof GraphArchiveSummaryRecord>>;
  readonly rankingSnapshots: ReadonlyArray<S.Schema.Type<typeof GraphRankingSnapshot>>;
}

const putFixture = (builtFixture: GraphFixture) => ({
  operationRef: operationRef("operation.put"),
  binding: builtFixture.binding,
  admission,
  built: builtFixture.built,
  artifactInventory: builtFixture.inventory,
  vectorRecords: builtFixture.vectorRecords,
  summaryRecords: builtFixture.summaryRecords,
  rankingSnapshots: builtFixture.rankingSnapshots,
});

const withGeneration = (builtFixture: GraphFixture, generation: number): GraphFixture => ({
  ...builtFixture,
  binding: S.decodeUnknownSync(GraphMemoryBinding)({ ...builtFixture.binding, generation }),
});

describe("portable graph memory conformance", () => {
  test("the disabled driver performs zero I/O", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const disabled: GraphMemoryStateStore = { ...driver, enabled: false };
    const graph = await Effect.runPromise(fixture());
    const store = await Effect.runPromise(makeGraphMemoryStore(disabled));
    const receipt = await Effect.runPromise(store.put(putFixture(graph)));
    const inspected = await Effect.runPromise(store.inspect(scope));

    expect(receipt.status).toBe("disabled");
    expect(inspected.current).toBeNull();
    expect(await Effect.runPromise(driver.reads)).toBe(0);
    expect(await Effect.runPromise(driver.writes)).toBe(0);
  });

  test("scope binding is deterministic and rejects a graph from another owner", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const store = await Effect.runPromise(makeGraphMemoryStore(driver));
    const graph = await Effect.runPromise(fixture(scope));
    const badBinding = S.decodeUnknownSync(GraphMemoryBinding)({
      ...graph.binding,
      owner: otherScope.owner,
      project: otherScope.project,
    });
    const outcome = await Effect.runPromise(Effect.result(store.put({
      ...putFixture(graph),
      binding: badBinding,
    })));

    expect(Result.isFailure(outcome)).toBe(true);
    if (Result.isFailure(outcome)) expect(outcome.failure.reason).toBe("scope_violation");
    expect(await Effect.runPromise(driver.writes)).toBe(0);
    expect(graphMemoryScopeRefFor(scope)).not.toBe(graphMemoryScopeRefFor(otherScope));
  });

  test("consent, redaction declaration, and secret policy fail before persistence", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const store = await Effect.runPromise(makeGraphMemoryStore(driver));
    const graph = await Effect.runPromise(fixture());
    const secretGraph = await Effect.runPromise(fixture(scope, true));
    const withheld = await Effect.runPromise(Effect.result(store.put({
      ...putFixture(graph),
      admission: { ...admission, consent: "withheld" },
    })));
    const unreviewed = await Effect.runPromise(Effect.result(store.put({
      ...putFixture(graph),
      admission: { ...admission, redactionState: "unreviewed" },
    })));
    const secretPolicy = await Effect.runPromise(Effect.result(store.put({
      ...putFixture(secretGraph),
      operationRef: operationRef("operation.secret-policy"),
    })));

    expect(Result.isFailure(withheld) && withheld.failure.reason).toBe("consent_required");
    expect(Result.isFailure(unreviewed) && unreviewed.failure.reason).toBe("unredacted_input");
    expect(Result.isFailure(secretPolicy) && secretPolicy.failure.reason).toBe("policy_mismatch");
    expect(await Effect.runPromise(driver.writes)).toBe(0);
  });

  test("archive digests round-trip and forget accounts for caller-owned exported bytes", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const store = await Effect.runPromise(makeGraphMemoryStore(driver));
    const graph = await Effect.runPromise(fixture());
    await Effect.runPromise(store.put(putFixture(graph)));
    const exported = await Effect.runPromise(store.exportArchive(scope, operationRef("operation.export")));
    const forgotten = await Effect.runPromise(store.forget(scope, operationRef("operation.forget")));
    const empty = await Effect.runPromise(store.inspect(scope));
    const forgottenEnvelope = S.decodeUnknownSync(GraphMemoryPersistedEnvelope)(
      await Effect.runPromise(driver.load(scope)),
    );
    const imported = await Effect.runPromise(store.importArchive({
      operationRef: operationRef("operation.import"),
      scope,
      generation: 2,
      admission,
      bytes: exported.bytes,
    }));
    const restored = await Effect.runPromise(store.inspect(scope));

    expect(exported.contentDigest).toBeDefined();
    expect(forgotten.after.archives).toBe(0);
    expect(forgotten.retainedShared).toContainEqual(expect.objectContaining({
      plane: "archive",
      targetRef: exported.archiveRef,
      reason: "owner_export_payload_not_stored",
    }));
    expect(forgotten.applied.some((item) => item.targetRef === exported.archiveRef)).toBe(false);
    expect(empty.current).toBeNull();
    expect(forgottenEnvelope.archiveExports).toEqual([]);
    expect(imported.status).toBe("complete");
    expect(restored.current?.binding.graphDigest).toBe(graph.built.snapshot.graphDigest);
    expect(restored.current?.binding.manifestDigest).toBe(graph.built.manifest.manifestDigest);
  });

  test("an export retry returns the exact committed bytes after later archival state changes", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const store = await Effect.runPromise(makeGraphMemoryStore(driver));
    const firstGraph = await Effect.runPromise(fixture(scope, false, true, 1));
    await Effect.runPromise(store.put(putFixture(firstGraph)));
    const exportRef = operationRef("operation.export-idempotent");
    const first = await Effect.runPromise(store.exportArchive(scope, exportRef));
    const changedGraph = withGeneration(
      await Effect.runPromise(fixture(scope, false, true, 2)),
      2,
    );
    await Effect.runPromise(store.put({
      ...putFixture(changedGraph),
      operationRef: operationRef("operation.put.changed-artifacts"),
    }));
    const retry = await Effect.runPromise(store.exportArchive(scope, exportRef));

    expect([...retry.bytes]).toEqual([...first.bytes]);
    expect(retry.archiveRef).toBe(first.archiveRef);
    expect(retry.receipt.receiptDigest).toBe(first.receipt.receiptDigest);
    expect(retry.contentDigest).toBe(first.contentDigest);
  });

  test("export CAS refuses a concurrent same-graph artifact-state replacement", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const firstGraph = await Effect.runPromise(fixture(scope, false, true, 1));
    const changedGraph = withGeneration(
      await Effect.runPromise(fixture(scope, false, true, 2)),
      2,
    );
    expect(changedGraph.built.snapshot.graphDigest).toBe(firstGraph.built.snapshot.graphDigest);
    const baseStore = await Effect.runPromise(makeGraphMemoryStore(driver));
    await Effect.runPromise(baseStore.put(putFixture(firstGraph)));

    const changedDriver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const changedStore = await Effect.runPromise(makeGraphMemoryStore(changedDriver));
    await Effect.runPromise(changedStore.put({
      ...putFixture(changedGraph),
      operationRef: operationRef("operation.put.concurrent-artifacts"),
    }));
    const changedCurrent = (await Effect.runPromise(changedStore.inspect(scope))).current;
    expect(changedCurrent).not.toBeNull();
    const intercepted = await Effect.runPromise(Ref.make(false));
    const racingDriver: GraphMemoryStateStore = {
      ...driver,
      compareAndSet: (targetScope, expectedRevision, next) =>
        Effect.gen(function* () {
          const prepared = S.decodeUnknownSync(GraphMemoryPersistedEnvelope)(next);
          const wasIntercepted = yield* Ref.get(intercepted);
          if (!wasIntercepted && prepared.pending?.receipt.operation === "archive_export") {
            yield* Ref.set(intercepted, true);
            const concurrent = S.decodeUnknownSync(GraphMemoryPersistedEnvelope)({
              schemaId: "openagents.graph_memory_state.v1",
              scope: targetScope,
              revision: prepared.revision,
              current: changedCurrent,
              receipts: prepared.receipts,
              archiveExports: prepared.archiveExports ?? [],
            });
            const replaced = yield* driver.compareAndSet(targetScope, expectedRevision, concurrent);
            expect(replaced).toBe(true);
            return false;
          }
          return yield* driver.compareAndSet(targetScope, expectedRevision, next);
        }),
    };
    const racingStore = await Effect.runPromise(makeGraphMemoryStore(racingDriver));
    const outcome = await Effect.runPromise(Effect.result(
      racingStore.exportArchive(scope, operationRef("operation.export-race")),
    ));

    expect(Result.isFailure(outcome)).toBe(true);
    if (Result.isFailure(outcome)) expect(outcome.failure.reason).toBe("stale_generation");
    expect((await Effect.runPromise(racingStore.inspect(scope))).current?.binding.generation).toBe(2);
  });

  test("caller-labeled redacted summary payloads still reject decoded secret text", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const store = await Effect.runPromise(makeGraphMemoryStore(driver));
    const graph = await Effect.runPromise(fixture(scope, false, true, 3));
    const outcome = await Effect.runPromise(Effect.result(store.put(putFixture(graph))));

    expect(Result.isFailure(outcome)).toBe(true);
    if (Result.isFailure(outcome)) expect(outcome.failure.reason).toBe("unredacted_input");
    expect(await Effect.runPromise(driver.writes)).toBe(0);
  });

  test("refuses stale and incomplete plans, then retains shared source facts on exact deletion", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const store = await Effect.runPromise(makeGraphMemoryStore(driver));
    const graph = await Effect.runPromise(fixture());
    await Effect.runPromise(store.put(putFixture(graph)));
    const plan = await Effect.runPromise(planGraphSourceDeletion(graph.built, sourceA, graph.inventory));
    const stale = await Effect.runPromise(store.applyDeletePlan({
      operationRef: operationRef("operation.delete-stale"),
      scope,
      expectedGeneration: 2,
      plan,
    }));
    const conflictingStaleRetry = await Effect.runPromise(Effect.result(store.applyDeletePlan({
      operationRef: operationRef("operation.delete-stale"),
      scope,
      expectedGeneration: 1,
      plan,
    })));
    const incompleteInventory = makeGraphArtifactInventory({
      built: graph.built,
      vectors: [],
      summaries: [],
      rankingRefs: [],
      coverage: {
        vectors: {
          _tag: "Incomplete",
          gaps: [{
            artifactKind: "vector",
            reason: "owner_unknown",
            evidenceRef: deleteRef("evidence.vector-owner-unknown"),
          }],
        },
        summaries: { _tag: "Complete" },
        rankingRefs: { _tag: "Complete" },
      },
    });
    const incompletePlan = await Effect.runPromise(
      planGraphSourceDeletion(graph.built, sourceA, incompleteInventory),
    );
    const incomplete = await Effect.runPromise(store.applyDeletePlan({
      operationRef: operationRef("operation.delete-incomplete"),
      scope,
      expectedGeneration: 1,
      plan: incompletePlan,
    }));
    const deleted = await Effect.runPromise(store.applyDeletePlan({
      operationRef: operationRef("operation.delete-complete"),
      scope,
      expectedGeneration: 1,
      plan,
    }));
    const after = await Effect.runPromise(store.inspect(scope));

    expect(stale.status).toBe("refused");
    expect(Result.isFailure(conflictingStaleRetry)).toBe(true);
    if (Result.isFailure(conflictingStaleRetry)) {
      expect(conflictingStaleRetry.failure.reason).toBe("state_conflict");
    }
    expect(incomplete.status).toBe("refused");
    expect(incomplete.unresolved.length).toBeGreaterThan(0);
    expect(deleted.status).toBe("complete");
    expect(deleted.retainedShared.length).toBeGreaterThan(0);
    expect(after.current?.binding.generation).toBe(2);
    expect(after.current?.binding.sourceBindings.map((item) => item.corpusRef)).toEqual([sourceB.corpusRef]);
    expect(after.current?.built.snapshot.entities).toHaveLength(1);
    const removedMerge = plan.actions.removableMerges[0];
    const rekeyedMerge = plan.actions.mergeRekeys[0];
    expect(removedMerge).toBeDefined();
    expect(rekeyedMerge).toBeDefined();
    expect(deleted.applied).toContainEqual(expect.objectContaining({
      plane: "graph",
      targetRef: removedMerge?.mergeRef,
      actionRef: removedMerge?.actionRef,
      reason: "remove_merge",
    }));
    expect(deleted.retainedShared.some((item) => item.targetRef === removedMerge?.mergeRef)).toBe(false);
    expect(deleted.applied).toContainEqual(expect.objectContaining({
      plane: "graph",
      targetRef: rekeyedMerge?.oldMergeRef,
      actionRef: rekeyedMerge?.actionRef,
      reason: "rekey_shared_merge",
    }));
  });

  test("source deletion maps artifact actions to exact receipt planes and leaves no owner orphan", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const store = await Effect.runPromise(makeGraphMemoryStore(driver));
    const graph = await Effect.runPromise(fixture(scope, false, true));
    await Effect.runPromise(store.put(putFixture(graph)));
    const plan = await Effect.runPromise(planGraphSourceDeletion(graph.built, sourceA, graph.inventory));
    const receipt = await Effect.runPromise(store.applyDeletePlan({
      operationRef: operationRef("operation.delete-artifacts"),
      scope,
      expectedGeneration: 1,
      plan,
    }));
    const after = await Effect.runPromise(store.inspect(scope));
    const planes = [
      ["vector", plan.actions.vectorActions],
      ["summary", plan.actions.summaryActions],
      ["ranking", plan.actions.rankingRefActions],
    ] as const;
    for (const [plane, actions] of planes) {
      expect(actions.length).toBeGreaterThan(0);
      for (const action of actions) {
        expect(receipt.applied).toContainEqual(expect.objectContaining({
          plane,
          targetRef: action.artifactRef,
          actionRef: action.actionRef,
        }));
      }
    }
    for (const snapshot of graph.rankingSnapshots) {
      const featureRefs = new Set(snapshot.features.map((item) => item.featureRef));
      const action = plan.actions.rankingRefActions.find((item) => featureRefs.has(item.artifactRef));
      expect(action).toBeDefined();
      expect(receipt.applied).toContainEqual(expect.objectContaining({
        plane: "ranking",
        targetRef: snapshot.snapshotRef,
        actionRef: action?.actionRef,
        reason: "remove_stale_ranking_snapshot",
      }));
    }
    expect(after.current?.artifactInventory.vectors).toEqual([]);
    expect(after.current?.artifactInventory.summaries).toEqual([]);
    expect(after.current?.artifactInventory.rankingRefs).toEqual([]);
    expect(after.current?.vectorRecords).toEqual([]);
    expect(after.current?.summaryRecords).toEqual([]);
    expect(after.current?.rankingSnapshots).toEqual([]);
  });

  test("retries are idempotent and full forget leaves no stored graph or artifact orphan", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const store = await Effect.runPromise(makeGraphMemoryStore(driver));
    const graph = await Effect.runPromise(fixture(scope, false, true));
    const first = await Effect.runPromise(store.put(putFixture(graph)));
    const writes = await Effect.runPromise(driver.writes);
    const retry = await Effect.runPromise(store.put(putFixture(graph)));
    expect(retry.receiptDigest).toBe(first.receiptDigest);
    expect(await Effect.runPromise(driver.writes)).toBe(writes);

    const forgotten = await Effect.runPromise(store.forget(scope, operationRef("operation.forget-all")));
    const forgetWrites = await Effect.runPromise(driver.writes);
    const repeated = await Effect.runPromise(store.forget(scope, operationRef("operation.forget-all")));
    const inspected = await Effect.runPromise(store.inspect(scope));

    expect(repeated.receiptDigest).toBe(forgotten.receiptDigest);
    expect(await Effect.runPromise(driver.writes)).toBe(forgetWrites);
    expect(inspected.current).toBeNull();
    expect(forgotten.before.vectors).toBeGreaterThan(0);
    expect(forgotten.before.summaries).toBeGreaterThan(0);
    expect(forgotten.before.rankingRefs).toBeGreaterThan(0);
    expect(forgotten.before.rankingSnapshots).toBeGreaterThan(0);
    expect(forgotten.after).toEqual({
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
  });

  test("recovery finalizes a prepared mutation after an interrupted commit", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const calls = await Effect.runPromise(Ref.make(0));
    const flaky: GraphMemoryStateStore = {
      ...driver,
      compareAndSet: (targetScope, expectedRevision, next) =>
        Ref.modify(calls, (count) => [count + 1, count + 1] as const).pipe(
          Effect.flatMap((call) =>
            call === 2
              ? Effect.fail(new GraphMemoryPersistenceError({
                  operation: "compareAndSet",
                  reason: "unavailable",
                  detailSafe: "simulated interruption",
                }))
              : driver.compareAndSet(targetScope, expectedRevision, next),
          ),
        ),
    };
    const graph = await Effect.runPromise(fixture());
    const interruptedStore = await Effect.runPromise(makeGraphMemoryStore(flaky));
    const interrupted = await Effect.runPromise(Effect.result(interruptedStore.put(putFixture(graph))));
    expect(Result.isFailure(interrupted)).toBe(true);

    const recoveredStore = await Effect.runPromise(makeGraphMemoryStore(driver));
    const recovered = await Effect.runPromise(recoveredStore.recover(scope));
    const inspected = await Effect.runPromise(recoveredStore.inspect(scope));

    expect(recovered.map((item) => item.operationRef)).toContain(operationRef("operation.put"));
    expect(inspected.pendingOperationRef).toBeNull();
    expect(inspected.current?.binding.graphDigest).toBe(graph.built.snapshot.graphDigest);
  });

  test("an operation reference cannot change requests and a full receipt ledger never stages pending work", async () => {
    const driver = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const store = await Effect.runPromise(makeGraphMemoryStore(driver));
    const graph = await Effect.runPromise(fixture());
    const sharedRef = operationRef("operation.bound-request");
    await Effect.runPromise(store.put({ ...putFixture(graph), operationRef: sharedRef }));
    const conflicting = await Effect.runPromise(Effect.result(store.forget(scope, sharedRef)));
    expect(Result.isFailure(conflicting)).toBe(true);
    if (Result.isFailure(conflicting)) expect(conflicting.failure.reason).toBe("state_conflict");

    const inspected = await Effect.runPromise(store.inspect(scope));
    const template = inspected.receipts[0];
    expect(template).toBeDefined();
    const receipts = Array.from({ length: GRAPH_MEMORY_RECEIPT_LIMIT }, (_, index) => ({
      ...template,
      operationRef: operationRef(`operation.saturated.${index}`),
    }));
    const saturated = S.decodeUnknownSync(GraphMemoryPersistedEnvelope)({
      schemaId: "openagents.graph_memory_state.v1",
      scope,
      revision: inspected.revision + 1,
      current: inspected.current,
      receipts,
      archiveExports: [],
    });
    expect(await Effect.runPromise(driver.compareAndSet(scope, inspected.revision, saturated))).toBe(true);
    const restarted = await Effect.runPromise(makeGraphMemoryStore(driver));
    const overflow = await Effect.runPromise(
      Effect.result(restarted.forget(scope, operationRef("operation.after-limit"))),
    );
    const after = await Effect.runPromise(restarted.inspect(scope));

    expect(Result.isFailure(overflow)).toBe(true);
    if (Result.isFailure(overflow)) expect(overflow.failure.reason).toBe("state_conflict");
    expect(after.pendingOperationRef).toBeNull();
    expect(after.receipts).toHaveLength(GRAPH_MEMORY_RECEIPT_LIMIT);
  });
});
