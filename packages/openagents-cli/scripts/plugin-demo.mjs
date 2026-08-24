#!/usr/bin/env node
/**
 * The plugin walking demo, end to end, from the shell:
 * manifest -> digest verified -> pure-compute check -> invoke -> result,
 * then the refusal paths: a guest refusal, a tampered digest, and a timeout.
 *
 * Run from packages/openagents-cli after a build:
 *
 *     pnpm build && node scripts/plugin-demo.mjs
 *
 * To see it inside the chat instead, load it as a session tool:
 *
 *     printf '/plugin load ../../plugins/word-stats/manifest.json\n<prompt>\n' \
 *       | node dist/main.js coder --plain
 */

import { copyFileSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const dist = resolve(here, "../dist/coder-plugins.js");

let host;
try {
  host = await import(dist);
} catch {
  console.error("Build first: pnpm build (this demo runs against dist/).");
  process.exit(1);
}
const { describeLoad, invokePlugin, isRefusal, loadPluginFromManifest, pluginTool } = host;

const manifestPath = resolve(here, "../../../plugins/word-stats/manifest.json");
const say = (label, text) => console.log(`\n== ${label}\n${text}`);

// 1. Load: manifest read, digest verified, imports proven empty.
const plugin = loadPluginFromManifest(manifestPath);
if (isRefusal(plugin)) {
  console.error(describeLoad(plugin));
  process.exit(1);
}
say("load", describeLoad(plugin));

// 2. Invoke through the tool the manifest materializes, as the model would.
const tool = pluginTool(plugin);
const text = "the quick brown fox jumps over the lazy dog";
say(
  `tool run: ${tool.name}({ text: "${text}" })`,
  await tool.run({ text }, new AbortController().signal),
);

// 3. The guest's own typed refusal, passed through as the output packet.
const bad = await invokePlugin(plugin, new TextEncoder().encode('{"wrong": true}'));
say("guest refusal", isRefusal(bad) ? describeLoad(bad) : new TextDecoder().decode(bad));

// 4. A tampered artifact does not load.
const dir = mkdtempSync(join(tmpdir(), "plugin-demo-"));
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
copyFileSync(
  resolve(dirname(manifestPath), manifest.artifact.path),
  join(dir, manifest.artifact.path),
);
manifest.artifact.digest = `sha256:${"0".repeat(64)}`;
writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
say("tampered digest", describeLoad(loadPluginFromManifest(join(dir, "manifest.json"))));

// 5. A guest that never returns is terminated at the bound.
const spin = new TextEncoder().encode(JSON.stringify({ text: "x", spin: true }));
const started = Date.now();
const timedOut = await invokePlugin(plugin, spin, { timeoutMs: 500 });
say(
  `runaway guest (${String(Date.now() - started)}ms)`,
  isRefusal(timedOut) ? `refused (${timedOut.code}): ${timedOut.reason}` : "unexpectedly answered",
);
