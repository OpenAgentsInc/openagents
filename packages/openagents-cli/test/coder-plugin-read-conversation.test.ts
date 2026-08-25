/**
 * The conversation reader through the real boundary: the checked-in
 * `read_conversation` plugin against staged fixture trees shaped like
 * `~/.claude` and `~/.codex`. The reader's logic is unit-tested against a
 * fake host in `plugins/read-conversation/src/tests.rs`; this file proves
 * the same behavior holds through the WASM sandbox — including the
 * bounded `read_file_range` import this plugin is the first to use, on a
 * session file past the whole-read bound.
 */

import { mkdirSync, mkdtempSync, copyFileSync, readFileSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  MOUNT_FILE_LIMIT,
  invokePlugin,
  isRefusal,
  loadPluginFromManifest,
  type LoadedPlugin,
} from "../src/coder-plugins.js";

const MANIFEST = fileURLToPath(
  new URL("../../../plugins/read-conversation/manifest.json", import.meta.url),
);
const WASM = fileURLToPath(
  new URL("../../../plugins/read-conversation/read_conversation.wasm", import.meta.url),
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

const stage = (options?: { oversized?: boolean }): { plugin: LoadedPlugin } => {
  const dir = mkdtempSync(join(tmpdir(), "read-conversation-"));
  const claudeRoot = join(dir, "dot-claude");
  const codexRoot = join(dir, "dot-codex");

  const project = join(claudeRoot, "projects", "-Users-ada-work-alpha");
  mkdirSync(project, { recursive: true });
  const conversation =
    claudeLine("/Users/ada/work/alpha", "good", "user", "what broke?") +
    claudeLine("/Users/ada/work/alpha", "good", "assistant", "the test; fixing it");
  const body = options?.oversized
    ? JSON.stringify({ type: "padding", filler: "p".repeat(MOUNT_FILE_LIMIT) }) +
      "\n" +
      conversation
    : conversation;
  writeFileSync(join(project, "good.jsonl"), body);
  touch(join(project, "good.jsonl"), NOW_MS - DAY_MS);

  mkdirSync(join(codexRoot, "sessions"), { recursive: true });

  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
    capabilities: { mounts: Array<{ path: string; readonly: true }> };
  };
  manifest.capabilities.mounts = [
    { path: claudeRoot, readonly: true },
    { path: codexRoot, readonly: true },
  ];
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  copyFileSync(WASM, join(dir, "read_conversation.wasm"));

  const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return { plugin: outcome };
};

type Output = {
  source: string;
  session_id: string;
  tail_only?: boolean;
  turns: Array<{ role: string; text: string }>;
  dropped_leading_turns: number;
};

const packet = (input: Record<string, unknown>): Uint8Array =>
  new TextEncoder().encode(JSON.stringify({ ...input, now_ms: NOW_MS }));

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

describe("the read_conversation plugin through the sandbox", () => {
  it("reads the newest conversation back as ordered turns", async () => {
    const { plugin } = stage();
    const out = await call(plugin, {});

    expect(out.source).toBe("claude");
    expect(out.session_id).toBe("good");
    expect(out.turns.map((turn) => [turn.role, turn.text])).toEqual([
      ["user", "what broke?"],
      ["assistant", "the test; fixing it"],
    ]);
    expect(out.tail_only ?? false).toBe(false);
  });

  it("reads an oversized session from its tail through the range import", async () => {
    const { plugin } = stage({ oversized: true });
    const out = await call(plugin, {});

    expect(out.tail_only).toBe(true);
    expect(out.turns.map((turn) => turn.text)).toEqual(["what broke?", "the test; fixing it"]);
  });

  it("refuses an unknown session id with a pointer at the scanner", async () => {
    const { plugin } = stage();
    const envelope = await invoke(plugin, { session_id: "nope" });

    expect(envelope.ok).toBeUndefined();
    expect(envelope.refusal?.reason).toContain("nope");
    expect(envelope.refusal?.reason).toContain("foreign_sessions");
  });
});
