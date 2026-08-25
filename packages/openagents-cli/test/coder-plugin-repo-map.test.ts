/**
 * The repository mapper through the real boundary: the checked-in
 * `repo_map` plugin against a staged polyglot fixture workspace. The
 * mapper's logic is unit-tested against a fake host in
 * `plugins/repo-map/src/tests.rs`; this file proves the same behavior
 * holds through the WASM sandbox — the heuristic outline, exact-name
 * definition lookup, and word-bounded reference counting, with the
 * `node_modules` skip list honored. The manifest's `${workspace}` mount
 * is rewritten to the fixture directory, as every sandbox test does; the
 * host-side `${workspace}` resolution is proven elsewhere.
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

const MANIFEST = fileURLToPath(
  new URL("../../../plugins/repo-map/manifest.json", import.meta.url),
);
const WASM = fileURLToPath(
  new URL("../../../plugins/repo-map/repo_map.wasm", import.meta.url),
);

const stage = (): { plugin: LoadedPlugin } => {
  const dir = mkdtempSync(join(tmpdir(), "repo-map-"));
  const workspace = join(dir, "workspace");

  mkdirSync(join(workspace, "src"), { recursive: true });
  mkdirSync(join(workspace, "lib"), { recursive: true });
  mkdirSync(join(workspace, "node_modules", "pkg"), { recursive: true });

  writeFileSync(
    join(workspace, "src", "main.py"),
    "class Greeter:\n    def greet(self):\n        return greet_all()\n\ndef greet_all():\n    pass\n",
  );
  writeFileSync(
    join(workspace, "src", "app.ts"),
    "export function greetAll(): void {}\nexport class App {}\nexport const run = () => greetAll();\n",
  );
  writeFileSync(
    join(workspace, "src", "map.rs"),
    "pub fn greet_all() -> u32 { 0 }\npub fn twice() -> u32 { greet_all() + greet_all() }\nfn greet_all_extra() {}\n",
  );
  writeFileSync(
    join(workspace, "lib", "demo.ex"),
    "defmodule Demo do\n  def greet_all do\n    :ok\n  end\nend\n",
  );
  writeFileSync(join(workspace, "node_modules", "pkg", "index.js"), "function hidden() {}\n");

  const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
    capabilities: { mounts: Array<{ path: string; readonly: true }> };
  };
  manifest.capabilities.mounts = [{ path: workspace, readonly: true }];
  writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
  copyFileSync(WASM, join(dir, "repo_map.wasm"));

  const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return { plugin: outcome };
};

type Symbol = { kind: string; name: string; line: number; parent?: string };
type Output = {
  files?: Array<{ path: string; language: string; symbols: Symbol[] | null }>;
  definitions?: Array<{ path: string; kind: string; line: number }>;
  references?: Array<{ path: string; count: number }>;
  total?: number;
  files_seen: number;
  files_parsed: number;
  oversized: number;
  unreadable: number;
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

describe("the repo_map plugin through the sandbox", () => {
  it("outlines the workspace per language and skips node_modules", async () => {
    const { plugin } = stage();
    const out = await call(plugin, {});

    const paths = (out.files ?? []).map((file) => file.path);
    expect(paths).toEqual(["lib/demo.ex", "src/app.ts", "src/main.py", "src/map.rs"]);
    expect(paths.some((path) => path.includes("node_modules"))).toBe(false);

    const byPath = new Map((out.files ?? []).map((file) => [file.path, file]));
    expect(byPath.get("src/main.py")?.language).toBe("python");
    expect(
      byPath.get("src/main.py")?.symbols?.map((s) => [s.kind, s.name, s.line]),
    ).toEqual([
      ["class", "Greeter", 1],
      ["method", "greet", 2],
      ["function", "greet_all", 5],
    ]);
    expect(byPath.get("src/main.py")?.symbols?.[1]?.parent).toBe("Greeter");
    expect(
      byPath.get("src/app.ts")?.symbols?.map((s) => [s.kind, s.name]),
    ).toEqual([
      ["function", "greetAll"],
      ["class", "App"],
      ["function", "run"],
    ]);
    expect(
      byPath.get("lib/demo.ex")?.symbols?.map((s) => [s.kind, s.name]),
    ).toEqual([
      ["module", "Demo"],
      ["function", "greet_all"],
    ]);
    expect(
      byPath.get("src/map.rs")?.symbols?.map((s) => s.name),
    ).toEqual(["greet_all", "twice", "greet_all_extra"]);

    expect(out.files_seen).toBe(4);
    expect(out.files_parsed).toBe(4);
    expect(out.oversized).toBe(0);
    expect(out.truncated).toBe(false);
  });

  it("looks a definition up by exact name across languages", async () => {
    const { plugin } = stage();
    const out = await call(plugin, { symbol: "greet_all" });

    expect(out.definitions).toEqual([
      { path: "lib/demo.ex", kind: "function", line: 2 },
      { path: "src/main.py", kind: "function", line: 5 },
      { path: "src/map.rs", kind: "function", line: 1 },
    ]);
  });

  it("counts whole-word references, skipping definition lines", async () => {
    const { plugin } = stage();
    const out = await call(plugin, { symbol: "greet_all", count_references: true });

    // map.rs: two calls in `twice`; main.py: one call in `greet`. The
    // definitions and `greet_all_extra` never count.
    expect(out.references).toEqual([
      { path: "src/map.rs", count: 2 },
      { path: "src/main.py", count: 1 },
    ]);
    expect(out.total).toBe(3);
  });

  it("refuses to count references without a symbol", async () => {
    const { plugin } = stage();
    const envelope = await invoke(plugin, { count_references: true });

    expect(envelope.ok).toBeUndefined();
    expect(envelope.refusal?.code).toBe("unsupported");
    expect(envelope.refusal?.reason).toContain("symbol");
  });
});
