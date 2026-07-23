import {
  GraphMemoryStore,
  graphMemoryStoreLayer,
  makeInMemoryGraphMemoryStateStore,
  type GraphMemoryStateStore,
  type GraphMemoryStoreInterface,
} from "@openagentsinc/agent-experience-memory";
import { Effect, Layer } from "effect";
import { describe, expect, test } from "vitest";

import { recallSarahGraphMemory, sarahGraphMemoryScope } from "./sarah-graph-memory";
import { persistSarahGraphMemoryTurn } from "./sarah-graph-memory-writeback";

const OWNER = "owner.9189.writeback.fixture";

/** Read the raw stored canonical keys for a scope over a fresh store instance. */
const storedFacts = async (
  stateStore: GraphMemoryStateStore,
  ownerUserId: string,
): Promise<ReadonlyArray<string>> => {
  const scope = sarahGraphMemoryScope(ownerUserId);
  const layer = graphMemoryStoreLayer(stateStore);
  return Effect.runPromise(
    Effect.gen(function* () {
      const store = yield* GraphMemoryStore;
      const inspection = yield* store.inspect(scope);
      const current = inspection.current;
      if (current === null) return [] as ReadonlyArray<string>;
      return current.built.snapshot.mentions.map((mention) => mention.identity.canonicalKey);
    }).pipe(Effect.provide(layer)),
  );
};

describe("persistSarahGraphMemoryTurn — flag gate", () => {
  test("flag OFF: no store opened, nothing written, byte-identical", async () => {
    const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const outcome = await persistSarahGraphMemoryTurn({
      assistantMessage: "The current release candidate is rctwentyone.",
      enabled: false,
      ownerUserId: OWNER,
      storeLayer: graphMemoryStoreLayer(stateStore),
      turnId: "turn.off.1",
      userMessage: "What is our release status?",
    });
    expect(outcome).toEqual({ _tag: "disabled" });
    expect(await Effect.runPromise(stateStore.reads)).toBe(0);
    expect(await Effect.runPromise(stateStore.writes)).toBe(0);
  });

  test("flag ON but disabled backing adapter: no-op store_disabled", async () => {
    const outcome = await persistSarahGraphMemoryTurn({
      assistantMessage: "answer",
      enabled: true,
      ownerUserId: OWNER,
      // no storeLayer -> SDK disabled adapter
      turnId: "turn.disabled.1",
      userMessage: "question",
    });
    expect(outcome).toEqual({ _tag: "store_disabled" });
  });
});

describe("persistSarahGraphMemoryTurn — end-to-end recall loop", () => {
  test("stored facts are recalled by a fresh store instance", async () => {
    const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const layer = graphMemoryStoreLayer(stateStore);
    const outcome = await persistSarahGraphMemoryTurn({
      assistantMessage: "The release candidate is at stage rctwentyone this week.",
      enabled: true,
      ownerUserId: OWNER,
      storeLayer: layer,
      turnId: "turn.e2e.1",
      userMessage: "What is our release status right now?",
    });
    expect(outcome._tag).toBe("stored");
    if (outcome._tag === "stored") {
      expect(outcome.factCount).toBeGreaterThanOrEqual(1);
      expect(outcome.generation).toBe(1);
    }

    // A FRESH store instance over the same durable state proves persistence.
    const recallLayer = graphMemoryStoreLayer(stateStore);
    const sources = await recallSarahGraphMemory({
      enabled: true,
      ownerUserId: OWNER,
      query: "release candidate status",
      storeLayer: recallLayer,
    });
    expect(sources.length).toBeGreaterThanOrEqual(1);
    expect(sources[0]?.summary.toLowerCase()).toContain("release");
    expect(sources[0]?.summary.toLowerCase()).toContain("untrusted");
  });

  test("accumulates across turns and increments generation", async () => {
    const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const layer = graphMemoryStoreLayer(stateStore);
    const first = await persistSarahGraphMemoryTurn({
      assistantMessage: "Our primary launch target is the mobile app.",
      enabled: true,
      ownerUserId: OWNER,
      storeLayer: layer,
      turnId: "turn.acc.1",
      userMessage: "Remind me our launch target.",
    });
    expect(first._tag).toBe("stored");
    const second = await persistSarahGraphMemoryTurn({
      assistantMessage: "The desktop build ships after the mobile launch.",
      enabled: true,
      ownerUserId: OWNER,
      storeLayer: layer,
      turnId: "turn.acc.2",
      userMessage: "When does desktop ship?",
    });
    expect(second._tag).toBe("stored");
    if (second._tag === "stored") expect(second.generation).toBe(2);

    const facts = await storedFacts(stateStore, OWNER);
    // Both turns' facts are present — memory accumulated, not replaced.
    expect(facts.some((fact) => fact.toLowerCase().includes("launch target"))).toBe(true);
    expect(facts.some((fact) => fact.toLowerCase().includes("desktop build"))).toBe(true);
  });

  test("no material delta: re-running the same turn does not churn a generation", async () => {
    const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const layer = graphMemoryStoreLayer(stateStore);
    const input = {
      assistantMessage: "The plan is unchanged.",
      enabled: true as const,
      ownerUserId: OWNER,
      storeLayer: layer,
      turnId: "turn.delta.1",
      userMessage: "Any change to the plan?",
    };
    const first = await persistSarahGraphMemoryTurn(input);
    expect(first._tag).toBe("stored");
    // A second turn whose facts are all already stored yields no delta.
    const second = await persistSarahGraphMemoryTurn({
      ...input,
      turnId: "turn.delta.2",
    });
    expect(second).toEqual({ _tag: "no_material_delta" });
  });
});

describe("persistSarahGraphMemoryTurn — redaction", () => {
  test("secret / path material never lands in the store; a clean fact still does", async () => {
    const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const layer = graphMemoryStoreLayer(stateStore);
    const outcome = await persistSarahGraphMemoryTurn({
      // A secret-bearing user message must be dropped wholesale.
      assistantMessage: "The release candidate is ready to ship.",
      enabled: true,
      ownerUserId: OWNER,
      storeLayer: layer,
      turnId: "turn.redact.1",
      userMessage: "Use AKIAIOSFODNN7EXAMPLE and read /Users/someone/.secrets/x.env to deploy",
    });
    expect(outcome._tag).toBe("stored");
    const facts = await storedFacts(stateStore, OWNER);
    const serialized = JSON.stringify(facts);
    expect(serialized).not.toContain("AKIAIOSFODNN7EXAMPLE");
    expect(serialized).not.toContain(".secrets");
    // The clean assistant fact still accumulated.
    expect(facts.some((fact) => fact.toLowerCase().includes("release candidate"))).toBe(true);
  });

  test("a soft email is scrubbed but the fact is still stored", async () => {
    const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const layer = graphMemoryStoreLayer(stateStore);
    const outcome = await persistSarahGraphMemoryTurn({
      assistantMessage: "Ping teammate@example.com about the milestone review.",
      enabled: true,
      ownerUserId: OWNER,
      storeLayer: layer,
      turnId: "turn.email.1",
      userMessage: "Who do I ping about the review?",
    });
    expect(outcome._tag).toBe("stored");
    const facts = await storedFacts(stateStore, OWNER);
    const serialized = JSON.stringify(facts);
    expect(serialized).not.toContain("teammate@example.com");
    // The redacted fact around the email is still stored.
    expect(facts.some((fact) => fact.toLowerCase().includes("milestone review"))).toBe(true);
  });
});

describe("persistSarahGraphMemoryTurn — isolation & fail-soft", () => {
  test("owner isolation: another owner reads nothing", async () => {
    const stateStore = await Effect.runPromise(makeInMemoryGraphMemoryStateStore());
    const layer = graphMemoryStoreLayer(stateStore);
    await persistSarahGraphMemoryTurn({
      assistantMessage: "Our runway is healthy through the release.",
      enabled: true,
      ownerUserId: OWNER,
      storeLayer: layer,
      turnId: "turn.iso.1",
      userMessage: "What is our runway?",
    });
    const otherFacts = await storedFacts(stateStore, "owner.other.9189.writeback");
    expect(otherFacts).toEqual([]);
    const otherRecall = await recallSarahGraphMemory({
      enabled: true,
      ownerUserId: "owner.other.9189.writeback",
      query: "runway release",
      storeLayer: graphMemoryStoreLayer(stateStore),
    });
    expect(otherRecall).toEqual([]);
  });

  test("fail-soft: a store put error yields a diagnostic and never throws", async () => {
    const brokenLayer: Layer.Layer<GraphMemoryStore> = Layer.succeed(
      GraphMemoryStore,
      GraphMemoryStore.of({
        enabled: true,
        inspect: () =>
          Effect.succeed({
            current: null,
            enabled: true,
            pendingOperationRef: null,
            receipts: [],
            revision: 0,
            scope: sarahGraphMemoryScope(OWNER),
          }),
        put: () => Effect.die(new Error("put exploded")),
        exportArchive: () => Effect.die("unused"),
        importArchive: () => Effect.die("unused"),
        applyDeletePlan: () => Effect.die("unused"),
        forget: () => Effect.die("unused"),
        recover: () => Effect.die("unused"),
      } as unknown as GraphMemoryStoreInterface),
    );
    const outcome = await persistSarahGraphMemoryTurn({
      assistantMessage: "answer text",
      enabled: true,
      ownerUserId: OWNER,
      storeLayer: brokenLayer,
      turnId: "turn.failsoft.1",
      userMessage: "question text",
    });
    expect(outcome._tag).toBe("failed");
  });
});
