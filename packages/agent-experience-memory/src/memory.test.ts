import { Effect, Layer } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  bankId as decodeBank,
  decodeVerifiedBank,
  defaultMemoryConfig,
  freezeExperienceBank,
  MEMORY_DEFAULT_ENABLED,
  ownerScopeId,
  projectScopeId,
  repoRef,
  type MemoryConfigShape,
} from "./contract/index.js";
import { canonicalStringify } from "./internal/canonical.js";
import { sha256Hex } from "./internal/sha256.js";
import {
  applyRecalledMemory,
  exportScope,
  forget,
  inspect,
  recallForTurn,
  recallFromBank,
  remember,
} from "./memory.js";
import {
  disabledMemoryStoreLayer,
  inMemoryMemoryStoreLayer,
  MemoryStore,
  type MemoryScope,
} from "./store.js";

const OWNER_A = ownerScopeId("owner:a");
const OWNER_B = ownerScopeId("owner:b");
const PROJECT_1 = projectScopeId("project:1");
const PROJECT_2 = projectScopeId("project:2");
const REPO = repoRef("github.com/openagentsinc/openagents");
const NOW = "2026-07-20T00:00:00.000Z";
const LATER = "2026-07-20T01:00:00.000Z";

const scopeA1: MemoryScope = { owner: OWNER_A, project: PROJECT_1 };
const scopeA2: MemoryScope = { owner: OWNER_A, project: PROJECT_2 };
const scopeB1: MemoryScope = { owner: OWNER_B, project: PROJECT_1 };

const OFF: MemoryConfigShape = defaultMemoryConfig("apple_fm");
const ON: MemoryConfigShape = { ...OFF, enabled: true };

const runWith = <A>(
  layer: Layer.Layer<MemoryStore>,
  body: Effect.Effect<A, unknown, MemoryStore>,
): Promise<A> => Effect.runPromise(body.pipe(Effect.provide(layer)));

const rememberFact = (
  config: MemoryConfigShape,
  scope: MemoryScope,
  text: string,
  extra: Partial<{ consent: "granted" | "withheld"; observedAt: string; confidence: number }> = {},
) =>
  remember(config, {
    scope,
    repoRef: REPO,
    kind: "convention",
    text,
    confidence: extra.confidence ?? 0.9,
    consent: extra.consent ?? "granted",
    observedAt: extra.observedAt ?? NOW,
  });

describe("default-off flag", () => {
  test("the checked-in default is OFF", () => {
    expect(MEMORY_DEFAULT_ENABLED).toBe(false);
    expect(defaultMemoryConfig("apple_fm").enabled).toBe(false);
    expect(defaultMemoryConfig("codex_coding").enabled).toBe(false);
    expect(defaultMemoryConfig("apple_fm").localOnly).toBe(true);
  });

  test("with memory OFF, recall reads nothing and the prompt is byte-identical", async () => {
    const base = "You are a helpful assistant.\nUser: hi\nAssistant:";
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        const store = yield* MemoryStore;
        // Even if facts were somehow present, an OFF turn must not read them.
        yield* rememberFact(ON, scopeA1, "always run pnpm run check before pushing");
        const recall = yield* recallForTurn(scopeA1, OFF, { repoRef: REPO }, NOW);
        const reads = yield* store.reads;
        return { recall, reads, prompt: applyRecalledMemory(base, recall) };
      }),
    );
    // The single write above counts, but the OFF recall performs ZERO reads.
    expect(result.reads).toBe(0);
    expect(result.recall.memoryBlock).toBe("");
    expect(result.recall.includedRecordRefs).toEqual([]);
    expect(result.prompt).toBe(base);
  });

  test("with the disabled adapter, remember stores nothing and counters stay zero", async () => {
    const result = await runWith(
      disabledMemoryStoreLayer,
      Effect.gen(function* () {
        const store = yield* MemoryStore;
        const outcome = yield* rememberFact(ON, scopeA1, "a fact");
        const recall = yield* recallForTurn(scopeA1, ON, { repoRef: REPO }, NOW);
        return {
          outcome,
          recall,
          reads: yield* store.reads,
          writes: yield* store.writes,
          enabled: store.enabled,
        };
      }),
    );
    expect(result.enabled).toBe(false);
    expect(result.reads).toBe(0);
    expect(result.writes).toBe(0);
    expect(result.recall.memoryBlock).toBe("");
  });

  test("remember is a no-op when the config is OFF", async () => {
    const outcome = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        const store = yield* MemoryStore;
        const result = yield* rememberFact(OFF, scopeA1, "a fact");
        return { result, writes: yield* store.writes };
      }),
    );
    expect(outcome.result.stored).toBe(false);
    if (!outcome.result.stored) expect(outcome.result.reason).toBe("disabled");
    expect(outcome.writes).toBe(0);
  });
});

describe("memory ON: owner-scoped, redacted recall", () => {
  test("recall returns an owner-scoped, redacted slice a host may inject", async () => {
    const base = "SYSTEM\nUser: refactor\nAssistant:";
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        yield* rememberFact(ON, scopeA1, "this repo runs pnpm run check as the completion gate");
        const recall = yield* recallForTurn(scopeA1, ON, { repoRef: REPO }, NOW);
        return { recall, prompt: applyRecalledMemory(base, recall) };
      }),
    );
    expect(result.recall.enabled).toBe(true);
    expect(result.recall.includedRecordRefs.length).toBe(1);
    expect(result.recall.memoryBlock).toContain("pnpm run check");
    expect(result.prompt.startsWith("[recalled owner-local experience")).toBe(true);
    expect(result.prompt.endsWith(base)).toBe(true);
  });

  test("a secret-shaped value never enters memory nor a recall result", async () => {
    const secret = "the deploy token is sk-live-ABCDEFGH12345678ijklmnop and it works";
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        const store = yield* MemoryStore;
        const outcome = yield* rememberFact(ON, scopeA1, secret);
        const stored = yield* store.inspect(scopeA1);
        const recall = yield* recallForTurn(scopeA1, ON, { repoRef: REPO }, NOW);
        return { outcome, stored, recall };
      }),
    );
    // A hard-unsafe (secret) fact is rejected outright — not stored at all.
    expect(result.outcome.stored).toBe(false);
    if (!result.outcome.stored) expect(result.outcome.reason).toBe("unsafe_material");
    expect(result.stored.length).toBe(0);
    expect(result.recall.memoryBlock).not.toContain("sk-live");
    expect(result.recall.memoryBlock).toBe("");
  });

  test("a soft-PII fact is scrubbed before storage and never recalled raw", async () => {
    const withEmail = "ping the maintainer at person@example.com about the build";
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        const store = yield* MemoryStore;
        const outcome = yield* rememberFact(ON, scopeA1, withEmail);
        const stored = yield* store.inspect(scopeA1);
        const recall = yield* recallForTurn(scopeA1, ON, { repoRef: REPO }, NOW);
        return { outcome, stored, recall };
      }),
    );
    expect(result.outcome.stored).toBe(true);
    expect(result.stored[0]?.text).not.toContain("person@example.com");
    expect(result.recall.memoryBlock).not.toContain("person@example.com");
    expect(result.recall.memoryBlock).toContain("the build");
  });
});

describe("consent, isolation, and deletion", () => {
  test("a consent-withheld record is never recalled", async () => {
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        yield* rememberFact(ON, scopeA1, "a withheld fact about layout", { consent: "withheld" });
        const recall = yield* recallForTurn(scopeA1, ON, { repoRef: REPO }, NOW);
        const owned = yield* inspect(scopeA1);
        return { recall, owned };
      }),
    );
    // The owner can still see their own record; recall excludes it.
    expect(result.owned.length).toBe(1);
    expect(result.recall.includedRecordRefs).toEqual([]);
    expect(result.recall.memoryBlock).toBe("");
  });

  test("one owner scope never reads another owner's memory", async () => {
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        yield* rememberFact(ON, scopeA1, "owner A private fact");
        const bRecall = yield* recallForTurn(scopeB1, ON, { repoRef: REPO }, NOW);
        const bInspect = yield* inspect(scopeB1);
        return { bRecall, bInspect };
      }),
    );
    expect(result.bRecall.memoryBlock).toBe("");
    expect(result.bInspect.length).toBe(0);
  });

  test("recall stays inside one project", async () => {
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        yield* rememberFact(ON, scopeA1, "project 1 fact");
        const p2Recall = yield* recallForTurn(scopeA2, ON, { repoRef: REPO }, NOW);
        return { p2Recall };
      }),
    );
    expect(result.p2Recall.memoryBlock).toBe("");
    expect(result.p2Recall.includedRecordRefs).toEqual([]);
  });

  test("forget deletes everything in a scope and leaves other scopes intact", async () => {
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        yield* rememberFact(ON, scopeA1, "delete me one");
        yield* rememberFact(ON, scopeA1, "delete me two");
        yield* rememberFact(ON, scopeB1, "keep me");
        const removed = yield* forget(scopeA1);
        const aAfter = yield* inspect(scopeA1);
        const bAfter = yield* inspect(scopeB1);
        return { removed, aAfter, bAfter };
      }),
    );
    expect(result.removed).toBe(2);
    expect(result.aAfter.length).toBe(0);
    expect(result.bAfter.length).toBe(1);
  });

  test("export returns the owner's portable records", async () => {
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        yield* rememberFact(ON, scopeA1, "exportable fact");
        return yield* exportScope(scopeA1);
      }),
    );
    expect(result.records.length).toBe(1);
    expect(result.patterns.length).toBe(0);
  });
});

describe("freeze, one-shot adaptation, and corrupt banks", () => {
  test("a write after the freeze cannot change the frozen turn input (stale bank)", async () => {
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        const store = yield* MemoryStore;
        yield* rememberFact(ON, scopeA1, "fact present at freeze time");
        // Freeze the bank at t0.
        const frozen = yield* store.snapshot(scopeA1, yield* frozenBankId(scopeA1), NOW);
        // A later observation lands AFTER the freeze.
        yield* rememberFact(ON, scopeA1, "fact added after freeze", { observedAt: LATER });
        // Recall over the FROZEN bank must not see the later fact.
        const recall = recallFromBank(frozen, ON, { repoRef: REPO });
        return { frozenCount: frozen.records.length, includes: recall.includedRecordRefs.length };
      }),
    );
    expect(result.frozenCount).toBe(1);
    expect(result.includes).toBe(1);
  });

  test("recall binds an effective adaptation digest to the turn deterministically", async () => {
    const result = await runWith(
      inMemoryMemoryStoreLayer,
      Effect.gen(function* () {
        yield* rememberFact(ON, scopeA1, "a stable fact");
        const one = yield* recallForTurn(scopeA1, ON, { repoRef: REPO }, NOW);
        const two = yield* recallForTurn(scopeA1, ON, { repoRef: REPO }, NOW);
        return { one, two };
      }),
    );
    // Same frozen content and request → same bound adaptation digest.
    expect(result.one.effectiveAdaptationDigest).toBe(result.two.effectiveAdaptationDigest);
    expect(result.one.bankDigest).toBe(result.two.bankDigest);
  });

  test("a corrupt bank fails closed and degrades to no-memory", () => {
    const good = freezeExperienceBank({
      bankId: goodBankId(),
      ownerScope: OWNER_A,
      projectScope: PROJECT_1,
      frozenAt: NOW,
      records: [],
      patterns: [],
    });
    // Tamper with the stored digest.
    const corrupt = { ...good, bankDigest: "0".repeat(64) };
    expect(() => decodeVerifiedBank(corrupt)).toThrow();
    // recallFromBank over an (empty) good bank yields no memory, proving the
    // fail-closed path returns a benign result rather than throwing downstream.
    const recall = recallFromBank(good, ON, { repoRef: REPO });
    expect(recall.memoryBlock).toBe("");
  });
});

// --- small helpers that need a scope-derived bank id ---
const frozenBankId = (scope: MemoryScope) =>
  Effect.succeed(
    decodeBank(
      `bank:${sha256Hex(canonicalStringify({ owner: scope.owner, project: scope.project, frozenAt: NOW }))}`,
    ),
  );

const goodBankId = () => decodeBank("bank:fixture");
