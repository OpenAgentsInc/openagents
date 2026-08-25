import * as NodeServices from "@effect/platform-node/NodeServices";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "vitest";

import { apiTransportTestLayer, type ApiRequest, type ApiResponse } from "../src/api-transport.js";
import { boxClientLayer } from "../src/box-client.js";
import { runCliWith } from "../src/cli.js";
import { credentialStoreUnavailableLayer } from "../src/credential-store.js";
import { environmentLayerFromValues } from "../src/environment.js";
import { gitRunnerTestLayer } from "../src/git-runner.js";
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
  standardInput: Readonly<Record<string, string>> = {},
) => {
  const written: Array<Written> = [];
  const transport = apiTransportTestLayer(handler);
  const layer = Layer.mergeAll(
    NodeServices.layer,
    environmentLayerFromValues({ token: "test-token" }),
    persistedConfigurationTestLayer({}),
    terminalSessionTestLayer(false),
    credentialStoreUnavailableLayer,
    gitRunnerTestLayer(() => Effect.void),
    secretInputTestLayer("stdin-token"),
    requestBodyInputTestLayer(standardInput),
    boxClientLayer.pipe(Layer.provide(transport)),
    outputTestLayer((document, mode) =>
      Effect.sync(() => {
        written.push({ document, mode });
      }),
    ),
  );
  return { written, layer };
};

describe("openagents box CLI commands", () => {
  it("resolves the conversation through the box-scoped route", async () => {
    // A `box:control` token cannot reach `/user`, which sits behind
    // `forge:write`. Resolving through `/conversation` is what lets a
    // box-scoped credential run a box command at all.
    const seen: string[] = [];
    const { layer, written } = harness((req) => {
      seen.push(req.path);
      if (req.path === "/api/v1/conversation") {
        return Effect.succeed({ status: 200, body: { conversation_id: "conv-box" } });
      }
      if (req.path === "/api/v1/user") {
        return Effect.succeed({ status: 401, body: { error: { code: "invalid_api_token" } } });
      }
      if (req.path === "/api/v1/conversations/conv-box/boxes") {
        return Effect.succeed({ status: 200, body: { boxes: [] } });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(runCliWith(["box", "list"]).pipe(Effect.provide(layer)));

    expect(seen).toContain("/api/v1/conversation");
    expect(seen).not.toContain("/api/v1/user");
    expect(written.length).toBe(1);
  });

  it("falls back to the user route on a deployment without the conversation route", async () => {
    // The CLI ships on its own schedule, so a published client still has to
    // work against a server that predates the route.
    const seen: string[] = [];
    const { layer, written: _written } = harness((req) => {
      seen.push(req.path);
      if (req.path === "/api/v1/conversation") {
        return Effect.succeed({ status: 404, body: {} });
      }
      if (req.path === "/api/v1/user") {
        return Effect.succeed({ status: 200, body: { conversation_id: "conv-old" } });
      }
      if (req.path === "/api/v1/conversations/conv-old/boxes") {
        return Effect.succeed({ status: 200, body: { boxes: [] } });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(runCliWith(["box", "list"]).pipe(Effect.provide(layer)));

    expect(seen).toContain("/api/v1/conversation");
    expect(seen).toContain("/api/v1/conversations/conv-old/boxes");
  });

  it("lists boxes for the conversation", async () => {
    const { layer, written } = harness((req) => {
      if (req.path === "/api/v1/user") {
        return Effect.succeed({
          status: 200,
          body: { conversation_id: "conv-123" },
        });
      }
      if (req.path === "/api/v1/conversations/conv-123/boxes") {
        return Effect.succeed({
          status: 200,
          body: {
            boxes: [
              {
                box_id: "bx_test123",
                label: "worker-1",
                state: "ready",
                setup_status: "done",
                created_at: "2026-08-25T12:00:00Z",
                stopped_at: null,
              },
            ],
          },
        });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith(["box", "list"]).pipe(Effect.provide(layer)),
    );

    expect(written.length).toBe(1);
    const human = written[0]!.document.human;
    expect(human.some((line) => line.includes("bx_test123"))).toBe(true);
    expect(human.some((line) => line.includes("worker-1"))).toBe(true);
  });

  it("provisions a new box", async () => {
    const { layer, written } = harness((req) => {
      if (req.path === "/api/v1/user") {
        return Effect.succeed({
          status: 200,
          body: { conversation_id: "conv-123" },
        });
      }
      if (req.path === "/api/v1/conversations/conv-123/boxes" && req.method === "POST") {
        return Effect.succeed({
          status: 201,
          body: {
            box: {
              box_id: "bx_created99",
              label: "new-box",
              state: "ready",
              setup_status: "done",
              created_at: "2026-08-25T12:05:00Z",
            },
          },
        });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith(["box", "create", "--label", "new-box"]).pipe(Effect.provide(layer)),
    );

    expect(written.length).toBe(1);
    const human = written[0]!.document.human;
    expect(human.some((line) => line.includes("bx_created99"))).toBe(true);
  });

  it("executes a command on a box", async () => {
    const { layer, written } = harness((req) => {
      if (req.path === "/api/v1/user") {
        return Effect.succeed({
          status: 200,
          body: { conversation_id: "conv-123" },
        });
      }
      if (req.path === "/api/v1/conversations/conv-123/boxes/bx_test123/commands") {
        return Effect.succeed({
          status: 200,
          body: {
            result: {
              box_id: "bx_test123",
              exit_code: 0,
              stdout: "hello box\n",
              stderr: "",
              timed_out: false,
              stdout_truncated: false,
              stderr_truncated: false,
            },
          },
        });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith(["box", "exec", "bx_test123", "echo", "hello box"]).pipe(Effect.provide(layer)),
    );

    expect(written.length).toBe(1);
    const human = written[0]!.document.human;
    expect(human.some((line) => line.includes("hello box"))).toBe(true);
  });

  it("stops a box", async () => {
    const { layer, written } = harness((req) => {
      if (req.path === "/api/v1/user") {
        return Effect.succeed({
          status: 200,
          body: { conversation_id: "conv-123" },
        });
      }
      if (req.path === "/api/v1/conversations/conv-123/boxes/bx_test123/stop") {
        return Effect.succeed({
          status: 200,
          body: {
            box: {
              box_id: "bx_test123",
              state: "archiving",
              setup_status: "done",
              created_at: "2026-08-25T12:00:00Z",
              stopped_at: "2026-08-25T12:10:00Z",
            },
          },
        });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith(["box", "stop", "bx_test123"]).pipe(Effect.provide(layer)),
    );

    expect(written.length).toBe(1);
    const human = written[0]!.document.human;
    expect(human.some((line) => line.includes("Stopped Box bx_test123"))).toBe(true);
  });

  // The server nests the read one level down, under `output.output`. Reading
  // the envelope's `output` as text made `box runs output` print an empty
  // line against production while every record was durable server-side.
  it("renders the nested run output envelope the server sends", async () => {
    const { layer, written } = harness((req) => {
      if (req.path === "/api/v1/user") {
        return Effect.succeed({
          status: 200,
          body: { conversation_id: "conv-123" },
        });
      }
      if (req.path === "/api/v1/conversations/conv-123/boxes/bx_test123/runs/run-9/output") {
        return Effect.succeed({
          status: 200,
          body: {
            run_id: "run-9",
            output: {
              output: "cloned openagents.com at 40dbd832\n",
              offset: 0,
              next_offset: 34,
              output_base_offset: 0,
              truncated: false,
            },
          },
        });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith(["box", "runs", "output", "bx_test123", "run-9"]).pipe(Effect.provide(layer)),
    );

    expect(written.length).toBe(1);
    const human = written[0]!.document.human;
    expect(human.some((line) => line.includes("cloned openagents.com at 40dbd832"))).toBe(true);
    expect(human.some((line) => line.includes("EARLIER OUTPUT DROPPED"))).toBe(false);
    expect(written[0]!.document.value).toMatchObject({
      run_id: "run-9",
      next_offset: 34,
      truncated: false,
    });
  });

  it("reports output the box dropped before the requested offset", async () => {
    const { layer, written } = harness((req) => {
      if (req.path === "/api/v1/user") {
        return Effect.succeed({
          status: 200,
          body: { conversation_id: "conv-123" },
        });
      }
      if (
        req.path === "/api/v1/conversations/conv-123/boxes/bx_test123/runs/run-9/output?offset=10"
      ) {
        return Effect.succeed({
          status: 200,
          body: {
            run_id: "run-9",
            output: {
              output: "resumed\n",
              offset: 10,
              next_offset: 4108,
              output_base_offset: 4100,
              truncated: true,
            },
          },
        });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith([
        "box",
        "runs",
        "output",
        "bx_test123",
        "run-9",
        "--offset",
        "10",
      ]).pipe(Effect.provide(layer)),
    );

    const human = written[0]!.document.human;
    expect(human.some((line) => line.includes("EARLIER OUTPUT DROPPED"))).toBe(true);
    expect(human.some((line) => line.includes("resumed"))).toBe(true);
  });

  // Kept so a deployment still answering the older flat shape keeps working
  // while the fix rolls out.
  it("still renders a flat run output body", async () => {
    const { layer, written } = harness((req) => {
      if (req.path === "/api/v1/user") {
        return Effect.succeed({
          status: 200,
          body: { conversation_id: "conv-123" },
        });
      }
      if (req.path === "/api/v1/conversations/conv-123/boxes/bx_test123/runs/run-9/output") {
        return Effect.succeed({
          status: 200,
          body: { run_id: "run-9", output: "flat body output\n" },
        });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith(["box", "runs", "output", "bx_test123", "run-9"]).pipe(Effect.provide(layer)),
    );

    const human = written[0]!.document.human;
    expect(human.some((line) => line.includes("flat body output"))).toBe(true);
  });

  // Production's `GET /api/v1/user` answers the forge identity and no
  // conversation, so the refusal has to point at the flag that works rather
  // than at an account state the caller cannot change.
  it("names --conversation when the deployment reports no conversation", async () => {
    const { layer } = harness((req) => {
      if (req.path === "/api/v1/user") {
        return Effect.succeed({
          status: 200,
          body: { id: 1, login: "operator", namespaces: [] },
        });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    // `Effect.flip` turns the refusal into the success value, so a run that
    // wrongly succeeded rejects the promise and fails the test.
    const failure = await Effect.runPromise(
      runCliWith(["box", "list"]).pipe(Effect.provide(layer), Effect.flip),
    );

    expect(JSON.stringify(failure)).toContain("--conversation");
  });

  it("skips the conversation probe when --conversation is given", async () => {
    const seen: Array<string> = [];
    const { layer, written } = harness((req) => {
      seen.push(req.path);
      if (req.path === "/api/v1/conversations/conv-explicit/boxes") {
        return Effect.succeed({ status: 200, body: { boxes: [] } });
      }
      return Effect.succeed({ status: 404, body: {} });
    });

    await Effect.runPromise(
      runCliWith(["box", "list", "--conversation", "conv-explicit"]).pipe(Effect.provide(layer)),
    );

    expect(seen).toEqual(["/api/v1/conversations/conv-explicit/boxes"]);
    expect(written.length).toBe(1);
  });
});
