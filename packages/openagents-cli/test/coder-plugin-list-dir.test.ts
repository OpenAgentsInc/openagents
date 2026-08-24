/**
 * Mounted directory listings, proved against the checked-in `dir_stats`
 * plugin: a declared mount grants the `openagents.list_dir` capability
 * import, confinement refuses every escape (`..`, absolute paths,
 * symlinks, out-of-range mount indices), the per-listing entry bound
 * holds, and mount roots may be declared absolute or `~`-relative.
 */

import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  MOUNT_DIR_ENTRY_LIMIT,
  expandMountPath,
  invokePlugin,
  isRefusal,
  loadPluginFromManifest,
  type LoadedPlugin,
} from "../src/coder-plugins.js";

const DIR_STATS_MANIFEST = fileURLToPath(
  new URL("../../../plugins/dir-stats/manifest.json", import.meta.url),
);
const DIR_STATS_WASM = fileURLToPath(
  new URL("../../../plugins/dir-stats/dir_stats.wasm", import.meta.url),
);

/**
 * Stage a private copy of the dir_stats plugin with an empty `data/`
 * mount, so a test can shape the mount's contents and the manifest.
 */
const stage = (mutateManifest?: (manifest: Record<string, unknown>) => void): string => {
  const dir = mkdtempSync(join(tmpdir(), "plugin-listdir-"));
  const manifest = JSON.parse(readFileSync(DIR_STATS_MANIFEST, "utf8")) as Record<
    string,
    unknown
  >;
  mutateManifest?.(manifest);
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  copyFileSync(DIR_STATS_WASM, join(dir, "dir_stats.wasm"));
  mkdirSync(join(dir, "data"));
  return dir;
};

const load = (dir: string): LoadedPlugin => {
  const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return outcome;
};

/** Invoke dir_stats for one listing and parse the output packet. */
const list = async (
  plugin: LoadedPlugin,
  path: string,
  mountIndex = 0,
): Promise<Record<string, Record<string, unknown> | undefined>> => {
  const packet = new TextEncoder().encode(JSON.stringify({ mount_index: mountIndex, path }));
  const outcome = await invokePlugin(plugin, packet);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return JSON.parse(new TextDecoder().decode(outcome)) as Record<
    string,
    Record<string, unknown> | undefined
  >;
};

type Entry = { name: string; kind: string; size: number; mtime_ms: number };

describe("mounted directory listings", () => {
  it("lists the mount root with names, kinds, sizes, and mtimes", async () => {
    const dir = stage();
    writeFileSync(join(dir, "data", "a.jsonl"), "one\n");
    mkdirSync(join(dir, "data", "nested"));
    const plugin = load(dir);

    const answer = await list(plugin, "");
    const entries = answer["ok"]?.["entries"] as Entry[];
    expect(entries.map((e) => [e.name, e.kind])).toEqual([
      ["a.jsonl", "file"],
      ["nested", "dir"],
    ]);
    expect(entries[0]?.size).toBe(4);
    expect(entries[0]?.mtime_ms).toBeGreaterThan(0);
    expect(answer["ok"]?.["truncated"]).toBe(false);
  });

  it("lists a subdirectory and reports symlink entries without following them", async () => {
    const dir = stage();
    mkdirSync(join(dir, "data", "nested"));
    writeFileSync(join(dir, "secret.txt"), "outside");
    symlinkSync(join(dir, "secret.txt"), join(dir, "data", "nested", "sneaky"));
    const plugin = load(dir);

    const answer = await list(plugin, "nested");
    const entries = answer["ok"]?.["entries"] as Entry[];
    expect(entries).toHaveLength(1);
    expect(entries[0]).toMatchObject({ name: "sneaky", kind: "symlink" });
  });

  it("refuses a `..` path that escapes the mount root", async () => {
    const plugin = load(stage());
    const answer = await list(plugin, "..");
    expect(answer["refusal"]?.["code"]).toBe("mount_denied");
  });

  it("refuses an absolute path", async () => {
    const plugin = load(stage());
    const answer = await list(plugin, "/etc");
    expect(answer["refusal"]?.["code"]).toBe("mount_denied");
  });

  it("refuses to list through a symlinked directory", async () => {
    const dir = stage();
    mkdirSync(join(dir, "outside"));
    symlinkSync(join(dir, "outside"), join(dir, "data", "portal"));
    const plugin = load(dir);

    const answer = await list(plugin, "portal");
    expect(answer["refusal"]?.["code"]).toBe("mount_denied");
  });

  it("refuses a mount index that names no declared mount", async () => {
    const plugin = load(stage());
    const answer = await list(plugin, "", 7);
    expect(answer["refusal"]?.["code"]).toBe("mount_denied");
  });

  it("answers a missing directory with a refusal, not a trap", async () => {
    const plugin = load(stage());
    const answer = await list(plugin, "no-such-dir");
    expect(answer["refusal"]?.["code"]).toBe("file_unreadable");
  });

  it("answers a file path with a refusal: only directories list", async () => {
    const dir = stage();
    writeFileSync(join(dir, "data", "plain.txt"), "x");
    const plugin = load(dir);
    const answer = await list(plugin, "plain.txt");
    expect(answer["refusal"]?.["code"]).toBe("file_unreadable");
  });

  it("bounds the entries per listing and says so", async () => {
    const dir = stage();
    for (let i = 0; i < MOUNT_DIR_ENTRY_LIMIT + 1; i += 1) {
      writeFileSync(join(dir, "data", `f${String(i).padStart(4, "0")}`), "");
    }
    const plugin = load(dir);

    const answer = await list(plugin, "");
    const entries = answer["ok"]?.["entries"] as Entry[] | undefined;
    expect(entries).toHaveLength(MOUNT_DIR_ENTRY_LIMIT);
    expect(answer["ok"]?.["truncated"]).toBe(true);
  });
});

describe("mount root declarations", () => {
  it("accepts an absolute mount root that exists and is a directory", async () => {
    const outside = mkdtempSync(join(tmpdir(), "plugin-absmount-"));
    writeFileSync(join(outside, "here.txt"), "hi");
    const dir = stage((manifest) => {
      (manifest["capabilities"] as Record<string, unknown>)["mounts"] = [
        { path: outside, readonly: true },
      ];
    });
    const plugin = load(dir);
    expect(plugin.mounts).toHaveLength(1);

    const answer = await list(plugin, "");
    const entries = answer["ok"]?.["entries"] as Entry[];
    expect(entries.map((e) => e.name)).toContain("here.txt");
  });

  it("refuses an absolute mount root that does not exist", () => {
    const dir = stage((manifest) => {
      (manifest["capabilities"] as Record<string, unknown>)["mounts"] = [
        { path: join(tmpdir(), "definitely-not-a-real-mount-root"), readonly: true },
      ];
    });
    const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
    expect(isRefusal(outcome) && outcome.code).toBe("mount_invalid");
  });

  it("expands `~` and `~/` to the invoking user's home, and nothing else", () => {
    expect(expandMountPath("~")).toBe(homedir());
    expect(expandMountPath("~/.claude")).toBe(join(homedir(), ".claude"));
    expect(expandMountPath("~alice/secrets")).toBe("~alice/secrets");
    expect(expandMountPath("data")).toBe("data");
    expect(expandMountPath("/absolute")).toBe("/absolute");
  });
});
