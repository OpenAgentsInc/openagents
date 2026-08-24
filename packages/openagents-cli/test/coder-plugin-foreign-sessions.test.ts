/**
 * The foreign-session scanner through the real boundary: the checked-in
 * `foreign_sessions` plugin against staged fixture trees shaped like
 * `~/.claude` and `~/.codex`, with good, malformed, oversized, and
 * symlinked entries. The scanner's own logic is unit-tested against a
 * fake host in `plugins/foreign-sessions/src/tests.rs`; this file proves
 * the same behavior holds through the WASM sandbox and the host's
 * confined `read_file` and `list_dir` imports, over absolute mount roots.
 */

import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
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
  new URL("../../../plugins/foreign-sessions/manifest.json", import.meta.url),
);
const WASM = fileURLToPath(
  new URL("../../../plugins/foreign-sessions/foreign_sessions.wasm", import.meta.url),
);

const NOW_MS = Date.now();
const DAY_MS = 86_400_000;

const claudeRecord = (cwd: string, sessionId: string): string =>
  `${JSON.stringify({
    type: "user",
    cwd,
    sessionId,
    message: { role: "user", content: "hello" },
  })}\n`;

const codexMeta = (cwd: string, id: string): string =>
  `${JSON.stringify({
    timestamp: "2026-08-20T10:00:00.000Z",
    type: "session_meta",
    payload: { id, cwd },
  })}\n`;

const touch = (path: string, mtimeMs: number): void => {
  utimesSync(path, new Date(mtimeMs), new Date(mtimeMs));
};

/**
 * Stage fixture `~/.claude` and `~/.codex` trees plus a manifest copy
 * whose mounts point at them by absolute path, and load the plugin.
 */
const stage = (): { plugin: LoadedPlugin; claudeRoot: string; codexRoot: string } => {
  const dir = mkdtempSync(join(tmpdir(), "foreign-sessions-"));
  const claudeRoot = join(dir, "dot-claude");
  const codexRoot = join(dir, "dot-codex");

  // Claude: one good recent session, one malformed, one oversized, one
  // symlinked, one stale (40 days old); a second project for cwd filtering.
  const projectA = join(claudeRoot, "projects", "-Users-ada-work-alpha");
  mkdirSync(projectA, { recursive: true });
  writeFileSync(
    join(projectA, "good.jsonl"),
    claudeRecord("/Users/ada/work/alpha", "good") + claudeRecord("/Users/ada/work/alpha", "good"),
  );
  touch(join(projectA, "good.jsonl"), NOW_MS - DAY_MS);
  writeFileSync(join(projectA, "malformed.jsonl"), "this is not json\n{}\n");
  touch(join(projectA, "malformed.jsonl"), NOW_MS - DAY_MS);
  writeFileSync(join(projectA, "huge.jsonl"), Buffer.alloc(MOUNT_FILE_LIMIT + 1, 0x7b));
  touch(join(projectA, "huge.jsonl"), NOW_MS - 2 * DAY_MS);
  writeFileSync(join(dir, "outside.jsonl"), claudeRecord("/elsewhere", "outside"));
  symlinkSync(join(dir, "outside.jsonl"), join(projectA, "sneaky.jsonl"));
  writeFileSync(join(projectA, "stale.jsonl"), claudeRecord("/Users/ada/work/alpha", "stale"));
  touch(join(projectA, "stale.jsonl"), NOW_MS - 40 * DAY_MS);

  const projectB = join(claudeRoot, "projects", "-Users-ada-work-beta");
  mkdirSync(projectB, { recursive: true });
  writeFileSync(join(projectB, "other.jsonl"), claudeRecord("/Users/ada/work/beta", "other"));
  touch(join(projectB, "other.jsonl"), NOW_MS - 3 * DAY_MS);

  // Codex: one good recent rollout and one malformed one.
  const day = join(codexRoot, "sessions", "2026", "08", "20");
  mkdirSync(day, { recursive: true });
  writeFileSync(
    join(day, "rollout-2026-08-20T10-00-00-abc.jsonl"),
    codexMeta("/Users/ada/work/gamma", "abc"),
  );
  touch(join(day, "rollout-2026-08-20T10-00-00-abc.jsonl"), NOW_MS - DAY_MS / 2);
  writeFileSync(join(day, "rollout-2026-08-20T11-00-00-bad.jsonl"), "not a rollout\n");
  touch(join(day, "rollout-2026-08-20T11-00-00-bad.jsonl"), NOW_MS - DAY_MS / 2);

  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
    capabilities: { mounts: Array<{ path: string; readonly: true }> };
  };
  manifest.capabilities.mounts = [
    { path: claudeRoot, readonly: true },
    { path: codexRoot, readonly: true },
  ];
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  copyFileSync(WASM, join(dir, "foreign_sessions.wasm"));

  const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return { plugin: outcome, claudeRoot, codexRoot };
};

type Session = {
  source: string;
  session_id: string;
  path: string;
  cwd?: string;
  project_dir?: string;
  mtime_ms: number;
  size_bytes: number;
  record_count?: number;
  metadata_truncated?: boolean;
};

type Output = {
  ok?: {
    sessions: Session[];
    skipped: { malformed: number; unreadable: number; symlinked: number };
    oversized: number;
    missing_sources?: string[];
    scan_truncated: boolean;
    read_budget_exhausted: boolean;
  };
  refusal?: { code: string; reason: string };
};

const run = async (plugin: LoadedPlugin, args: Record<string, unknown>): Promise<Output> => {
  const packet = new TextEncoder().encode(JSON.stringify({ now_ms: NOW_MS, ...args }));
  const outcome = await invokePlugin(plugin, packet);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return JSON.parse(new TextDecoder().decode(outcome)) as Output;
};

describe("foreign session discovery", () => {
  it("discovers recent sessions from both stores, newest first, with metadata", async () => {
    const { plugin } = stage();
    const { ok } = await run(plugin, {});
    expect(ok).toBeDefined();

    const ids = ok?.sessions.map((s) => s.session_id);
    expect(ids).toEqual(["abc", "good", "huge", "other"]);

    const good = ok?.sessions.find((s) => s.session_id === "good");
    expect(good).toMatchObject({
      source: "claude",
      cwd: "/Users/ada/work/alpha",
      project_dir: "-Users-ada-work-alpha",
      path: "projects/-Users-ada-work-alpha/good.jsonl",
      record_count: 2,
    });
    expect(good?.metadata_truncated).toBeUndefined();
    expect(good?.size_bytes).toBeGreaterThan(0);

    const codex = ok?.sessions.find((s) => s.session_id === "abc");
    expect(codex).toMatchObject({
      source: "codex",
      cwd: "/Users/ada/work/gamma",
      path: "sessions/2026/08/20/rollout-2026-08-20T10-00-00-abc.jsonl",
      record_count: 1,
    });
  });

  it("fails soft: malformed skipped and counted, oversized kept but truncated, symlinks refused", async () => {
    const { plugin } = stage();
    const { ok } = await run(plugin, {});

    // malformed.jsonl (claude) and the bad rollout (codex).
    expect(ok?.skipped.malformed).toBe(2);
    // sneaky.jsonl, reported by the listing as a symlink and never read.
    expect(ok?.skipped.symlinked).toBe(1);

    const huge = ok?.sessions.find((s) => s.session_id === "huge");
    expect(huge?.metadata_truncated).toBe(true);
    expect(huge?.cwd).toBeUndefined();
    expect(huge?.record_count).toBeUndefined();
    expect(huge?.size_bytes).toBe(MOUNT_FILE_LIMIT + 1);
    expect(ok?.oversized).toBe(1);
  });

  it("drops sessions older than the age cutoff", async () => {
    const { plugin } = stage();
    const { ok } = await run(plugin, {});
    expect(ok?.sessions.some((s) => s.session_id === "stale")).toBe(false);

    const wide = await run(plugin, { max_age_days: 365 });
    expect(wide.ok?.sessions.some((s) => s.session_id === "stale")).toBe(true);
  });

  it("narrows to a working directory with cwd_filter", async () => {
    const { plugin } = stage();
    const { ok } = await run(plugin, { cwd_filter: "work/beta" });
    expect(ok?.sessions.map((s) => s.session_id)).toEqual(["other"]);
  });

  it("honors limit and source selection", async () => {
    const { plugin } = stage();
    const one = await run(plugin, { limit: 1 });
    expect(one.ok?.sessions).toHaveLength(1);

    const codexOnly = await run(plugin, { sources: ["codex"] });
    expect(codexOnly.ok?.sessions.map((s) => s.source)).toEqual(["codex"]);
  });

  it("reports a store that is not present instead of failing", async () => {
    const { plugin } = stage();
    // The codex mount exists but holds no `sessions` directory in this
    // staging; rebuild the fixture without it.
    const dir = mkdtempSync(join(tmpdir(), "foreign-sessions-empty-"));
    const claudeRoot = join(dir, "dot-claude");
    const codexRoot = join(dir, "dot-codex");
    mkdirSync(join(claudeRoot, "projects"), { recursive: true });
    mkdirSync(codexRoot, { recursive: true });
    const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
      capabilities: { mounts: Array<{ path: string; readonly: true }> };
    };
    manifest.capabilities.mounts = [
      { path: claudeRoot, readonly: true },
      { path: codexRoot, readonly: true },
    ];
    writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
    copyFileSync(WASM, join(dir, "foreign_sessions.wasm"));
    const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
    if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);

    const { ok } = await run(outcome, {});
    expect(ok?.sessions).toEqual([]);
    expect(ok?.missing_sources).toEqual(["codex"]);
    void plugin;
  });

  it("refuses an unknown source as a typed guest refusal", async () => {
    const { plugin } = stage();
    const answer = await run(plugin, { sources: ["cursor"] });
    expect(answer.refusal?.code).toBe("unsupported");
    expect(answer.refusal?.reason).toContain("cursor");
  });
});
