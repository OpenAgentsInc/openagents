/**
 * The workspace search through the real boundary: the checked-in
 * `code_search` plugin against a staged fixture workspace. The search
 * logic is unit-tested against a fake host in
 * `plugins/code-search/src/tests.rs`; this file proves the same behavior
 * holds through the WASM sandbox — literal and regex matching with
 * per-file grouping, the gitignore subset, the bound ceilings with their
 * honest truncation record, and the clean refusal of patterns outside the
 * regex subset. The manifest's `${workspace}` mount is rewritten to the
 * fixture directory, as every sandbox test does; the host-side
 * `${workspace}` resolution is proven elsewhere.
 */

import { mkdirSync, mkdtempSync, copyFileSync, readFileSync, writeFileSync } from "node:fs";
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
import { discoverPluginCatalog, matchCapabilities } from "../src/coder-capability.js";

const MANIFEST = fileURLToPath(
  new URL("../../../plugins/code-search/manifest.json", import.meta.url),
);
const WASM = fileURLToPath(
  new URL("../../../plugins/code-search/code_search.wasm", import.meta.url),
);

const stage = (): { plugin: LoadedPlugin } => {
  const dir = mkdtempSync(join(tmpdir(), "code-search-"));
  const workspace = join(dir, "workspace");

  mkdirSync(join(workspace, "src"), { recursive: true });
  mkdirSync(join(workspace, "node_modules", "pkg"), { recursive: true });

  writeFileSync(join(workspace, ".gitignore"), "node_modules/\n*.log\n");
  writeFileSync(
    join(workspace, "src", "auth.ex"),
    "defmodule Auth do\n  def handle_login(conn) do\n    conn\n  end\n\n  def handle_logout(conn) do\n    conn\n  end\nend\n",
  );
  writeFileSync(
    join(workspace, "src", "router.ex"),
    "defmodule Router do\n  # login is handled by Auth.handle_login\nend\n",
  );
  writeFileSync(join(workspace, "node_modules", "pkg", "index.js"), "function login() {}\n");
  writeFileSync(join(workspace, "debug.log"), "login attempt failed\n");

  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
    capabilities: { mounts: Array<{ path: string; readonly: true }> };
  };
  manifest.capabilities.mounts = [{ path: workspace, readonly: true }];
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  copyFileSync(WASM, join(dir, "code_search.wasm"));

  const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return { plugin: outcome };
};

type Match = { line: number; text: string; before: string[]; after: string[] };
type Output = {
  files: Array<{ path: string; matches: Match[]; matches_total: number }>;
  files_considered: number;
  files_scanned: number;
  files_unscanned: number;
  files_matched: number;
  matches_returned: number;
  matches_dropped: number;
  skipped_gitignored: number;
  ignored_negations: number;
  skipped_binary: number;
  skipped_oversized: number;
  skipped_unreadable: number;
  truncated: boolean;
};

/** The guest envelope: `ok` on success, `refusal` as a value otherwise. */
type Envelope = { ok?: Output; refusal?: { code: string; reason: string } };

const invoke = async (plugin: LoadedPlugin, input: Record<string, unknown>): Promise<Envelope> => {
  const packet = new TextEncoder().encode(JSON.stringify(input));
  const outcome = await invokePlugin(plugin, packet);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return JSON.parse(new TextDecoder().decode(outcome)) as Envelope;
};

const call = async (plugin: LoadedPlugin, input: Record<string, unknown>): Promise<Output> => {
  const envelope = await invoke(plugin, input);
  if (envelope.ok === undefined) throw new Error(JSON.stringify(envelope.refusal));
  return envelope.ok;
};

describe("the code_search plugin through the sandbox", () => {
  it("finds a literal pattern, grouped per file, honoring gitignore", async () => {
    const { plugin } = stage();
    const out = await call(plugin, { pattern: "login" });

    const paths = out.files.map((file) => file.path);
    expect(paths).toEqual(["src/auth.ex", "src/router.ex"]);
    expect(paths.some((path) => path.includes("node_modules"))).toBe(false);
    expect(paths.some((path) => path.endsWith(".log"))).toBe(false);
    // The ignored directory counts once and the log file once.
    expect(out.skipped_gitignored).toBe(2);
    expect(out.files_matched).toBe(2);
    expect(out.truncated).toBe(false);

    const auth = out.files[0];
    expect(auth?.matches.map((match) => match.line)).toEqual([2]);
    expect(auth?.matches[0]?.text).toBe("  def handle_login(conn) do");
    expect(auth?.matches[0]?.before).toEqual(["defmodule Auth do"]);
    expect(auth?.matches[0]?.after).toEqual(["    conn", "  end"]);
  });

  it("matches a regex from the documented subset", async () => {
    const { plugin } = stage();
    const out = await call(plugin, { pattern: "def handle_\\w+\\(", regex: true });

    expect(out.files.map((file) => file.path)).toEqual(["src/auth.ex"]);
    expect(out.files[0]?.matches.map((match) => match.line)).toEqual([2, 6]);
    expect(out.matches_returned).toBe(2);
  });

  it("enforces the match ceiling and reports what was dropped", async () => {
    const { plugin } = stage();
    const out = await call(plugin, { pattern: "conn", max_matches: 2, context_lines: 0 });

    // auth.ex holds four `conn` lines; the ceiling returns two of them,
    // counts the other two dropped, and leaves router.ex unscanned.
    expect(out.matches_returned).toBe(2);
    expect(out.files[0]?.matches_total).toBe(4);
    expect(out.matches_dropped).toBe(2);
    expect(out.files_unscanned).toBeGreaterThan(0);
    expect(out.truncated).toBe(true);
  });

  it("returns an empty, untruncated result when nothing matches", async () => {
    const { plugin } = stage();
    const out = await call(plugin, { pattern: "no_such_token_anywhere" });

    expect(out.files).toEqual([]);
    expect(out.matches_returned).toBe(0);
    expect(out.files_scanned).toBeGreaterThan(0);
    expect(out.truncated).toBe(false);
  });

  it("refuses a pattern outside the regex subset with a reason, as a value", async () => {
    const { plugin } = stage();
    const envelope = await invoke(plugin, { pattern: "handle_(login|logout)", regex: true });

    expect(envelope.ok).toBeUndefined();
    expect(envelope.refusal?.code).toBe("unsupported");
    expect(envelope.refusal?.reason).toContain("groups");
  });

  it("refuses an empty pattern cleanly", async () => {
    const { plugin } = stage();
    const envelope = await invoke(plugin, { pattern: "   " });

    expect(envelope.ok).toBeUndefined();
    expect(envelope.refusal?.code).toBe("unsupported");
  });

  it("is deterministic: the same tree and query produce identical output", async () => {
    const { plugin } = stage();
    const first = await call(plugin, { pattern: "handle", context_lines: 1 });
    const second = await call(plugin, { pattern: "handle", context_lines: 1 });

    expect(second).toEqual(first);
  });
});

describe("code_search in the capability catalog", () => {
  it("is discovered and surfaces for a `where is X handled?` prompt", () => {
    const catalog = discoverPluginCatalog(fileURLToPath(import.meta.url));
    expect(catalog.map((entry) => entry.name)).toContain("code_search");

    const matches = matchCapabilities(catalog, "where is the login flow handled?");
    expect(matches[0]?.entry.name).toBe("code_search");
  });
});
