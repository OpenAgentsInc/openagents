import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import {
  GraphMemoryBinding,
  GraphMemoryOperationRef,
  GraphMemoryStore,
  GRAPH_MEMORY_STATE_SCHEMA_ID,
  graphMemoryScopeRefFor,
  ownerScopeId,
  projectScopeId,
  type GraphMemoryScope,
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
import { Effect, Schema } from "effect";
import { afterEach, describe, expect, test } from "vite-plus/test";

import { openDesktopGraphMemoryStore } from "./desktop-graph-memory-store.js";
import type { SafeStorageLike } from "./desktop-session-vault.js";

const roots: Array<string> = [];
const temporaryRoot = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-graph-store-"));
  roots.push(root);
  return root;
};

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { force: true, recursive: true });
});

const safeStorage = (): SafeStorageLike => ({
  isEncryptionAvailable: () => true,
  getSelectedStorageBackend: () => "keychain_access",
  encryptString: (plaintext) => Buffer.from(`wrapped:${plaintext}`, "utf8"),
  decryptString: (encrypted) => encrypted.toString("utf8").slice("wrapped:".length),
});

const scope = (owner: string, project: string): GraphMemoryScope => ({
  owner: ownerScopeId(owner),
  project: projectScopeId(project),
});

const envelope = (selected: GraphMemoryScope, revision: number) => ({
  schemaId: GRAPH_MEMORY_STATE_SCHEMA_ID,
  scope: selected,
  revision,
  receipts: [],
});

const digest = (value: unknown) => graphDigest(sha256Hex(canonicalJson(value)));
const operationRef = Schema.decodeUnknownSync(GraphMemoryOperationRef);

const graphFixture = (selected: GraphMemoryScope) =>
  Effect.gen(function* () {
    const source = Schema.decodeUnknownSync(GraphSourceMembership)({
      source: {
        sourcePlane: "repository",
        sourceKind: "desktop-graph-memory-test",
        sourceAddress: {
          addressSchemaId: "openagents.test.repository_address.v1",
          encodedAddress: "memory://desktop-graph",
        },
        corpusRef: "corpus.desktop-graph",
        contentDigest: digest({ corpus: "desktop-graph" }),
        entryRef: "entry.desktop-graph",
      },
    }).source;
    const derivation = Schema.decodeUnknownSync(GraphDerivation)({
      _tag: "Deterministic",
      parserRef: "parser.desktop-graph",
      parserVersion: "version.1",
    });
    const mention = makeGraphMention({
      identityNamespace: "desktop-test",
      canonicalKey: "durable-memory",
      source,
      derivation,
    });
    const policy = Schema.decodeUnknownSync(GraphCorpusPolicy)({
      includeVisibilities: ["private"],
      includeRedactionClasses: ["redacted"],
    });
    const built = yield* buildGraphCorpus({
      graphRef: "graph.desktop-memory-test",
      scopeRef: graphMemoryScopeRefFor(selected),
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
      owner: selected.owner,
      project: selected.project,
      graphScopeRef: built.snapshot.scopeRef,
      sourceBindings: [{ corpusRef: source.corpusRef, contentDigest: source.contentDigest }],
      graphRef: built.snapshot.graphRef,
      graphDigest: built.snapshot.graphDigest,
      manifestDigest: built.manifest.manifestDigest,
      policyDigest: digest(built.snapshot.policy),
      generation: 1,
    });
    return { built, artifactInventory, binding };
  });

describe("Desktop graph-memory store composition", () => {
  test("disabled composition leaves the portable state port at zero I/O", async () => {
    let opened = false;
    const desktop = openDesktopGraphMemoryStore({
      enabled: false,
      databasePath: "/must/not/exist/graph-memory.sqlite",
      safeStorage: safeStorage(),
      openDatabase: () => {
        opened = true;
        throw new Error("must not open");
      },
    });
    const selected = scope("owner.disabled", "project.disabled");

    await expect(Effect.runPromise(desktop.stateStore.load(selected))).resolves.toBeNull();
    await expect(
      Effect.runPromise(desktop.stateStore.compareAndSet(selected, null, envelope(selected, 0))),
    ).resolves.toBe(false);
    await expect(Effect.runPromise(desktop.stateStore.reads)).resolves.toBe(0);
    await expect(Effect.runPromise(desktop.stateStore.writes)).resolves.toBe(0);
    expect(opened).toBe(false);
    desktop.close();
  });

  test("persists opaque envelopes with atomic revision comparison and scope isolation", async () => {
    const databasePath = path.join(temporaryRoot(), "graph-memory.sqlite");
    const desktop = openDesktopGraphMemoryStore({
      enabled: true,
      databasePath,
      safeStorage: safeStorage(),
    });
    const selected = scope("owner.a", "project.a");
    const anotherProject = scope("owner.a", "project.b");
    const initial = envelope(selected, 0);
    const next = envelope(selected, 1);

    await expect(
      Effect.runPromise(desktop.stateStore.compareAndSet(selected, null, initial)),
    ).resolves.toBe(true);
    await expect(
      Effect.runPromise(desktop.stateStore.compareAndSet(selected, null, next)),
    ).resolves.toBe(false);
    await expect(
      Effect.runPromise(desktop.stateStore.compareAndSet(selected, 0, next)),
    ).resolves.toBe(true);
    await expect(Effect.runPromise(desktop.stateStore.load(anotherProject))).resolves.toBeNull();
    desktop.close();

    const reopened = openDesktopGraphMemoryStore({
      enabled: true,
      databasePath,
      safeStorage: safeStorage(),
    });
    await expect(Effect.runPromise(reopened.stateStore.load(selected))).resolves.toEqual(next);
    reopened.close();
  });

  test("runs inspect, archive, restart, and full forget on the encrypted adapter", async () => {
    const databasePath = path.join(temporaryRoot(), "graph-memory.sqlite");
    const selected = scope("owner.lifecycle", "project.lifecycle");
    const fixture = await Effect.runPromise(graphFixture(selected));
    const first = openDesktopGraphMemoryStore({
      enabled: true,
      databasePath,
      safeStorage: safeStorage(),
    });
    const putReceipt = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* GraphMemoryStore;
        return yield* store.put({
          operationRef: operationRef("operation.desktop.put"),
          binding: fixture.binding,
          admission: {
            consent: "granted",
            consentRef: "consent.desktop-memory",
            policyRef: "policy.desktop-memory",
            redactionState: "already_redacted",
          },
          built: fixture.built,
          artifactInventory: fixture.artifactInventory,
        });
      }).pipe(Effect.provide(first.layer)),
    );
    expect(putReceipt.status).toBe("complete");
    first.close();

    const second = openDesktopGraphMemoryStore({
      enabled: true,
      databasePath,
      safeStorage: safeStorage(),
    });
    const lifecycle = await Effect.runPromise(
      Effect.gen(function* () {
        const store = yield* GraphMemoryStore;
        const before = yield* store.inspect(selected);
        const exported = yield* store.exportArchive(
          selected,
          operationRef("operation.desktop.export"),
        );
        const forgotten = yield* store.forget(selected, operationRef("operation.desktop.forget"));
        const after = yield* store.inspect(selected);
        const repeated = yield* store.forget(selected, operationRef("operation.desktop.forget"));
        return { before, exported, forgotten, after, repeated };
      }).pipe(Effect.provide(second.layer)),
    );
    expect(lifecycle.before.current?.binding.graphDigest).toBe(fixture.built.snapshot.graphDigest);
    expect(lifecycle.exported.bytes.byteLength).toBeGreaterThan(0);
    expect(lifecycle.forgotten.before.mentions).toBe(1);
    expect(lifecycle.forgotten.after).toMatchObject({
      mentions: 0,
      vectors: 0,
      summaries: 0,
      rankingRefs: 0,
      rankingSnapshots: 0,
      archives: 0,
    });
    expect(lifecycle.after.current).toBeNull();
    expect(lifecycle.repeated.receiptRef).toBe(lifecycle.forgotten.receiptRef);
    second.close();
  });
});
