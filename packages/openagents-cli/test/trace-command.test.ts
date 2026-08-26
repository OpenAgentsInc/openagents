import { mkdtempSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vitest";

import { apiTransportTestLayer, type ApiRequest, type ApiResponse } from "../src/api-transport.js";
import { runCliWith } from "../src/cli.js";
import { traceClientLayer } from "../src/trace-client.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

interface Written {
  readonly document: OutputDocument;
  readonly mode: OutputMode;
}

const harness = (
  handler: (input: ApiRequest) => Effect.Effect<ApiResponse, never> = () =>
    Effect.succeed({ status: 500, body: {} }),
  token?: string,
) => {
  const written: Array<Written> = [];
  const requests: Array<ApiRequest> = [];
  const transport = apiTransportTestLayer((input) =>
    Effect.suspend(() => {
      requests.push(input);
      return handler(input);
    }),
  );
  const layer = Layer.mergeAll(
    NodeServices.layer,
    environmentLayerFromValues(token === undefined ? {} : { token }),
    persistedConfigurationTestLayer({}),
    terminalSessionTestLayer(false),
    credentialStoreUnavailableLayer,
    traceClientLayer.pipe(Layer.provide(transport)),
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
  return { run, fail, written, requests };
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

  it("uploads the document to the ingest route at the dark default", async () => {
    const { path } = scratchStore();
    const { requests, run, written } = harness(
      () =>
        Effect.succeed({
          status: 201,
          body: {
            id: "trace-1",
            url: "https://openagents.com/api/v1/traces/trace-1",
            digest: "sha256:" + "a".repeat(64),
            byte_size: 412,
            visibility: "dark",
            inserted_at: "2026-08-26T03:00:00Z",
          },
        }),
      "test-token",
    );

    await run(["--profile", "local", "trace", "upload", path]);

    expect(requests[0]?.method).toBe("POST");
    // The visibility the server stores at is named explicitly rather than
    // relying on a default the CLI cannot see.
    expect(requests[0]?.path).toBe("/api/v1/traces?visibility=dark");
    // The body is the document itself, with nothing wrapped around it.
    expect((requests[0]?.body as Record<string, unknown>)["schema_version"]).toBe("ATIF-v1.7");

    const human = written[0]?.document.human ?? [];
    expect(human[0]).toContain("Uploaded");
    expect(human).toContain("Trace: trace-1");
    // No link is printed: the url the server returns points at
    // GET /api/v1/traces/:id, and that route does not exist.
    expect(human.join("\n")).not.toContain("http");
  });

  it("reports a 200 as already stored rather than as an upload", async () => {
    const { path } = scratchStore();
    const { run, written } = harness(
      () =>
        Effect.succeed({
          status: 200,
          body: {
            id: "trace-1",
            digest: "sha256:" + "b".repeat(64),
            byte_size: 412,
            visibility: "dark",
            inserted_at: "2026-08-26T03:00:00Z",
          },
        }),
      "test-token",
    );

    await run(["--profile", "local", "trace", "upload", path]);

    // The server answers 200 for a digest it already holds. Calling that an
    // upload would report a write that did not happen.
    expect(written[0]?.document.human?.[0]).toContain("Already stored");
  });

  it("passes a named visibility and an attempt binding through to the route", async () => {
    const { path } = scratchStore();
    const { requests, run } = harness(
      () =>
        Effect.succeed({
          status: 201,
          body: {
            id: "trace-2",
            digest: "sha256:" + "c".repeat(64),
            byte_size: 1,
            visibility: "ledger",
            inserted_at: "2026-08-26T03:00:00Z",
          },
        }),
      "test-token",
    );

    await run([
      "--profile",
      "local",
      "trace",
      "upload",
      path,
      "--visibility",
      "ledger",
      "--assignment",
      "asg-9",
    ]);

    const url = new URL(requests[0]?.path ?? "", "http://localhost/");
    expect(url.searchParams.get("visibility")).toBe("ledger");
    expect(url.searchParams.get("assignment_id")).toBe("asg-9");
  });

  it("refuses a visibility the server does not have, and names the ones it does", async () => {
    const { path } = scratchStore();
    const { fail, requests } = harness(() => Effect.succeed({ status: 201, body: {} }), "t");
    const error = await fail([
      "--profile",
      "local",
      "trace",
      "upload",
      path,
      "--visibility",
      "public",
    ]);

    expect(error).toMatchObject({ _tag: "OpenAgentsCli.InputError" });
    expect(String((error as { message: string }).message)).toContain("dark, pulse, ledger, glass");
    // Refused before anything left this machine.
    expect(requests).toHaveLength(0);
  });

  it("refuses a file that is not an ATIF document before sending it", async () => {
    const { root } = scratchStore();
    const notAtif = join(root, "notes.json");
    writeFileSync(notAtif, JSON.stringify({ hello: "world" }), "utf8");
    const { fail, requests } = harness(() => Effect.succeed({ status: 201, body: {} }), "t");

    const error = await fail(["--profile", "local", "trace", "upload", notAtif]);

    expect(error).toMatchObject({ _tag: "OpenAgentsCli.InputError" });
    expect(String((error as { message: string }).message)).toContain("schema_version");
    expect(requests).toHaveLength(0);
  });

  it("refuses an accepted status that names nothing stored", async () => {
    const { path } = scratchStore();
    // 201 with an empty body: the server said yes and said nothing. Reporting a
    // stored trace here is how a caller comes to believe in one that has no id.
    const { fail } = harness(() => Effect.succeed({ status: 201, body: {} }), "test-token");

    const error = await fail(["--profile", "local", "trace", "upload", path]);

    expect(error).toMatchObject({ _tag: "OpenAgentsCli.ApiError" });
    expect(String((error as { message: string }).message)).toContain("did not say what it stored");
  });

  it("still validates the local path before reaching for the network", async () => {
    const { fail, requests } = harness();
    const error = await fail(["trace", "upload", "no-such-trace-file-atif.json"]);
    expect(error).toMatchObject({ _tag: "OpenAgentsCli.InputError" });
    expect(requests).toHaveLength(0);
  });
});
