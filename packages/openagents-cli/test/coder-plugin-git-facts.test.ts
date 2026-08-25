/**
 * The git facts reader through the real boundary: the checked-in
 * `git_facts` plugin against a staged fixture "repo" — a hand-written
 * `.git/HEAD`, loose ref, `packed-refs`, `logs/HEAD` reflog, and a
 * hand-built binary version-2 index, with matching and mismatching
 * workdir files around them. The parsing logic is unit-tested against a
 * fake host in `plugins/git-facts/src/tests.rs`; this file proves the
 * same facts hold through the WASM sandbox, and that a workspace with no
 * git repository is refused. The checked-in manifest declares the
 * `${workspace}` mount; here it is rewritten to the fixture root, the
 * established pattern for mount-bearing plugins.
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

const MANIFEST = fileURLToPath(new URL("../../../plugins/git-facts/manifest.json", import.meta.url));
const WASM = fileURLToPath(new URL("../../../plugins/git-facts/git_facts.wasm", import.meta.url));

const SHA_A = "a".repeat(40);
const SHA_B = "b".repeat(40);
const ZERO = "0".repeat(40);

/** Whole seconds, so index mtimes and filesystem mtimes agree exactly. */
const KEPT_MTIME_MS = 1_700_000_000_000;
const DRIFT_MTIME_MS = 1_700_000_100_000;

/**
 * Build a version-2 git index: the 12-byte DIRC header, 62-byte fixed
 * entries with 8-byte-aligned NUL padding after each path, and a zeroed
 * 20-byte trailing checksum (the plugin never verifies it).
 */
const buildIndex = (entries: Array<{ path: string; size: number; mtimeMs: number }>): Buffer => {
  const parts: Buffer[] = [];
  const header = Buffer.alloc(12);
  header.write("DIRC", 0, "ascii");
  header.writeUInt32BE(2, 4);
  header.writeUInt32BE(entries.length, 8);
  parts.push(header);
  for (const entry of entries) {
    const name = Buffer.from(entry.path, "utf8");
    const entryLen = (62 + name.length + 8) & ~7;
    const buffer = Buffer.alloc(entryLen);
    buffer.writeUInt32BE(Math.floor(entry.mtimeMs / 1000), 8); // mtime sec
    buffer.writeUInt32BE((entry.mtimeMs % 1000) * 1_000_000, 12); // mtime nsec
    buffer.writeUInt32BE(0o100644, 24); // mode
    buffer.writeUInt32BE(entry.size, 36);
    buffer.writeUInt16BE(name.length, 60); // flags: name length
    name.copy(buffer, 62);
    parts.push(buffer);
  }
  parts.push(Buffer.alloc(20));
  return Buffer.concat(parts);
};

const reflogLine = (oldId: string, newId: string, seconds: number, message: string): string =>
  `${oldId} ${newId} Ada Lovelace <ada@example.com> ${seconds} -0600\t${message}\n`;

const touch = (path: string, mtimeMs: number): void => {
  utimesSync(path, new Date(mtimeMs), new Date(mtimeMs));
};

const load = (workspace: string): LoadedPlugin => {
  const dir = mkdtempSync(join(tmpdir(), "git-facts-plugin-"));
  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
    capabilities: { mounts: Array<{ path: string; readonly: true }> };
  };
  manifest.capabilities.mounts = [{ path: workspace, readonly: true }];
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  copyFileSync(WASM, join(dir, "git_facts.wasm"));

  const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return outcome;
};

/**
 * A fixture repo on branch `main`: one loose ref, one packed branch, a
 * two-entry reflog, and a three-entry index against a workdir where
 * `kept.txt` matches its index stat, `drifted.txt` differs in size,
 * `gone.txt` is absent, and `extra.txt` is untracked.
 */
const stageRepo = (): string => {
  const workspace = mkdtempSync(join(tmpdir(), "git-facts-repo-"));
  const git = join(workspace, ".git");
  mkdirSync(join(git, "refs", "heads"), { recursive: true });
  mkdirSync(join(git, "logs"), { recursive: true });

  writeFileSync(join(git, "HEAD"), "ref: refs/heads/main\n");
  writeFileSync(join(git, "refs", "heads", "main"), `${SHA_A}\n`);
  writeFileSync(
    join(git, "packed-refs"),
    `# pack-refs with: peeled fully-peeled sorted \n${SHA_B} refs/heads/archive\n`,
  );
  writeFileSync(
    join(git, "logs", "HEAD"),
    reflogLine(ZERO, SHA_B, 1_700_000_000, "commit (initial): begin") +
      reflogLine(SHA_B, SHA_A, 1_700_000_100, "commit: keep\tgoing"),
  );

  writeFileSync(join(workspace, "kept.txt"), "kept\n");
  touch(join(workspace, "kept.txt"), KEPT_MTIME_MS);
  writeFileSync(join(workspace, "drifted.txt"), "drifted longer now\n");
  touch(join(workspace, "drifted.txt"), DRIFT_MTIME_MS);
  writeFileSync(join(workspace, "extra.txt"), "new\n");

  writeFileSync(
    join(git, "index"),
    buildIndex([
      { path: "drifted.txt", size: 8, mtimeMs: DRIFT_MTIME_MS },
      { path: "gone.txt", size: 5, mtimeMs: KEPT_MTIME_MS },
      { path: "kept.txt", size: 5, mtimeMs: KEPT_MTIME_MS },
    ]),
  );
  return workspace;
};

type PathFacts = { count: number; paths: string[] };
type Output = {
  head?: { branch?: string; detached?: string };
  branches?: Array<{ name: string; id: string }>;
  log?: Array<{ id: string; at_ms: number; message: string }>;
  status?: {
    tracked: number;
    changed_candidates: PathFacts;
    untracked: PathFacts;
    missing: PathFacts;
    walk_truncated: boolean;
    comparison: string;
  };
  notes: string[];
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

describe("the git_facts plugin through the sandbox", () => {
  it("reads head, branches, log, and status from the plumbing files", async () => {
    const plugin = load(stageRepo());
    const out = await call(plugin, {});

    expect(out.head).toEqual({ branch: "main" });

    expect(out.branches).toEqual([
      { name: "archive", id: SHA_B },
      { name: "main", id: SHA_A },
    ]);

    expect(out.log).toEqual([
      { id: SHA_A, at_ms: 1_700_000_100_000, message: "commit: keep\tgoing" },
      { id: SHA_B, at_ms: 1_700_000_000_000, message: "commit (initial): begin" },
    ]);

    const status = out.status!;
    expect(status.tracked).toBe(3);
    expect(status.changed_candidates.paths).toEqual(["drifted.txt"]);
    expect(status.untracked.paths).toEqual(["extra.txt"]);
    expect(status.missing.paths).toEqual(["gone.txt"]);
    expect(status.walk_truncated).toBe(false);
    expect(status.comparison).toBe("size_and_mtime_only");
  });

  it("reports only the facts asked for", async () => {
    const plugin = load(stageRepo());
    const out = await call(plugin, { facts: ["head", "log"], max_log: 1 });

    expect(out.head).toEqual({ branch: "main" });
    expect(out.log).toEqual([
      { id: SHA_A, at_ms: 1_700_000_100_000, message: "commit: keep\tgoing" },
    ]);
    expect(out.branches).toBeUndefined();
    expect(out.status).toBeUndefined();
  });

  it("refuses a workspace with no git repository at its root", async () => {
    const workspace = mkdtempSync(join(tmpdir(), "git-facts-bare-"));
    writeFileSync(join(workspace, "README.md"), "no repo here\n");
    const plugin = load(workspace);

    const envelope = await invoke(plugin, {});
    expect(envelope.ok).toBeUndefined();
    expect(envelope.refusal?.code).toBe("unsupported");
    expect(envelope.refusal?.reason).toContain("no git repository");
  });
});
