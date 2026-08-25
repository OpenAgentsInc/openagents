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
});
