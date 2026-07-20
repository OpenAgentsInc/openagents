import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { afterEach, describe, expect, test } from "vite-plus/test";

import { ideDebugBindingFor } from "./dap-host.ts";
import { discoverIdeDebugManifest } from "./debug-discovery.ts";

const roots: Array<string> = [];

afterEach(async () => {
  await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true })));
});

const workspace = async (): Promise<string> => {
  const root = await mkdtemp(path.join(os.tmpdir(), "openagents-debug-discovery-"));
  roots.push(root);
  await mkdir(path.join(root, ".openagents"), { recursive: true });
  return root;
};

const entry = (changes: Readonly<Record<string, unknown>> = {}) => ({
  ref: "fixture",
  label: "Fixture launch",
  adapterType: "fixture",
  adapterVersion: "1",
  adapterExecutable: "node",
  adapterArguments: ["fixture.cjs"],
  request: "launch",
  startArguments: { program: "fixture.ts" },
  cwd: ".",
  environmentKeys: ["PATH", "API_TOKEN"],
  sourceRoots: ["."],
  remoteRoots: [],
  executableRef: "ide.executable.fixture",
  executableLabel: "Fixture target",
  argumentLabels: [],
  timeoutMs: 30_000,
  ...changes,
});

const writeManifest = async (root: string, entries: ReadonlyArray<unknown>): Promise<void> => {
  await writeFile(
    path.join(root, ".openagents", "debug.json"),
    JSON.stringify({
      schemaVersion: "openagents.desktop.ide-debug-manifest.v1",
      configurations: entries,
    }),
    { encoding: "utf8", mode: 0o600 },
  );
};

describe("IDE-11 main-owned debug discovery", () => {
  test("returns no configurations when the project has no debug manifest", async () => {
    const root = await workspace();
    await expect(
      discoverIdeDebugManifest({ root, binding: ideDebugBindingFor({ root, grantRef: "grant.fixture" }) }),
    ).resolves.toEqual([]);
  });

  test("keeps selected environment values in the main-only adapter resolution", async () => {
    const root = await workspace();
    await writeManifest(root, [entry()]);
    const discovered = await discoverIdeDebugManifest({
      root,
      binding: ideDebugBindingFor({ root, grantRef: "grant.fixture" }),
      environment: { PATH: "/usr/bin", API_TOKEN: "private-fixture-value", UNLISTED: "withheld" },
    });
    expect(discovered).toHaveLength(1);
    expect(discovered[0]?.resolution.environment).toEqual({
      PATH: "/usr/bin",
      API_TOKEN: "private-fixture-value",
    });
    expect(discovered[0]?.configuration.environment.redactedKeys).toEqual(["API_TOKEN"]);
    expect(JSON.stringify(discovered[0]?.configuration)).not.toContain("private-fixture-value");
    expect(JSON.stringify(discovered[0]?.configuration)).not.toContain("UNLISTED");
  });

  test("refuses path escape, absolute adapter commands, and unauthenticated attach", async () => {
    const root = await workspace();
    const binding = ideDebugBindingFor({ root, grantRef: "grant.fixture" });
    await writeManifest(root, [entry({ cwd: "../escape" })]);
    await expect(discoverIdeDebugManifest({ root, binding })).rejects.toThrow(/active project root/u);
    await writeManifest(root, [entry({ adapterExecutable: "/tmp/unadmitted-adapter" })]);
    await expect(discoverIdeDebugManifest({ root, binding })).rejects.toThrow(/PATH command/u);
    await writeManifest(root, [entry({
      request: "attach",
      transportRef: "ide.debug-transport.fixture",
      targetProcessRef: "ide.process.fixture",
      targetProcessLabel: "Fixture process",
    })]);
    const [attach] = await discoverIdeDebugManifest({ root, binding });
    expect(attach?.configuration.admitted).toBe(false);
    expect(attach?.configuration.refusalReason).toMatch(/authentication reference/u);
  });
});
