import {
  buildGraphCorpus,
  canonicalJson,
  graphDigest,
  makeCanonicalEntity,
  makeEmbeddingProjectionDescriptor,
  makeGraphMention,
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
  GraphMemoryPersistenceError,
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
    const entity = makeCanonicalEntity({
      identityNamespace: "test",
      canonicalKey: "portable-shared",
      mentions: [mentionA, mentionB],
      derivation,
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
      mentions: [mentionA, mentionB],
      entities: [entity],
      relations: [],
      embeddingProjections: includeArtifacts ? [descriptor] : [],
    });
    const vectorDigest = graphDigest("df3f619804a92fdb4057192dc43dd748ea778adc52bc498ce80524c014b81119");
    const summaryDigest = graphDigest("5abacc050045ac77f2cf27391427fee97844fa8e7fe1e94757fbd0ee5c81313f");
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
          payloadBase64: "cG9ydGFibGUgc3VtbWFyeQ==",
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
    expect(imported.status).toBe("complete");
    expect(restored.current?.binding.graphDigest).toBe(graph.built.snapshot.graphDigest);
    expect(restored.current?.binding.manifestDigest).toBe(graph.built.manifest.manifestDigest);
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
    expect(incomplete.status).toBe("refused");
    expect(incomplete.unresolved.length).toBeGreaterThan(0);
    expect(deleted.status).toBe("complete");
    expect(deleted.retainedShared.length).toBeGreaterThan(0);
    expect(after.current?.binding.generation).toBe(2);
    expect(after.current?.binding.sourceBindings.map((item) => item.corpusRef)).toEqual([sourceB.corpusRef]);
    expect(after.current?.built.snapshot.entities).toHaveLength(1);
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
});
