/**
 * The plugin demo, proved end to end against the checked-in artifact:
 * manifest read, digest verified, module inspected, packet in, packet out,
 * refusals typed, timeout enforced by termination, and the whole path
 * surfaced through a coder session as a tool call in the transcript.
 */

import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { PassThrough } from "node:stream";

import { describe, expect, it } from "vitest";

import {
  describeLoad,
  invokePlugin,
  isRefusal,
  loadPluginFromManifest,
  pluginTool,
  type LoadedPlugin,
} from "../src/coder-plugins.js";
import { CoderSession, type ReplyChunk, type ReplySource } from "../src/coder-session.js";
import { runCoderPlain } from "../src/coder-plain.js";
import type { CoderTool } from "../src/coder-tools.js";

const MANIFEST = fileURLToPath(
  new URL("../../../plugins/word-stats/manifest.json", import.meta.url),
);

const loadFixture = (): LoadedPlugin => {
  const outcome = loadPluginFromManifest(MANIFEST);
  if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);
  return outcome;
};

describe("loadPluginFromManifest", () => {
  it("loads the checked-in demo plugin and verifies its digest", () => {
    const plugin = loadFixture();
    expect(plugin.manifest.name).toBe("word_stats");
    expect(plugin.digest).toBe(plugin.manifest.artifact.digest);
    expect(describeLoad(plugin)).toContain("digest verified");
  });

  it("refuses an artifact whose digest does not match the manifest's pin", () => {
    const dir = mkdtempSync(join(tmpdir(), "plugin-digest-"));
    const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
      artifact: { digest: string; path: string };
    };
    manifest.artifact.digest = `sha256:${"0".repeat(64)}`;
    writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));
    writeFileSync(
      join(dir, manifest.artifact.path),
      readFileSync(
        fileURLToPath(new URL("../../../plugins/word-stats/word_stats.wasm", import.meta.url)),
      ),
    );

    const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
    expect(isRefusal(outcome) && outcome.code).toBe("digest_mismatch");
  });

  it("refuses a manifest that declares mounts or hosts", () => {
    const dir = mkdtempSync(join(tmpdir(), "plugin-caps-"));
    const manifest = JSON.parse(readFileSync(MANIFEST, "utf8")) as {
      capabilities: { hosts: unknown[] };
    };
    manifest.capabilities.hosts = ["api.example.com"];
    writeFileSync(join(dir, "manifest.json"), JSON.stringify(manifest));

    const outcome = loadPluginFromManifest(join(dir, "manifest.json"));
    expect(isRefusal(outcome) && outcome.code).toBe("capabilities_unsupported");
  });
});

describe("invokePlugin", () => {
  it("answers a packet with the statistics the guest computed", async () => {
    const plugin = loadFixture();
    const packet = new TextEncoder().encode(
      JSON.stringify({ text: "the quick brown fox jumps over the lazy dog" }),
    );
    const outcome = await invokePlugin(plugin, packet);
    if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);

    const parsed = JSON.parse(new TextDecoder().decode(outcome)) as {
      ok: { words: number; top_word: { word: string; count: number } };
    };
    expect(parsed.ok.words).toBe(9);
    expect(parsed.ok.top_word).toEqual({ word: "the", count: 2 });
  });

  it("passes the guest's own typed refusal through as the output packet", async () => {
    const plugin = loadFixture();
    const outcome = await invokePlugin(plugin, new TextEncoder().encode('{"nope":1}'));
    if (isRefusal(outcome)) throw new Error(`${outcome.code}: ${outcome.reason}`);

    const parsed = JSON.parse(new TextDecoder().decode(outcome)) as {
      refusal: { code: string };
    };
    expect(parsed.refusal.code).toBe("bad_packet");
  });

  it("terminates a guest that never returns, at the declared bound", async () => {
    const plugin = loadFixture();
    const packet = new TextEncoder().encode(JSON.stringify({ text: "x", spin: true }));
    const started = Date.now();
    const outcome = await invokePlugin(plugin, packet, { timeoutMs: 400 });
    expect(isRefusal(outcome) && outcome.code).toBe("timeout");
    // Generous ceiling: the point is that it came back at the bound, not at
    // the heat death of the worker.
    expect(Date.now() - started).toBeLessThan(5_000);
  });
});

describe("pluginTool", () => {
  it("materializes the manifest as a CoderTool the model can call", async () => {
    const tool = pluginTool(loadFixture());
    expect(tool.name).toBe("word_stats");
    expect(tool.parameters).toMatchObject({ type: "object", required: ["text"] });

    const answer = await tool.run({ text: "one two two" }, new AbortController().signal);
    expect(JSON.parse(answer)).toMatchObject({ ok: { words: 3 } });
  });

  it("reports a host refusal as a sentence rather than throwing", async () => {
    const plugin = loadFixture();
    const short: LoadedPlugin = {
      ...plugin,
      manifest: {
        ...plugin.manifest,
        capabilities: { ...plugin.manifest.capabilities, timeout_ms: 200 },
      },
    };
    const tool = pluginTool(short);
    const answer = await tool.run({ text: "x", spin: true }, new AbortController().signal);
    expect(answer).toContain("The plugin refused (timeout)");
  });
});

/**
 * A source that behaves the way the thread source does with tools: it takes
 * the declaration, and its one reply calls the plugin tool and reports the
 * call as chunks. This is the transcript proof: the tool a `/plugin load`
 * registered is reachable through the session loop and renders in `--plain`.
 */
class PluginCallingSource implements ReplySource {
  readonly model = "scripted";
  private tools: ReadonlyArray<CoderTool> = [];

  useTools(tools: ReadonlyArray<CoderTool>): void {
    this.tools = tools;
  }

  async *reply(prompt: string, signal: AbortSignal): AsyncIterable<ReplyChunk> {
    const tool = this.tools.find((candidate) => candidate.name === "word_stats");
    if (tool === undefined) {
      yield { type: "text", value: "No word_stats tool is declared." };
      return;
    }
    const args = { text: prompt };
    yield { type: "tool_call", callId: "call-1", name: tool.name, arguments: JSON.stringify(args) };
    const output = await tool.run(args, signal);
    yield { type: "tool_result", callId: "call-1", output, error: undefined };
    yield { type: "text", value: `The plugin answered: ${output}` };
  }
}

describe("the transcript", () => {
  it("shows /plugin load registering the tool and a turn calling it", async () => {
    const source = new PluginCallingSource();
    const session = new CoderSession(source, "openagents", "coder-plugin-demo");

    // The same wiring `openagents coder` does: the loader owns the registry
    // and re-declares the tools when a plugin lands.
    const plugins: LoadedPlugin[] = [];
    const declare = () => {
      source.useTools(plugins.map((plugin) => pluginTool(plugin)));
    };
    declare();
    const loadPlugin = (path: string): string => {
      const outcome = loadPluginFromManifest(path);
      if (isRefusal(outcome)) return describeLoad(outcome);
      plugins.push(outcome);
      declare();
      return describeLoad(outcome);
    };

    const stdin = new PassThrough();
    const stdout = new PassThrough();
    let transcript = "";
    stdout.on("data", (chunk: Buffer) => {
      transcript += chunk.toString("utf8");
    });

    const done = runCoderPlain(session, { stdin, stdout, skills: undefined, loadPlugin });
    stdin.write(`/plugin load ${MANIFEST}\n`);
    stdin.write("the quick brown fox\n");
    stdin.end();
    expect(await done).toBe(0);

    expect(transcript).toContain("Loaded plugin `word_stats`");
    expect(transcript).toContain("digest verified");
    expect(transcript).toContain("[tool] word_stats");
    expect(transcript).toContain('"words":4');
  });
});
