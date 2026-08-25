/**
 * Read-only mounts, proved against the checked-in `file_stats` plugin:
 * a declared mount grants exactly the `openagents.read_file` capability
 * import, confinement refuses every escape (`..`, absolute paths,
 * symlinks), the per-file size bound holds, an undeclared mount refuses at
 * load, and an unknown ABI never loads at all.
 */

import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  symlinkSync,
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

const FILE_STATS_MANIFEST = fileURLToPath(
  new URL("../../../plugins/file-stats/manifest.json", import.meta.url),
);
const FILE_STATS_WASM = fileURLToPath(
  new URL("../../../plugins/file-stats/file_stats.wasm", import.meta.url),
);
const WORD_STATS_MANIFEST = fileURLToPath(
  new URL("../../../plugins/word-stats/manifest.json", import.meta.url),
);

/**
 * Stage a private copy of the file_stats plugin in a temp directory with an
 * empty `data/` mount, so a test can shape the mount's contents (and the
 * manifest) without touching the checked-in fixture.
 */
const stage = (mutateManifest?: (manifest: Record<string, unknown>) => void): string => {
  const dir = mkdtempSync(join(tmpdir(), "plugin-mount-"));
  const manifest = JSON.parse(readFileSync(FILE_STATS_MANIFEST, "utf8")) as Record<string, unknown>;
  mutateManifest?.(manifest);
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  copyFileSync(FILE_STATS_WASM, join(dir, "file_stats.wasm"));
  mkdirSync(join(dir, "data"));
  return dir;
};

const load = (dir: string): LoadedPlugin => {
  const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return outcome;
};

/** Invoke file_stats for one path and parse the output packet. */
const statPath = async (
  plugin: LoadedPlugin,
  path: string,
): Promise<Record<string, Record<string, unknown>>> => {
  const packet = new TextEncoder().encode(JSON.stringify({ path }));
  const outcome = await invokePlugin(plugin, packet);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return JSON.parse(new TextDecoder().decode(outcome)) as Record<string, Record<string, unknown>>;
};

describe("read-only mounts", () => {
  it("loads the checked-in plugin and reads a file inside the mount", async () => {
    const outcome = loadPluginFromManifest(FILE_STATS_MANIFEST);
    if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
    expect(outcome.mounts).toHaveLength(1);

    const answer = await statPath(outcome, "sample.txt");
    expect(answer["ok"]).toMatchObject({ path: "sample.txt", utf8: true, lines: 3 });
    expect(answer["ok"]?.["bytes"]).toBeGreaterThan(0);
  });

  it("reads through a subdirectory of the mount, but never a directory itself", async () => {
    const dir = stage();
    mkdirSync(join(dir, "data", "nested"));
    writeFileSync(join(dir, "data", "nested", "inner.txt"), "one\ntwo\n");
    const plugin = load(dir);

    const ok = await statPath(plugin, "nested/inner.txt");
    expect(ok["ok"]).toMatchObject({ lines: 2 });

    const refused = await statPath(plugin, "nested");
    expect(refused["refusal"]?.["code"]).toBe("file_unreadable");
  });

  it("refuses a `..` path that escapes the mount root", async () => {
    const dir = stage();
    writeFileSync(join(dir, "secret.txt"), "outside the mount");
    const plugin = load(dir);

    const answer = await statPath(plugin, "../secret.txt");
    expect(answer["refusal"]?.["code"]).toBe("mount_denied");
    // The secret's existence must not leak through the wording.
    expect(String(answer["refusal"]?.["reason"])).not.toContain("secret");
  });

  it("refuses an absolute path outside the root", async () => {
    const plugin = load(stage());
    const answer = await statPath(plugin, "/etc/passwd");
    expect(answer["refusal"]?.["code"]).toBe("mount_denied");
  });

  it("refuses a symlink inside the mount, wherever it points", async () => {
    const dir = stage();
    writeFileSync(join(dir, "secret.txt"), "outside the mount");
    symlinkSync(join(dir, "secret.txt"), join(dir, "data", "sneaky.txt"));
    const plugin = load(dir);

    const answer = await statPath(plugin, "sneaky.txt");
    expect(answer["refusal"]?.["code"]).toBe("mount_denied");
  });

  it("bounds the bytes a single read may return", async () => {
    const dir = stage();
    writeFileSync(join(dir, "data", "big.bin"), Buffer.alloc(MOUNT_FILE_LIMIT + 1));
    const plugin = load(dir);

    const answer = await statPath(plugin, "big.bin");
    expect(answer["refusal"]?.["code"]).toBe("file_too_large");
  });

  it("answers a missing file with a refusal, not a trap", async () => {
    const plugin = load(stage());
    const answer = await statPath(plugin, "not-there.txt");
    expect(answer["refusal"]?.["code"]).toBe("mount_denied");
  });

  it("refuses at load a module whose imports its manifest does not declare", () => {
    // file_stats imports openagents.read_file; strip the mount declaration
    // and the import is no longer granted by anything.
    const dir = stage((manifest) => {
      (manifest["capabilities"] as Record<string, unknown>)["mounts"] = [];
    });
    const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
    expect(isRefusal(outcome) && outcome.code).toBe("imports_undeclared");
  });

  it("refuses a mount that is not declared read-only", () => {
    const dir = stage((manifest) => {
      (manifest["capabilities"] as Record<string, unknown>)["mounts"] = [
        { path: "data", readonly: false },
      ];
    });
    const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
    expect(isRefusal(outcome) && outcome.code).toBe("capabilities_unsupported");
  });

  it("refuses a mount that does not resolve to a directory", () => {
    const dir = stage((manifest) => {
      (manifest["capabilities"] as Record<string, unknown>)["mounts"] = [
        { path: "no-such-dir", readonly: true },
      ];
    });
    const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
    expect(isRefusal(outcome) && outcome.code).toBe("mount_invalid");
  });
});

describe("abi versioning", () => {
  it("refuses a manifest that declares an abi this host does not speak", () => {
    const dir = mkdtempSync(join(tmpdir(), "plugin-abi-"));
    const manifest = JSON.parse(readFileSync(WORD_STATS_MANIFEST, "utf8")) as {
      abi: { kind: string };
      artifact: { path: string };
    };
    manifest.abi.kind = "packet-v9";
    writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
    copyFileSync(
      fileURLToPath(new URL("../../../plugins/word-stats/word_stats.wasm", import.meta.url)),
      join(dir, manifest.artifact.path),
    );

    const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
    expect(isRefusal(outcome) && outcome.code).toBe("abi_unsupported");
    expect(isRefusal(outcome) && outcome.reason).toContain("packet-v0");
  });

  it("loads the declared packet-v0 abi", () => {
    const outcome = loadPluginFromManifest(WORD_STATS_MANIFEST);
    expect(isRefusal(outcome)).toBe(false);
  });
});
