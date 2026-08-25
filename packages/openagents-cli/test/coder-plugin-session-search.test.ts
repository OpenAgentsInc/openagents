/**
 * The session searcher through the real boundary: the checked-in
 * `session_search` plugin against staged fixture trees shaped like
 * `~/.claude` and `~/.codex`. The searcher's logic is unit-tested against
 * a fake host in `plugins/session-search/src/tests.rs`; this file proves
 * the same behavior holds through the WASM sandbox — a phrase found across
 * both sources with bounded context, a miss returning empty matches, and a
 * blank query coming back as a refusal envelope rather than a trap.
 */

import { mkdirSync, mkdtempSync, copyFileSync, readFileSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  invokePlugin,
  isRefusal,
  loadPluginFromManifest,
  type LoadedPlugin,
} from "../src/coder-plugins.js";

const MANIFEST = fileURLToPath(
  new URL("../../../plugins/session-search/manifest.json", import.meta.url),
);
const WASM = fileURLToPath(
  new URL("../../../plugins/session-search/session_search.wasm", import.meta.url),
);

const NOW_MS = Date.now();
const DAY_MS = 86_400_000;

const touch = (path: string, mtimeMs: number): void => {
  utimesSync(path, new Date(mtimeMs), new Date(mtimeMs));
};

const claudeLine = (cwd: string, id: string, role: string, text: string): string =>
  JSON.stringify({
    type: role,
    cwd,
    sessionId: id,
    message: { role, content: text },
  }) + "\n";

const codexLine = (payload: Record<string, unknown>): string =>
  JSON.stringify({ type: "response_item", payload }) + "\n";

const stage = (): { plugin: LoadedPlugin } => {
  const dir = mkdtempSync(join(tmpdir(), "session-search-"));
  const claudeRoot = join(dir, "dot-claude");
  const codexRoot = join(dir, "dot-codex");

  const project = join(claudeRoot, "projects", "-Users-ada-work-alpha");
  mkdirSync(project, { recursive: true });
  const filler = "the build is green and nothing else happened here ".repeat(20);
  writeFileSync(
    join(project, "cafe.jsonl"),
    claudeLine("/Users/ada/work/alpha", "cafe", "user", `${filler}the flux capacitor is rattling${filler}`) +
      claudeLine("/Users/ada/work/alpha", "cafe", "assistant", "tighten its bolts"),
  );
  touch(join(project, "cafe.jsonl"), NOW_MS - DAY_MS);

  const day = join(codexRoot, "sessions", "2026", "08", "20");
  mkdirSync(day, { recursive: true });
  const rollout = join(day, "rollout-2026-08-20T10-00-00-beef.jsonl");
  writeFileSync(
    rollout,
    JSON.stringify({ type: "session_meta", payload: { id: "beef", cwd: "/Users/ada/work/beta" } }) +
      "\n" +
      codexLine({
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "why does the Flux Capacitor overheat?" }],
      }) +
      codexLine({
        type: "message",
        role: "assistant",
        content: [{ type: "output_text", text: "it needs coolant" }],
      }),
  );
  touch(rollout, NOW_MS - 2 * DAY_MS);

  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
    capabilities: { mounts: Array<{ path: string; readonly: true }> };
  };
  manifest.capabilities.mounts = [
    { path: claudeRoot, readonly: true },
    { path: codexRoot, readonly: true },
  ];
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  copyFileSync(WASM, join(dir, "session_search.wasm"));

  const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return { plugin: outcome };
};

type Output = {
  query: string;
  sessions_searched: number;
  sessions_matched: number;
  matches: Array<{
    source: string;
    session_id: string;
    hits: Array<{ role: string; context: string }>;
    hits_total: number;
  }>;
  truncated: boolean;
  skipped_unreadable: number;
};

const packet = (input: Record<string, unknown>): Uint8Array =>
  new TextEncoder().encode(JSON.stringify({ now_ms: NOW_MS, ...input }));

/** The guest envelope: `ok` on success, `refusal` as a value otherwise. */
type Envelope = { ok?: Output; refusal?: { code: string; reason: string } };

const invoke = async (plugin: LoadedPlugin, input: Record<string, unknown>): Promise<Envelope> => {
  const outcome = await invokePlugin(plugin, packet(input));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return JSON.parse(new TextDecoder().decode(outcome)) as Envelope;
};

const call = async (plugin: LoadedPlugin, input: Record<string, unknown>): Promise<Output> => {
  const envelope = await invoke(plugin, input);
  if (envelope.ok === undefined) throw new Error(JSON.stringify(envelope.refusal));
  return envelope.ok;
};

describe("the session_search plugin through the sandbox", () => {
  it("finds a phrase across both stores with bounded context", async () => {
    const { plugin } = stage();
    const out = await call(plugin, { query: "flux capacitor", context_chars: 80 });

    expect(out.sessions_searched).toBe(2);
    expect(out.sessions_matched).toBe(2);
    expect(out.matches.map((m) => [m.source, m.session_id])).toEqual([
      ["claude", "cafe"],
      ["codex", "beef"],
    ]);

    const claude = out.matches[0];
    expect(claude.hits[0].role).toBe("user");
    expect(claude.hits[0].context).toContain("flux capacitor");
    // The hit sits mid-turn inside long filler, so the window is elided on
    // both sides and stays near the character budget.
    expect(claude.hits[0].context.startsWith("…")).toBe(true);
    expect(claude.hits[0].context.endsWith("…")).toBe(true);
    expect(claude.hits[0].context.length).toBeLessThan(200);

    // Case-insensitive: the Codex turn spells it "Flux Capacitor".
    const codex = out.matches[1];
    expect(codex.hits[0].context).toContain("Flux Capacitor");
  });

  it("returns empty matches for a phrase nobody said", async () => {
    const { plugin } = stage();
    const out = await call(plugin, { query: "perpetual motion machine" });

    expect(out.sessions_searched).toBe(2);
    expect(out.sessions_matched).toBe(0);
    expect(out.matches).toEqual([]);
  });

  it("refuses a blank query as a refusal envelope", async () => {
    const { plugin } = stage();
    const envelope = await invoke(plugin, { query: "   " });

    expect(envelope.ok).toBeUndefined();
    expect(envelope.refusal?.reason).toContain("query");
  });
});
