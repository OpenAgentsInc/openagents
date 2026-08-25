/**
 * The `repo_tree` plugin through the real boundary, plus the parameterized
 * workspace mount it rides on.
 *
 * The tree walker's logic — the gitignore subset, the ceilings, the query
 * ranking — is unit-tested against a fake host in
 * `plugins/repo-tree/src/tests.rs`; this file proves the same behavior
 * holds through the WASM sandbox against a staged fixture workspace, and
 * that the checked-in manifest's `${workspace}` mount
 * (OpenAgentsInc/openagents#44) resolves to the load-time working
 * directory.
 */

import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { realpathSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  invokePlugin,
  isRefusal,
  loadPluginFromManifest,
  type LoadedPlugin,
} from "../src/coder-plugins.js";

const MANIFEST = fileURLToPath(
  new URL("../../../plugins/repo-tree/manifest.json", import.meta.url),
);
const WASM = fileURLToPath(new URL("../../../plugins/repo-tree/repo_tree.wasm", import.meta.url));

/**
 * Stage a fixture workspace: real files, a root `.gitignore` with a
 * negation, a nested ignored directory, a nested `.gitignore`, and a
 * `.git` directory that must never appear.
 */
const stage = (): { plugin: LoadedPlugin; root: string } => {
  const dir = mkdtempSync(join(tmpdir(), "repo-tree-"));
  const root = join(dir, "workspace");

  const write = (relative: string, body: string): void => {
    const path = join(root, relative);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, body);
  };
  write(".gitignore", "# artifacts\n*.log\nbuild/\n!keep.log\n");
  write(".git/config", "[core]");
  write(".git/HEAD", "ref: refs/heads/main");
  write("README.md", "hello");
  write("build/out.o", "object code");
  write("carbon.log", "dropped by *.log");
  write("keep.log", "the negation cannot save it");
  write("lib/auth_controller.ex", "defmodule Auth do end");
  write("lib/nested/.gitignore", "*.tmp\n");
  write("lib/nested/junk.tmp", "dropped by the nested ignore");
  write("lib/nested/real.ex", "kept");

  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
    capabilities: { mounts: Array<{ path: string; readonly: true }> };
  };
  manifest.capabilities.mounts = [{ path: root, readonly: true }];
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  copyFileSync(WASM, join(dir, "repo_tree.wasm"));

  const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return { plugin: outcome, root };
};

type TreeOutput = {
  entries: Array<{ path: string; kind: string; size: number }>;
  total_seen: number;
  truncated: boolean;
  ignored_negations: number;
  skipped_gitignored: number;
};

type QueryOutput = {
  matches: Array<{ path: string; size: number }>;
  searched: number;
  truncated: boolean;
};

type Envelope<O> = { ok?: O; refusal?: { code: string; reason: string } };

const call = async <O>(plugin: LoadedPlugin, input: Record<string, unknown>): Promise<O> => {
  const packet = new TextEncoder().encode(JSON.stringify(input));
  const outcome = await invokePlugin(plugin, packet);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  const envelope = JSON.parse(new TextDecoder().decode(outcome)) as Envelope<O>;
  if (envelope.ok === undefined) throw new Error(JSON.stringify(envelope.refusal));
  return envelope.ok;
};

describe("the repo_tree plugin through the sandbox", () => {
  it("walks the tree honoring gitignore and never shows .git", async () => {
    const { plugin } = stage();
    const out = await call<TreeOutput>(plugin, {});

    const paths = out.entries.map((entry) => entry.path);
    expect(paths).toEqual([
      ".gitignore",
      "README.md",
      "lib",
      "lib/auth_controller.ex",
      "lib/nested",
      "lib/nested/.gitignore",
      "lib/nested/real.ex",
    ]);
    // `build/` (directory rule), both `.log` files (the negation is
    // ignored and counted), and the nested `.tmp` were dropped; `.git`
    // was never even seen.
    expect(out.skipped_gitignored).toBe(4);
    expect(out.ignored_negations).toBe(1);
    expect(out.truncated).toBe(false);
    expect(out.entries.find((entry) => entry.path === "README.md")?.kind).toBe("file");
    expect(out.entries.find((entry) => entry.path === "README.md")?.size).toBe(5);
    expect(out.entries.find((entry) => entry.path === "lib")?.kind).toBe("dir");
  });

  it("caps the entries and reports the truncation", async () => {
    const { plugin } = stage();
    const out = await call<TreeOutput>(plugin, { max_entries: 2 });

    expect(out.entries).toHaveLength(2);
    expect(out.truncated).toBe(true);
  });

  it("answers a fuzzy query with ranked matches instead of the tree", async () => {
    const { plugin } = stage();
    const out = await call<QueryOutput>(plugin, { query: "AuthController" });

    expect(out.matches.map((match) => match.path)).toEqual(["lib/auth_controller.ex"]);
    // Every non-ignored file was searched; the gitignored and `.git`
    // files never entered the candidate set.
    expect(out.searched).toBe(5);
    expect(out.truncated).toBe(false);
  });
});

describe("the parameterized workspace mount (OpenAgentsInc/openagents#44)", () => {
  it("loads the checked-in manifest with `${workspace}` resolved to the load-time cwd", async () => {
    // The checked-in manifest, untouched: its one mount is the literal
    // `${workspace}`.
    const raw = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
      capabilities: { mounts: Array<{ path: string }> };
    };
    expect(raw.capabilities.mounts.map((mount) => mount.path)).toEqual(["${workspace}"]);

    const outcome = loadPluginFromManifest(MANIFEST);
    if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);

    // The resolved mount is the process working directory — the confined
    // root every read and listing goes through — not the manifest's
    // directory and not a literal `${workspace}` path.
    expect(outcome.mounts).toEqual([realpathSync(process.cwd())]);

    // And the plugin really reads through it: a depth-1 tree of the
    // load-time cwd (this package) names this package's own files.
    const out = await call<TreeOutput>(outcome, { max_depth: 1 });
    const paths = out.entries.map((entry) => entry.path);
    expect(paths).toContain("package.json");
    for (const path of paths) {
      expect(path.startsWith("/")).toBe(false);
      expect(path.startsWith("..")).toBe(false);
    }
  });
});
