import { describe, expect, test } from "vite-plus/test";

import {
  buildSubagentMemoryContext,
  DEFAULT_SUBAGENT_MEMORY_BUDGET_TOKENS,
  HARVEST_LEDGER_SCHEMA_ID,
  harvestSubagentOutcome,
  ledgerEntriesAsHeuristics,
  SUBAGENT_MEMORY_HEADER,
} from "./subagent-memory.js";

const OWNER = "owner/test";
const PROJECT = "project/test";

describe("buildSubagentMemoryContext", () => {
  const heuristics = [
    { ref: "h1", text: "run the typecheck before pushing", confidence: 0.9 },
    { ref: "h2", text: "restart the gateway when the queue stalls", confidence: 0.7 },
    { ref: "h3", text: "prefer small pull requests over large ones", confidence: 0.4 },
  ];

  test("packs heuristics into a bounded advisory block", () => {
    const ctx = buildSubagentMemoryContext({ heuristics });
    expect(ctx.block.startsWith(SUBAGENT_MEMORY_HEADER)).toBe(true);
    expect(ctx.includedRefs.length).toBe(3);
    expect(ctx.usedTokens).toBeLessThanOrEqual(ctx.budgetTokens);
    for (const h of heuristics) {
      expect(ctx.block).toContain(h.text);
    }
  });

  test("drops lowest-priority items to fit the token budget", () => {
    const ctx = buildSubagentMemoryContext({
      heuristics,
      budgetTokens: estimateOf(heuristics.slice(0, 2)) + 1,
    });
    // The two high-confidence items fit; the low-confidence one does not.
    expect(ctx.includedRefs).toEqual(["h1", "h2"]);
    expect(ctx.droppedRefs).toContain("h3");
  });

  test("respects maxItems", () => {
    const ctx = buildSubagentMemoryContext({ heuristics, maxItems: 2 });
    expect(ctx.includedRefs.length).toBe(2);
  });

  test("alwaysInclude pins an item that would otherwise be dropped", () => {
    const ctx = buildSubagentMemoryContext({
      heuristics,
      budgetTokens: estimateOf([heuristics[0]!]) + 1,
      alwaysInclude: ["h3"],
    });
    expect(ctx.includedRefs).toContain("h3");
  });

  test("returns an empty block when there is nothing to inherit", () => {
    const empty = buildSubagentMemoryContext({ heuristics: [] });
    expect(empty.block).toBe("");
    expect(empty.includedRefs).toEqual([]);
  });

  test("ranks by cosine similarity to the task when embeddings are given", () => {
    const embedded = [
      { ref: "near", text: "gateway queue restart", embedding: [1, 0] },
      { ref: "far", text: "sourdough starter feeding", embedding: [-1, 0] },
    ];
    const ctx = buildSubagentMemoryContext({
      heuristics: embedded,
      taskEmbedding: [1, 0],
      budgetTokens: DEFAULT_SUBAGENT_MEMORY_BUDGET_TOKENS,
      maxItems: 1,
    });
    expect(ctx.includedRefs).toEqual(["near"]);
  });

  test("never leaks the parent's private per-case refs", () => {
    const ctx = buildSubagentMemoryContext({ heuristics });
    // Refs inside the block are opaque content digests, not parent ledger ids.
    expect(ctx.block).not.toContain(":h1");
    expect(ctx.block).not.toContain(":h2");
  });

  test("is deterministic", () => {
    const one = buildSubagentMemoryContext({ heuristics });
    const two = buildSubagentMemoryContext({ heuristics });
    expect(one).toEqual(two);
  });
});

/** The token estimate of the block these heuristics would produce. */
const estimateOf = (
  items: ReadonlyArray<{ readonly text: string }>,
): number =>
  Math.ceil(
    [SUBAGENT_MEMORY_HEADER, ...items.map((h) => `- ${h.text}`)].join("\n").length / 4,
  );

describe("harvestSubagentOutcome", () => {
  test("keeps clean findings as bounded ledger entries", () => {
    const result = harvestSubagentOutcome({
      ownerScope: OWNER,
      projectScope: PROJECT,
      outcome: {
        childId: "child-1",
        summary: "added the parser tests",
        findings: ["the parser rejects duplicate keys", "tests cover the error path"],
        completedAtMs: Date.UTC(2026, 0, 15, 12, 0, 0),
      },
    });
    expect(result.entries.length).toBe(2);
    expect(result.rejectedUnsafe).toBe(0);
    const entry = result.entries[0]!;
    expect(entry.schema).toBe(HARVEST_LEDGER_SCHEMA_ID);
    expect(entry.entryId).toMatch(/^harvest:[0-9a-f]{64}$/);
    expect(entry.ownerScope).toBe(OWNER);
    expect(entry.childId).toBe("child-1");
    expect(entry.digest).toMatch(/^sha256:[a-f0-9]{64}$/);
    expect(entry.completedAt).toBe("2026-01-15T12:00:00.000Z");
  });

  test("rejects credential-shaped findings outright", () => {
    const token = "ghp_1234567890abcdef1234567890abcdef";
    const result = harvestSubagentOutcome({
      ownerScope: OWNER,
      projectScope: PROJECT,
      outcome: {
        childId: "child-1",
        summary: "done",
        findings: [`use ${token} for api calls`, "a clean finding"],
        completedAtMs: Date.UTC(2026, 0, 15, 12, 0, 0),
      },
    });
    expect(result.rejectedUnsafe).toBe(1);
    expect(result.entries.length).toBe(1);
    expect(JSON.stringify(result.entries)).not.toContain(token);
  });

  test("redacts soft PII but keeps the finding", () => {
    const result = harvestSubagentOutcome({
      ownerScope: OWNER,
      projectScope: PROJECT,
      outcome: {
        childId: "child-1",
        summary: "done",
        findings: ["email chris@example.com about the launch"],
        completedAtMs: Date.UTC(2026, 0, 15, 12, 0, 0),
      },
    });
    expect(result.entries.length).toBe(1);
    expect(result.entries[0]!.finding).not.toContain("chris@example.com");
    expect(result.entries[0]!.finding).toContain("[REDACTED:");
  });

  test("records inherited provenance refs on every entry", () => {
    const result = harvestSubagentOutcome({
      ownerScope: OWNER,
      projectScope: PROJECT,
      outcome: {
        childId: "child-2",
        summary: "done",
        findings: ["one finding"],
        completedAtMs: Date.UTC(2026, 0, 15, 12, 0, 0),
      },
      inheritedRefs: [{ patternRef: "pattern:abc" }],
    });
    expect(result.entries[0]!.inheritedRefs).toEqual(["pattern:abc"]);
  });

  test("falls back to the summary when no findings are supplied", () => {
    const result = harvestSubagentOutcome({
      ownerScope: OWNER,
      projectScope: PROJECT,
      outcome: {
        childId: "child-3",
        summary: "the whole summary becomes one entry",
        completedAtMs: Date.UTC(2026, 0, 15, 12, 0, 0),
      },
    });
    expect(result.entries.length).toBe(1);
    expect(result.entries[0]!.finding).toBe("the whole summary becomes one entry");
  });

  test("entry ids are deterministic per child and finding", () => {
    const run = (): string =>
      harvestSubagentOutcome({
        ownerScope: OWNER,
        projectScope: PROJECT,
        outcome: {
          childId: "child-4",
          summary: "s",
          findings: ["same finding"],
          completedAtMs: Date.UTC(2026, 0, 15, 12, 0, 0),
        },
      }).entries[0]!.entryId;
    expect(run()).toBe(run());
  });

  test("round-trips back into delegation as a heuristic", () => {
    const first = harvestSubagentOutcome({
      ownerScope: OWNER,
      projectScope: PROJECT,
      outcome: {
        childId: "child-5",
        summary: "s",
        findings: ["the flaky test needs a fresh database per run"],
        completedAtMs: Date.UTC(2026, 0, 15, 12, 0, 0),
      },
    });
    const heuristics = ledgerEntriesAsHeuristics(first.entries);
    expect(heuristics[0]!.text).toBe("the flaky test needs a fresh database per run");
    expect(heuristics[0]!.confidence).toBeGreaterThan(0);

    const ctx = buildSubagentMemoryContext({ heuristics });
    expect(ctx.block).toContain("fresh database per run");
  });
});
