import { mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vitest";

import { runCliWith } from "../src/cli.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

interface Written {
  readonly document: OutputDocument;
  readonly mode: OutputMode;
}

const harness = () => {
  const written: Array<Written> = [];
  const layer = Layer.mergeAll(
    NodeServices.layer,
    environmentLayerFromValues({}),
    persistedConfigurationTestLayer({}),
    terminalSessionTestLayer(false),
    credentialStoreUnavailableLayer,
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        written.push({ document, mode });
      }),
    ),
  );
  const run = (argv: ReadonlyArray<string>) =>
    Effect.runPromise(
      runCliWith([...argv]).pipe(Effect.provide(layer)) as Effect.Effect<void, unknown>,
    );
  const fail = (argv: ReadonlyArray<string>) =>
    Effect.runPromise(
      runCliWith([...argv]).pipe(Effect.provide(layer), Effect.flip) as Effect.Effect<
        unknown,
        unknown
      >,
    );
  return { run, fail, written };
};

const atifDocument = (message: string) => ({
  schema_version: "ATIF-v1.7",
  session_id: "octavia/project-2026-08-24T14:00:00.000Z",
  agent: { name: "openagents-coder", version: "0.3.5", model_name: "Ollama qwen" },
  steps: [
    { step_id: 1, timestamp: "2026-08-24T14:00:00.000Z", source: "user", message },
    {
      step_id: 2,
      timestamp: "2026-08-24T14:00:05.000Z",
      source: "agent",
      message: "done",
      model_name: "Ollama qwen",
      metrics: { prompt_tokens: 5, completion_tokens: 2 },
    },
  ],
  final_metrics: { total_prompt_tokens: 5, total_completion_tokens: 2, total_steps: 2 },
});

const scratchStore = (message = "hello") => {
  const root = mkdtempSync(join(tmpdir(), "trace-command-"));
  const path = join(root, "session-atif.json");
  writeFileSync(path, JSON.stringify(atifDocument(message)), "utf8");
  return { root, path };
};

describe("openagents trace", () => {
  it("lists an explicit store as JSON with scan bounds visible", async () => {
    const { root, path } = scratchStore();
    const { run, written } = harness();
    await run(["--json", "trace", "list", "--path", root]);

    expect(written).toHaveLength(1);
    const value = written[0]?.document.value as {
      schema: string;
      stores: ReadonlyArray<Record<string, unknown>>;
      traces: ReadonlyArray<Record<string, unknown>>;
    };
    expect(value.schema).toBe("openagents.trace_list.v1");
    expect(value.stores).toHaveLength(1);
    expect(value.stores[0]).toMatchObject({
      kind: "trace_path",
      present: true,
      matched: 1,
      listed: 1,
      skipped_symlinks: 0,
      truncated: false,
    });
    expect(value.traces).toHaveLength(1);
    expect(value.traces[0]).toMatchObject({ path, kind: "trace_path" });
  });

  it("skips a planted symlink and says so in the scan", async () => {
    const outside = mkdtempSync(join(tmpdir(), "trace-outside-"));
    writeFileSync(join(outside, "foreign.json"), "{}", "utf8");
    const { root } = scratchStore();
    symlinkSync(join(outside, "foreign.json"), join(root, "linked.json"));

    const { run, written } = harness();
    await run(["--json", "trace", "list", "--path", root]);
    const value = written[0]?.document.value as {
      stores: ReadonlyArray<Record<string, unknown>>;
      traces: ReadonlyArray<{ path: string }>;
    };
    expect(value.stores[0]).toMatchObject({ skipped_symlinks: 1 });
    expect(value.traces.some((trace) => trace.path.includes("linked.json"))).toBe(false);
  });

  it("refuses a non-positive list limit", async () => {
    const { fail } = harness();
    const error = await fail(["trace", "list", "--limit", "0"]);
    expect(error).toMatchObject({ _tag: "OpenAgentsCli.InputError" });
  });

  it("shows a trace summary without printing payloads", async () => {
    const { path } = scratchStore("the payload text that must not leak");
    const { run, written } = harness();
    await run(["--json", "trace", "show", path]);

    const value = written[0]?.document.value as Record<string, unknown>;
    expect(value).toMatchObject({
      schema: "openagents.trace_summary.v1",
      format: "atif",
      schema_version: "ATIF-v1.7",
      steps: 2,
      steps_by_source: { user: 1, agent: 1 },
      models: ["Ollama qwen"],
      total_prompt_tokens: 5,
      total_completion_tokens: 2,
    });
    expect(JSON.stringify(value)).not.toContain("the payload text that must not leak");
  });

  it("refuses to show a trace that does not exist anywhere", async () => {
    const { fail } = harness();
    const error = await fail(["trace", "show", "no-such-trace-file-atif.json"]);
    expect(error).toMatchObject({ _tag: "OpenAgentsCli.InputError" });
  });

  it("redacts into a sibling file and reports counts, not secrets", async () => {
    const { root, path } = scratchStore("token OPENAGENTS_TOKEN=super-secret-value here");
    const { run, written } = harness();
    await run(["--json", "trace", "redact", path]);

    const value = written[0]?.document.value as {
      schema: string;
      output: string;
      counts: Record<string, number>;
      total: number;
      valid_json: boolean | null;
    };
    expect(value.schema).toBe("openagents.trace_redaction.v1");
    expect(value.output).toBe(join(root, "session-atif.redacted.json"));
    expect(value.counts["env_value"]).toBe(1);
    expect(value.valid_json).toBe(true);
    expect(JSON.stringify(value)).not.toContain("super-secret-value");
    expect(readFileSync(value.output, "utf8")).not.toContain("super-secret-value");
    expect(readFileSync(path, "utf8")).toContain("super-secret-value");
  });

  it("refuses to redact an already-redacted copy", async () => {
    const { root } = scratchStore();
    const redacted = join(root, "session-atif.redacted.json");
    writeFileSync(redacted, "{}", "utf8");
    const { fail } = harness();
    const error = await fail(["trace", "redact", redacted]);
    expect(error).toMatchObject({ _tag: "OpenAgentsCli.InputError" });
  });

  it("refuses upload with a typed error naming the missing server route", async () => {
    const { path } = scratchStore();
    const { fail } = harness();
    const error = await fail(["trace", "upload", path]);
    expect(error).toMatchObject({ _tag: "OpenAgentsCli.TraceUploadUnsupported" });
    expect(String((error as { message: string }).message)).toContain("POST /api/v1/traces");
  });

  it("still validates the local path before refusing an upload", async () => {
    const { fail } = harness();
    const error = await fail(["trace", "upload", "no-such-trace-file-atif.json"]);
    expect(error).toMatchObject({ _tag: "OpenAgentsCli.InputError" });
  });

  it("refuses --public together with --unlisted", async () => {
    const { path } = scratchStore();
    const { fail } = harness();
    const error = await fail(["trace", "upload", path, "--public", "--unlisted"]);
    expect(error).toMatchObject({ _tag: "OpenAgentsCli.InputError" });
  });
});
