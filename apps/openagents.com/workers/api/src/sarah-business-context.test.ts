import {
  GraphMemoryBinding,
  GraphMemoryOperationRef,
  GraphMemoryStore,
  graphMemoryScopeRefFor,
  graphMemoryStoreLayer,
  makeInMemoryGraphMemoryStateStore,
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
import type { SyncSql } from "@openagentsinc/khala-sync-server";
import { Effect, Schema } from "effect";
import { describe, expect, test } from "vitest";

import { collectSarahBusinessContext } from "./sarah-business-context";
import { sarahGraphMemoryScope } from "./sarah-graph-memory";

const timestamp = "2026-07-18T15:00:00.000Z";

const sql = (async (strings: TemplateStringsArray) => {
  const query = strings.join(" ");
  if (query.includes("khala_sync_chat_messages")) {
    return [
      {
        source_ref: "source.sarah.message.message.fixture",
        summary: "What is our release status?",
        observed_at: timestamp,
      },
    ];
  }
  if (query.includes("desktop_full_auto_run_projections")) {
    return [
      {
        run_ref: "run.fixture",
        objective: "Finish Full Auto",
        lifecycle_state: "running",
        updated_at: timestamp,
      },
    ];
  }
  if (query.includes("sarah_fleet_run_requests")) {
    return [
      {
        run_ref: "fleet.fixture",
        status: "claimed_by_pylon",
        worker_kind: "auto",
        updated_at: timestamp,
      },
    ];
  }
  if (query.includes("forum_posts")) {
    return [
      {
        post_id: "post.fixture",
        actor_ref: "tester.fixture",
        topic_title: "Release candidates",
        body_text: "The latest candidate launches.",
        updated_at: timestamp,
      },
    ];
  }
  return [];
}) as unknown as SyncSql;

const fetchFixture: typeof fetch = async (input) => {
  const url = String(input);
  if (url.endsWith("/releases/latest")) {
    return Response.json({
      html_url: "https://github.com/OpenAgentsInc/openagents/releases/tag/v0.1.0-rc.21",
      name: "RC21",
      published_at: timestamp,
      tag_name: "v0.1.0-rc.21",
    });
  }
  if (url.includes("/issues?")) {
    return Response.json([
      {
        html_url: "https://github.com/OpenAgentsInc/openagents/issues/9001",
        number: 9001,
        title: "Full Auto runtime",
        updated_at: timestamp,
      },
    ]);
  }
  return Response.json({ ok: true });
};

describe("Sarah business context", () => {
  test("collects bounded cited conversation, release, issue, Forum, Full Auto, fleet, health, and contract sources", async () => {
    const context = await collectSarahBusinessContext({
      sql,
      ownerUserId: "owner.fixture",
      threadRef: "thread.sarah.0123456789abcdef01234567",
      fetch: fetchFixture,
      now: () => new Date(timestamp),
    });
    expect(new Set(context.sources.map((source) => source.kind))).toEqual(
      new Set([
        "conversation",
        "github_release",
        "github_issue",
        "forum",
        "full_auto",
        "fleet",
        "cloud_health",
        "product_contract",
      ]),
    );
    expect(context.sources.length).toBeLessThanOrEqual(32);
    expect(context.sources.every((source) => source.sourceRef.startsWith("source."))).toBe(true);
    expect(JSON.stringify(context)).not.toContain("private-password");
  });

  test("fails soft when optional live sources are unavailable", async () => {
    const context = await collectSarahBusinessContext({
      sql: (async () => {
        throw new Error("unavailable");
      }) as unknown as SyncSql,
      ownerUserId: "owner.fixture",
      threadRef: "thread.sarah.0123456789abcdef01234567",
      fetch: async () => {
        throw new Error("unavailable");
      },
      now: () => new Date(timestamp),
    });
    expect(context.sources).toHaveLength(1);
    expect(context.sources[0]?.kind).toBe("product_contract");
  });

  test("graph-memory recall is off by default: no graph_memory source, zero behavior change", async () => {
    const context = await collectSarahBusinessContext({
      sql,
      ownerUserId: "owner.fixture",
      threadRef: "thread.sarah.0123456789abcdef01234567",
      fetch: fetchFixture,
      now: () => new Date(timestamp),
    });
    expect(
      context.sources.some((source) => source.sourceRef.startsWith("source.graph_memory.")),
    ).toBe(false);

    const explicitOff = await collectSarahBusinessContext({
      sql,
      ownerUserId: "owner.fixture",
      threadRef: "thread.sarah.0123456789abcdef01234567",
      fetch: fetchFixture,
      now: () => new Date(timestamp),
      graphMemoryRecall: { enabled: false, query: "release status" },
    });
    // Identical context whether the recall config is absent or explicitly off.
    // (cloud_health stamps a live timestamp, so compare the stable fields.)
    const stable = (sources: typeof context.sources) =>
      sources.map((source) => ({
        sourceRef: source.sourceRef,
        kind: source.kind,
        summary: source.summary,
      }));
    expect(stable(explicitOff.sources)).toEqual(stable(context.sources));
  });

  test("graph-memory recall on: injects a bounded redacted graph_memory source", async () => {
    const ownerUserId = "owner.fixture";
    const scope = sarahGraphMemoryScope(ownerUserId);
    const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const layer = graphMemoryStoreLayer(stateStore);
    const digest = (value: unknown) => graphDigest(sha256Hex(canonicalJson(value)));
    await Effect.runPromise(
      Effect.gen(function* () {
        const source = Schema.decodeUnknownSync(GraphSourceMembership)({
          source: {
            sourcePlane: "repository",
            sourceKind: "sarah-context-test",
            sourceAddress: {
              addressSchemaId: "openagents.test.repository_address.v1",
              encodedAddress: "memory://sarah-context",
            },
            corpusRef: "corpus.sarah-context",
            contentDigest: digest({ corpus: "sarah-context" }),
            entryRef: "entry.sarah-context",
          },
        }).source;
        const derivation = Schema.decodeUnknownSync(GraphDerivation)({
          _tag: "Deterministic",
          parserRef: "parser.sarah-context",
          parserVersion: "version.1",
        });
        const mention = makeGraphMention({
          identityNamespace: "sarah-context-test",
          canonicalKey: "release status latest candidate rctwentyone",
          source,
          derivation,
        });
        const policy = Schema.decodeUnknownSync(GraphCorpusPolicy)({
          includeVisibilities: ["private"],
          includeRedactionClasses: ["redacted"],
        });
        const built = yield* buildGraphCorpus({
          graphRef: "graph.sarah-context-test",
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
        const store = yield* GraphMemoryStore;
        yield* store.put({
          operationRef: Schema.decodeUnknownSync(GraphMemoryOperationRef)("op.sarah-context.put.1"),
          binding,
          admission: {
            consent: "granted",
            consentRef: "consent.sarah-context.test",
            policyRef: "policy.sarah-context.test",
            redactionState: "already_redacted",
          },
          built,
          artifactInventory,
        });
      }).pipe(Effect.provide(layer)),
    );

    const context = await collectSarahBusinessContext({
      sql,
      ownerUserId,
      threadRef: "thread.sarah.0123456789abcdef01234567",
      fetch: fetchFixture,
      now: () => new Date(timestamp),
      graphMemoryRecall: {
        enabled: true,
        query: "What is our release status?",
        storeLayer: layer,
      },
    });
    const memorySources = context.sources.filter((source) =>
      source.sourceRef.startsWith("source.graph_memory."),
    );
    expect(memorySources.length).toBeGreaterThanOrEqual(1);
    expect(memorySources[0]?.kind).toBe("memory");
    expect(memorySources[0]?.sensitivity).toBe("owner_private");
    expect(memorySources[0]?.summary.toLowerCase()).toContain("release status");
    expect(context.sources.length).toBeLessThanOrEqual(32);
  });
});
