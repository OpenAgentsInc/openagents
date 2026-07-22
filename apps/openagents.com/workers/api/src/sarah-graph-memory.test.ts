import {
  GraphMemoryBinding,
  GraphMemoryOperationRef,
  GraphMemoryStore,
  graphMemoryScopeRefFor,
  graphMemoryStoreLayer,
  makeInMemoryGraphMemoryStateStore,
  type GraphMemoryScope,
  type GraphMemoryStoreInterface,
} from "@openagentsinc/agent-experience-memory";
import {
  buildGraphCorpus,
  canonicalJson,
  graphDigest,
  makeGraphMention,
  sha256Hex,
} from "@openagentsinc/graph-corpus";
import { makeGraphArtifactInventory } from "@openagentsinc/graph-corpus/deletion";
import {
  GraphCorpusPolicy,
  GraphDerivation,
  GraphSourceMembership,
} from "@openagentsinc/graph-corpus/schemas";
import { Effect, Layer, Schema } from "effect";
import { describe, expect, test } from "vitest";

import {
  recallSarahGraphMemory,
  sarahGraphMemoryRecallEnabled,
  sarahGraphMemoryScope,
  SARAH_GRAPH_MEMORY_KIND,
  SARAH_GRAPH_MEMORY_SOURCE_PREFIX,
} from "./sarah-graph-memory";

const OWNER = "owner.9189.fixture";
const digest = (value: unknown) => graphDigest(sha256Hex(canonicalJson(value)));
const operationRef = Schema.decodeUnknownSync(GraphMemoryOperationRef);

const buildStoredGraph = (scope: GraphMemoryScope, canonicalKey: string) =>
  Effect.gen(function* () {
    const source = Schema.decodeUnknownSync(GraphSourceMembership)({
      source: {
        sourcePlane: "repository",
        sourceKind: "sarah-graph-memory-test",
        sourceAddress: {
          addressSchemaId: "openagents.test.repository_address.v1",
          encodedAddress: "memory://sarah-graph",
        },
        corpusRef: "corpus.sarah-graph",
        contentDigest: digest({ corpus: "sarah-graph" }),
        entryRef: "entry.sarah-graph",
      },
    }).source;
    const derivation = Schema.decodeUnknownSync(GraphDerivation)({
      _tag: "Deterministic",
      parserRef: "parser.sarah-graph",
      parserVersion: "version.1",
    });
    const mention = makeGraphMention({
      identityNamespace: "sarah-test",
      canonicalKey,
      source,
      derivation,
    });
    const policy = Schema.decodeUnknownSync(GraphCorpusPolicy)({
      includeVisibilities: ["private"],
      includeRedactionClasses: ["redacted"],
    });
    const built = yield* buildGraphCorpus({
      graphRef: "graph.sarah-memory-test",
      scopeRef: graphMemoryScopeRefFor(scope),
      policy,
      mentions: [mention],
      entities: [],
      relations: [],
    });
    const artifactInventory = makeGraphArtifactInventory({
      built,
      vectors: [],
      summaries: [],
      rankingRefs: [],
      coverage: {
        vectors: { _tag: "Complete" },
        summaries: { _tag: "Complete" },
        rankingRefs: { _tag: "Complete" },
      },
    });
    const binding = Schema.decodeUnknownSync(GraphMemoryBinding)({
      owner: scope.owner,
      project: scope.project,
      graphScopeRef: built.snapshot.scopeRef,
      sourceBindings: [{ corpusRef: source.corpusRef, contentDigest: source.contentDigest }],
      graphRef: built.snapshot.graphRef,
      graphDigest: built.snapshot.graphDigest,
      manifestDigest: built.manifest.manifestDigest,
      policyDigest: digest(built.snapshot.policy),
      generation: 1,
    });
    return { built, artifactInventory, binding, mention };
  });

/** A real in-memory backing store seeded with one redacted owner-scoped graph. */
const seededLayer = async (
  ownerUserId: string,
  canonicalKey: string,
): Promise<Layer.Layer<GraphMemoryStore>> => {
  const scope = sarahGraphMemoryScope(ownerUserId);
  const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
  const layer = graphMemoryStoreLayer(stateStore);
  await Effect.runPromise(
    Effect.gen(function* () {
      const fixture = yield* buildStoredGraph(scope, canonicalKey);
      const store = yield* GraphMemoryStore;
      yield* store.put({
        operationRef: operationRef("op.sarah-graph.put.1"),
        binding: fixture.binding,
        admission: {
          consent: "granted",
          consentRef: "consent.sarah-graph.test",
          policyRef: "policy.sarah-graph.test",
          redactionState: "already_redacted",
        },
        built: fixture.built,
        artifactInventory: fixture.artifactInventory,
      });
    }).pipe(Effect.provide(layer)),
  );
  return layer;
};

/** A fake store that returns a hand-built inspection, bypassing put validation. */
const fakeInspectLayer = (
  current: unknown,
): Layer.Layer<GraphMemoryStore> => {
  const unused = Effect.die("unused in test");
  const iface = {
    enabled: true,
    inspect: (scope: GraphMemoryScope) =>
      Effect.succeed({
        enabled: true,
        scope,
        revision: 1,
        current,
        receipts: [],
        pendingOperationRef: null,
      }),
    put: () => unused,
    exportArchive: () => unused,
    importArchive: () => unused,
    applyDeletePlan: () => unused,
    forget: () => unused,
    recover: () => unused,
  } as unknown as GraphMemoryStoreInterface;
  return Layer.succeed(GraphMemoryStore, GraphMemoryStore.of(iface));
};

describe("sarahGraphMemoryRecallEnabled", () => {
  test("is off by default and only on for explicit on values", () => {
    expect(sarahGraphMemoryRecallEnabled(undefined)).toBe(false);
    expect(sarahGraphMemoryRecallEnabled({})).toBe(false);
    expect(sarahGraphMemoryRecallEnabled({ SARAH_GRAPH_MEMORY_RECALL_ENABLED: "" })).toBe(false);
    expect(sarahGraphMemoryRecallEnabled({ SARAH_GRAPH_MEMORY_RECALL_ENABLED: "false" })).toBe(
      false,
    );
    expect(sarahGraphMemoryRecallEnabled({ SARAH_GRAPH_MEMORY_RECALL_ENABLED: "true" })).toBe(true);
    expect(sarahGraphMemoryRecallEnabled({ SARAH_GRAPH_MEMORY_RECALL_ENABLED: "1" })).toBe(true);
    expect(sarahGraphMemoryRecallEnabled({ SARAH_GRAPH_MEMORY_RECALL_ENABLED: "on" })).toBe(true);
  });
});

describe("sarahGraphMemoryScope", () => {
  test("derives a valid owner-hashed scope that never embeds the raw owner id", () => {
    const scope = sarahGraphMemoryScope("chris@openagents.com");
    expect(scope.owner.startsWith("owner.sarah.")).toBe(true);
    expect(scope.owner).not.toContain("chris@openagents.com");
    expect(scope.owner).not.toContain("@");
    // Distinct owners get distinct scopes.
    expect(sarahGraphMemoryScope("a").owner).not.toBe(sarahGraphMemoryScope("b").owner);
  });
});

describe("recallSarahGraphMemory", () => {
  test("flag OFF: returns an empty slice and never opens the store", async () => {
    const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const layer = graphMemoryStoreLayer(stateStore);
    const sources = await recallSarahGraphMemory({
      ownerUserId: OWNER,
      query: "what is our release status?",
      enabled: false,
      storeLayer: layer,
    });
    expect(sources).toEqual([]);
    // No inspect/load ever ran against the backing state store.
    expect(await Effect.runPromise(stateStore.reads)).toBe(0);
    expect(await Effect.runPromise(stateStore.writes)).toBe(0);
  });

  test("flag ON, disabled-adapter baseline: empty recall, no throw", async () => {
    const sources = await recallSarahGraphMemory({
      ownerUserId: OWNER,
      query: "release status",
      enabled: true,
      // no storeLayer -> SDK disabled adapter
    });
    expect(sources).toEqual([]);
  });

  test("flag ON with a seeded store: injects a bounded, redacted graph_memory slice", async () => {
    const layer = await seededLayer(OWNER, "release status latest candidate rctwentyone");
    const sources = await recallSarahGraphMemory({
      ownerUserId: OWNER,
      query: "What is our release status?",
      enabled: true,
      storeLayer: layer,
    });
    expect(sources.length).toBeGreaterThanOrEqual(1);
    const first = sources[0];
    expect(first?.kind).toBe(SARAH_GRAPH_MEMORY_KIND);
    expect(first?.sensitivity).toBe("owner_private");
    expect(first?.sourceRef.startsWith(SARAH_GRAPH_MEMORY_SOURCE_PREFIX)).toBe(true);
    expect(first?.summary.toLowerCase()).toContain("release status");
    // The advisory frames memory as untrusted reference, not an instruction.
    expect(first?.summary.toLowerCase()).toContain("untrusted");
  });

  test("respects the item bound", async () => {
    const layer = await seededLayer(OWNER, "release status candidate");
    const sources = await recallSarahGraphMemory({
      ownerUserId: OWNER,
      query: "release status candidate",
      enabled: true,
      storeLayer: layer,
      maxItems: 1,
    });
    expect(sources.length).toBeLessThanOrEqual(1);
  });

  test("owner isolation: a different owner reads nothing from another owner's graph", async () => {
    const layer = await seededLayer(OWNER, "release status candidate");
    const sources = await recallSarahGraphMemory({
      ownerUserId: "owner.other.9189",
      query: "release status candidate",
      enabled: true,
      storeLayer: layer,
    });
    expect(sources).toEqual([]);
  });

  test("secret-shaped material never enters a recall slice", async () => {
    const scope = sarahGraphMemoryScope(OWNER);
    // Build a real clean graph, then splice a secret-bearing element into the
    // returned snapshot to exercise the recall-side redaction backstop.
    const fixture = await Effect.runPromise(buildStoredGraph(scope, "release status clean fact"));
    const cleanMention = fixture.mention;
    const secretMention = {
      ...cleanMention,
      identity: {
        ...cleanMention.identity,
        canonicalKey: "release status AKIAIOSFODNN7EXAMPLE token",
      },
    };
    const current = {
      binding: {
        owner: scope.owner,
        project: scope.project,
        graphScopeRef: graphMemoryScopeRefFor(scope),
      },
      built: {
        snapshot: {
          mentions: [cleanMention, secretMention],
          entities: [],
          relations: [],
        },
      },
    };
    const sources = await recallSarahGraphMemory({
      ownerUserId: OWNER,
      query: "release status token",
      enabled: true,
      storeLayer: fakeInspectLayer(current),
    });
    const serialized = JSON.stringify(sources);
    expect(serialized).not.toContain("AKIAIOSFODNN7EXAMPLE");
    // The clean fact still recalls.
    expect(sources.some((source) => source.summary.toLowerCase().includes("clean fact"))).toBe(
      true,
    );
  });

  test("private-path material is dropped from a recall slice", async () => {
    const scope = sarahGraphMemoryScope(OWNER);
    const current = {
      binding: {
        owner: scope.owner,
        project: scope.project,
        graphScopeRef: graphMemoryScopeRefFor(scope),
      },
      built: {
        snapshot: {
          mentions: [
            {
              elementRef: "element.mention.secretpath",
              identity: { canonicalKey: "release status /Users/someone/.secrets/x.env" },
            },
          ],
          entities: [],
          relations: [],
        },
      },
    };
    const sources = await recallSarahGraphMemory({
      ownerUserId: OWNER,
      query: "release status",
      enabled: true,
      storeLayer: fakeInspectLayer(current),
    });
    expect(JSON.stringify(sources)).not.toContain(".secrets");
  });

  test("recall failure is fail-soft (defect in inspect yields an empty slice)", async () => {
    const brokenLayer = Layer.succeed(
      GraphMemoryStore,
      GraphMemoryStore.of({
        enabled: true,
        inspect: () => Effect.die(new Error("inspect exploded")),
        put: () => Effect.die("unused"),
        exportArchive: () => Effect.die("unused"),
        importArchive: () => Effect.die("unused"),
        applyDeletePlan: () => Effect.die("unused"),
        forget: () => Effect.die("unused"),
        recover: () => Effect.die("unused"),
      } as unknown as GraphMemoryStoreInterface),
    );
    const sources = await recallSarahGraphMemory({
      ownerUserId: OWNER,
      query: "release status",
      enabled: true,
      storeLayer: brokenLayer,
    });
    expect(sources).toEqual([]);
  });
});
