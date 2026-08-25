/**
 * The knowledge base through the real boundary and onto the rail: the
 * checked-in `knowledge_base` plugin answers through the WASM sandbox from
 * its embedded corpus (no mounts, no hosts), and the harness-side note
 * builder turns those answers into the attached context a turn carries.
 * The ranking itself is unit-tested against the corpus in
 * `plugins/knowledge-base/src/tests.rs`; this file proves the loaded
 * artifact and the note agree end to end.
 */

import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  invokePlugin,
  isRefusal,
  loadPluginFromManifest,
  type LoadedPlugin,
} from "../src/coder-plugins.js";
import {
  KNOWLEDGE_ATTACH_FLOOR,
  knowledgeHits,
  knowledgeNote,
  type KnowledgeHit,
} from "../src/coder-knowledge.js";

const MANIFEST = fileURLToPath(
  new URL("../../../plugins/knowledge-base/manifest.json", import.meta.url),
);

const load = (): LoadedPlugin => {
  const outcome = loadPluginFromManifest(MANIFEST);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return outcome;
};

const ask = async (plugin: LoadedPlugin, input: Record<string, unknown>): Promise<unknown> => {
  const outcome = await invokePlugin(plugin, new TextEncoder().encode(JSON.stringify(input)));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return JSON.parse(new TextDecoder().decode(outcome));
};

describe("the knowledge_base plugin through the sandbox", () => {
  it("answers the earning question with the parked-economy stance", async () => {
    const plugin = load();
    const envelope = await ask(plugin, { query: "How can I earn bitcoin with this system?" });
    const hits = knowledgeHits(envelope);

    const top = hits[0];
    expect(top?.kind).toBe("stance");
    expect(top?.title).toBe("Earning bitcoin or money on OpenAgents");
    expect(top?.state).toContain("parked");
    expect(top?.date).toBeDefined();
  });

  it("has nothing to say about an unrelated topic", async () => {
    const plugin = load();
    const envelope = await ask(plugin, { query: "quaternion spline interpolation" });
    expect(knowledgeHits(envelope)).toEqual([]);
  });

  it("refuses a query with no matchable words, inside the envelope", async () => {
    const plugin = load();
    const envelope = (await ask(plugin, { query: "a & b" })) as {
      refusal?: { code: string };
    };
    expect(envelope.refusal?.code).toBe("unsupported");
  });
});

describe("the knowledge note on the rail", () => {
  const stance: KnowledgeHit = {
    kind: "stance",
    title: "Earning bitcoin or money on OpenAgents",
    body: "Earning is real but parked.",
    state: "deliberately parked",
    sources: ["docs/economy.md"],
    date: "2026-08-25",
    score: 12,
  };

  it("attaches a strong stance with its state and review date", () => {
    const note = knowledgeNote([stance]);
    expect(note).toContain("knowledge base");
    expect(note).toContain("deliberately parked");
    expect(note).toContain("reviewed 2026-08-25");
    expect(note).toContain("Earning is real but parked.");
  });

  it("stays silent below the floor and on no hits", () => {
    expect(knowledgeNote([])).toBeUndefined();
    expect(
      knowledgeNote([{ ...stance, score: KNOWLEDGE_ATTACH_FLOOR - 1 }]),
    ).toBeUndefined();
  });

  it("carries at most two hits and clips a runaway body", () => {
    const long = { ...stance, body: "x".repeat(5000) };
    const note = knowledgeNote([long, stance, stance]);
    expect(note).toBeDefined();
    expect((note ?? "").length).toBeLessThan(2000);
    expect((note ?? "").split("\n- ").length - 1).toBeLessThanOrEqual(2);
  });

  it("end to end: the sandbox answer becomes an attachable note", async () => {
    const plugin = load();
    const envelope = await ask(plugin, { query: "How can I earn bitcoin with this system?" });
    const note = knowledgeNote(knowledgeHits(envelope));
    expect(note).toContain("Earning bitcoin or money on OpenAgents");
    expect(note).toContain("parked");
  });
});
