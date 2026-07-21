import { Effect, Layer } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  applyOwnerProfile,
  disabledOwnerProfileStoreLayer,
  guardOwnerFact,
  inMemoryOwnerProfileStoreLayer,
  ownerProfileScopeForNpub,
  OwnerProfileStore,
  projectOwnerProfileBlock,
  type OwnerProfileFact,
  type OwnerProfileScope,
} from "./index.js";

const NOW = "2026-07-20T00:00:00Z";
const scopeA: OwnerProfileScope = ownerProfileScopeForNpub("npub1aaa");
const scopeB: OwnerProfileScope = ownerProfileScopeForNpub("npub1bbb");

const runWith = <A, E>(
  layer: Layer.Layer<OwnerProfileStore>,
  effect: Effect.Effect<A, E, OwnerProfileStore>,
): Promise<A> => Effect.runPromise(effect.pipe(Effect.provide(layer)));

const facts = (entries: ReadonlyArray<[OwnerProfileFact["category"], string]>): OwnerProfileFact[] =>
  entries.map(([category, value]) => ({ category, value, statedAt: NOW }));

describe("guardOwnerFact — the ATIF boundary rejects hard-unsafe values", () => {
  test("a plain owner fact is stored as-is", () => {
    const guard = guardOwnerFact("role", "founder and CEO", NOW);
    expect(guard.ok).toBe(true);
    if (guard.ok) expect(guard.fact.value).toBe("founder and CEO");
  });

  test("a hard-unsafe value (provider key) is rejected, never stored", () => {
    const guard = guardOwnerFact("preference", "my key is sk-live-abc123def456ghi789", NOW);
    expect(guard.ok).toBe(false);
  });

  test("a local path is rejected as hard-unsafe", () => {
    const guard = guardOwnerFact("project", "notes live in /Users/chris/private/notes", NOW);
    expect(guard.ok).toBe(false);
  });

  test("soft PII (an email) is scrubbed but the fact is kept", () => {
    const guard = guardOwnerFact("handle", "reach me at person@example.com", NOW);
    expect(guard.ok).toBe(true);
    if (guard.ok) expect(guard.fact.value).not.toContain("person@example.com");
  });
});

describe("projectOwnerProfileBlock — default-off and honest", () => {
  test("default-off returns the empty string", () => {
    expect(projectOwnerProfileBlock(facts([["name", "Chris"]]))).toBe("");
    expect(projectOwnerProfileBlock(facts([["name", "Chris"]]), { enabled: false })).toBe("");
  });

  test("enabled with facts renders a cited block of exactly the stored facts", () => {
    const block = projectOwnerProfileBlock(
      facts([
        ["name", "Chris"],
        ["role", "founder"],
        ["project", "OpenAgents"],
      ]),
      { enabled: true },
    );
    expect(block).toContain("Owner profile");
    expect(block).toContain("Name: Chris");
    expect(block).toContain("Role: founder");
    expect(block).toContain("Project: OpenAgents");
  });

  test("enabled but empty says so and does not invent", () => {
    const block = projectOwnerProfileBlock([], { enabled: true });
    expect(block).toContain("No stored owner profile facts");
    expect(block).toContain("do not invent");
  });

  test("the block is bounded by the char budget", () => {
    const many = facts(
      Array.from({ length: 40 }, (_, i) => ["preference", `preference number ${i} is a long stated line`] as const),
    );
    const block = projectOwnerProfileBlock(many, { enabled: true, budgetChars: 200 });
    expect(block.length).toBeLessThanOrEqual(240);
    expect(block).toContain("truncated to budget");
  });
});

describe("applyOwnerProfile — default-off is a byte-identical no-op", () => {
  const base = "You are the on-device assistant.\n\n## Ambient context\n- date: 2026-07-20";

  test("with the profile off the base prompt is returned unchanged", () => {
    expect(applyOwnerProfile(base, facts([["name", "Chris"]]))).toBe(base);
    expect(applyOwnerProfile(base, facts([["name", "Chris"]]), { enabled: false })).toBe(base);
  });

  test("with the profile on the block is appended", () => {
    const out = applyOwnerProfile(base, facts([["name", "Chris"]]), { enabled: true });
    expect(out.startsWith(base)).toBe(true);
    expect(out).toContain("Name: Chris");
  });
});

describe("OwnerProfileStore — owner-scoped, redacted, inspectable, forgettable", () => {
  test("put then inspect returns the owner's facts", async () => {
    const stored = await runWith(
      inMemoryOwnerProfileStoreLayer,
      Effect.gen(function* () {
        const store = yield* OwnerProfileStore;
        yield* store.put(scopeA, "name", "Chris", NOW);
        yield* store.put(scopeA, "role", "founder", NOW);
        return yield* store.inspect(scopeA);
      }),
    );
    expect(stored.map((f) => f.category)).toEqual(["name", "role"]);
  });

  test("a hard-unsafe value is rejected by the store, nothing is written", async () => {
    const result = await runWith(
      inMemoryOwnerProfileStoreLayer,
      Effect.gen(function* () {
        const store = yield* OwnerProfileStore;
        const put = yield* store
          .put(scopeA, "preference", "token sk-live-abc123def456ghi789", NOW)
          .pipe(
            Effect.map(() => "stored" as const),
            Effect.catch(() => Effect.succeed("rejected" as const)),
          );
        const stored = yield* store.inspect(scopeA);
        const writes = yield* store.writes;
        return { put, stored, writes };
      }),
    );
    expect(result.put).toBe("rejected");
    expect(result.stored.length).toBe(0);
    expect(result.writes).toBe(0);
  });

  test("one owner scope never reads another owner's profile", async () => {
    const result = await runWith(
      inMemoryOwnerProfileStoreLayer,
      Effect.gen(function* () {
        const store = yield* OwnerProfileStore;
        yield* store.put(scopeA, "name", "Chris", NOW);
        const a = yield* store.inspect(scopeA);
        const b = yield* store.inspect(scopeB);
        return { a, b };
      }),
    );
    expect(result.a.length).toBe(1);
    expect(result.b.length).toBe(0);
  });

  test("forget removes a category, or the whole profile", async () => {
    const result = await runWith(
      inMemoryOwnerProfileStoreLayer,
      Effect.gen(function* () {
        const store = yield* OwnerProfileStore;
        yield* store.put(scopeA, "name", "Chris", NOW);
        yield* store.put(scopeA, "project", "OpenAgents", NOW);
        yield* store.put(scopeA, "project", "Effect Native", NOW);
        const removedProjects = yield* store.forget(scopeA, "project");
        const afterCategory = yield* store.inspect(scopeA);
        const removedAll = yield* store.forget(scopeA);
        const afterAll = yield* store.inspect(scopeA);
        return { removedProjects, afterCategory, removedAll, afterAll };
      }),
    );
    expect(result.removedProjects).toBe(2);
    expect(result.afterCategory.map((f) => f.category)).toEqual(["name"]);
    expect(result.removedAll).toBe(1);
    expect(result.afterAll.length).toBe(0);
  });
});

describe("disabled store proves profile-off touches nothing", () => {
  test("the disabled adapter never reads or writes and inspect is empty", async () => {
    const result = await runWith(
      disabledOwnerProfileStoreLayer,
      Effect.gen(function* () {
        const store = yield* OwnerProfileStore;
        expect(store.enabled).toBe(false);
        const put = yield* store.put(scopeA, "name", "Chris", NOW).pipe(
          Effect.map(() => "stored" as const),
          Effect.catch(() => Effect.succeed("rejected" as const)),
        );
        const stored = yield* store.inspect(scopeA);
        const reads = yield* store.reads;
        const writes = yield* store.writes;
        return { put, stored, reads, writes };
      }),
    );
    expect(result.put).toBe("rejected");
    expect(result.stored.length).toBe(0);
    expect(result.reads).toBe(0);
    expect(result.writes).toBe(0);
  });
});
