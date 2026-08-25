import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vitest";

import { apiTransportTestLayer, type ApiRequest, type ApiResponse } from "../src/api-transport.js";
import { runCliWith } from "../src/cli.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { gitRunnerTestLayer } from "../src/git-runner.js";
import { memoryClientLayer } from "../src/memory-client.js";
import { outputTestLayer, type OutputDocument, type OutputMode } from "../src/output.js";
import { persistedConfigurationTestLayer } from "../src/persisted-configuration.js";
import { requestBodyInputTestLayer } from "../src/request-body-input.js";
import { secretInputTestLayer } from "../src/secret-input.js";
import { terminalSessionTestLayer } from "../src/terminal-session.js";

interface Written {
  readonly document: OutputDocument;
  readonly mode: OutputMode;
}

const harness = (
  handler: (input: ApiRequest) => Effect.Effect<ApiResponse, never>,
  environment: Readonly<Record<string, string>> = { token: "test-token" },
) => {
  const written: Array<Written> = [];
  const requests: Array<ApiRequest> = [];
  const transport = apiTransportTestLayer((input) => {
    requests.push(input);
    return handler(input);
  });
  const layer = Layer.mergeAll(
    NodeServices.layer,
    environmentLayerFromValues(environment),
    persistedConfigurationTestLayer({}),
    terminalSessionTestLayer(false),
    credentialStoreUnavailableLayer,
    gitRunnerTestLayer(() => Effect.void),
    secretInputTestLayer("stdin-token"),
    requestBodyInputTestLayer({}),
    memoryClientLayer.pipe(Layer.provide(transport)),
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        written.push({ document, mode });
      }),
    ),
  );
  return { written, requests, layer };
};

const memoryRow = {
  id: "mem_1",
  bucket: "user",
  body: "Uses pnpm, not npm.",
  source_ref: "thread-7",
  superseded_by: null,
  created_at: "2026-08-25T12:00:00Z",
};

describe("openagents memory CLI commands", () => {
  it("lists the account's memories", async () => {
    const { layer, written } = harness((req) => {
      if (req.path === "/api/v1/memories" && req.method === "GET") {
        return Effect.succeed({ status: 200, body: { memories: [memoryRow] } });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(runCliWith(["memory", "list"]).pipe(Effect.provide(layer)));

    expect(written.length).toBe(1);
    const human = written[0]!.document.human;
    expect(human.some((line) => line.includes("mem_1"))).toBe(true);
    expect(human.some((line) => line.includes("Uses pnpm, not npm."))).toBe(true);
  });

  it("narrows the listing with the bucket, limit, and superseded flags", async () => {
    const { layer, requests } = harness((req) => {
      if (req.method === "GET" && req.path.startsWith("/api/v1/memories")) {
        return Effect.succeed({ status: 200, body: { memories: [] } });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith([
        "memory",
        "list",
        "--bucket",
        "learned",
        "--limit",
        "5",
        "--include-superseded",
      ]).pipe(Effect.provide(layer)),
    );

    expect(requests[0]!.path).toContain("bucket=learned");
    expect(requests[0]!.path).toContain("limit=5");
    expect(requests[0]!.path).toContain("include_superseded=true");
  });

  it("says the store is empty rather than printing nothing", async () => {
    const { layer, written } = harness(() =>
      Effect.succeed({ status: 200, body: { memories: [] } }),
    );

    await Effect.runPromise(runCliWith(["memory", "list"]).pipe(Effect.provide(layer)));

    expect(written[0]!.document.human.some((line) => line.includes("No memories"))).toBe(true);
  });

  it("stores a memory in the user bucket", async () => {
    const { layer, requests, written } = harness((req) => {
      if (req.path === "/api/v1/memories" && req.method === "POST") {
        return Effect.succeed({ status: 201, body: { memory: memoryRow } });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith(["memory", "add", "Uses", "pnpm,", "not", "npm."]).pipe(Effect.provide(layer)),
    );

    expect(requests[0]!.body).toMatchObject({ body: "Uses pnpm, not npm.", bucket: "user" });
    const human = written[0]!.document.human;
    expect(human.some((line) => line.includes("Stored memory mem_1"))).toBe(true);
  });

  // A correction is a new row, never an edit: the server has no `PATCH`, and
  // the chain a wrong memory was corrected through is what makes it traceable.
  it("sends a correction as a new memory carrying supersedes", async () => {
    const { layer, requests, written } = harness((req) => {
      if (req.path === "/api/v1/memories" && req.method === "POST") {
        return Effect.succeed({
          status: 201,
          body: { memory: { ...memoryRow, id: "mem_2", body: "Uses bun, not pnpm." } },
        });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith(["memory", "add", "--supersedes", "mem_1", "Uses", "bun,", "not", "pnpm."]).pipe(
        Effect.provide(layer),
      ),
    );

    expect(requests[0]!.method).toBe("POST");
    expect(requests[0]!.body).toMatchObject({ supersedes: "mem_1" });
    expect(written[0]!.document.human.some((line) => line.includes("Supersedes mem_1"))).toBe(true);
  });

  it("removes one memory", async () => {
    const { layer, requests, written } = harness((req) => {
      if (req.path === "/api/v1/memories/mem_1" && req.method === "DELETE") {
        return Effect.succeed({ status: 200, body: { memory: memoryRow } });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(runCliWith(["memory", "delete", "mem_1"]).pipe(Effect.provide(layer)));

    expect(requests[0]!.method).toBe("DELETE");
    expect(written[0]!.document.human.some((line) => line.includes("Removed memory mem_1"))).toBe(
      true,
    );
  });

  it("reports a memory the server does not hold", async () => {
    const { layer } = harness(() =>
      Effect.succeed({
        status: 404,
        body: { code: "not_found", message: "No memory with that id.", errors: {} },
      }),
    );

    const failure = await Effect.runPromise(
      runCliWith(["memory", "delete", "mem_gone"]).pipe(Effect.provide(layer), Effect.flip),
    );

    expect(JSON.stringify(failure)).toContain("not_found");
  });

  // The refusal has to survive as a refusal. A write that was rejected and
  // reported as a success would tell the reader their preference is stored.
  it("surfaces a rejected credential rather than reporting a write", async () => {
    const { layer, written } = harness(() =>
      Effect.succeed({
        status: 401,
        body: {
          code: "unauthenticated",
          error: "invalid_api_token",
          message: "Requires an API token with the scope this route needs",
          errors: {},
        },
      }),
    );

    const failure = await Effect.runPromise(
      runCliWith(["memory", "add", "Uses", "pnpm."]).pipe(Effect.provide(layer), Effect.flip),
    );

    expect(written.length).toBe(0);
    expect(JSON.stringify(failure)).toContain("unauthenticated");
  });

  it("surfaces the quota refusal with the reason the server gave", async () => {
    const { layer, written } = harness(() =>
      Effect.succeed({
        status: 429,
        body: {
          code: "memory_quota_reached",
          message:
            "This account already holds 200 memories. Remove one, or supersede one, before writing another.",
          errors: {},
        },
      }),
    );

    const failure = await Effect.runPromise(
      runCliWith(["memory", "add", "One", "more", "thing."]).pipe(
        Effect.provide(layer),
        Effect.flip,
      ),
    );

    expect(written.length).toBe(0);
    const reported = JSON.stringify(failure);
    expect(reported).toContain("memory_quota_reached");
    expect(reported).toContain("supersede one");
  });

  it("names the rejected field on a validation refusal", async () => {
    const { layer } = harness(() =>
      Effect.succeed({
        status: 422,
        body: {
          code: "validation_failed",
          message: "The request could not be processed",
          errors: { supersedes: ["names no live memory of this account"] },
        },
      }),
    );

    const failure = await Effect.runPromise(
      runCliWith(["memory", "add", "--supersedes", "mem_absent", "Something."]).pipe(
        Effect.provide(layer),
        Effect.flip,
      ),
    );

    expect(JSON.stringify(failure)).toContain("names no live memory");
  });

  it("refuses a bucket the server would reject, without a round trip", async () => {
    const { layer, requests } = harness(() => Effect.succeed({ status: 200, body: {} }));

    const failure = await Effect.runPromise(
      runCliWith(["memory", "list", "--bucket", "system"]).pipe(Effect.provide(layer), Effect.flip),
    );

    expect(requests.length).toBe(0);
    expect(JSON.stringify(failure)).toContain("--bucket");
  });

  it("refuses without a credential rather than writing nowhere", async () => {
    const { layer, requests } = harness(() => Effect.succeed({ status: 201, body: {} }), {});

    const failure = await Effect.runPromise(
      runCliWith(["memory", "add", "Uses", "pnpm."]).pipe(Effect.provide(layer), Effect.flip),
    );

    expect(requests.length).toBe(0);
    expect(JSON.stringify(failure)).toContain("token");
  });
});
