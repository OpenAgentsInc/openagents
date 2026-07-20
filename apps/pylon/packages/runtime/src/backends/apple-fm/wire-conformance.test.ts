import { Runtime } from "@openagentsinc/runtime-platform";
import { describe, expect, test } from "vite-plus/test";
import { Effect, Schema as S } from "effect";
import {
  APPLE_FM_BRIDGE_WIRE_CONTRACT,
  APPLE_FM_BRIDGE_WIRE_VERSION,
  AppleFmWireErrorResponse,
  AppleFmWireModelsResponse,
  AppleFmWireSessionCreateResponse,
  AppleFmWireStreamCompletedFrame,
  AppleFmWireStreamSnapshotFrame,
  AppleFmWireToolCallbackPayload,
  AppleFmWireToolCallbackResult,
} from "./wire.js";
import { AppleFmChatCompletionResponse, AppleFmHealthResponse } from "./contract.js";
import { discoverAppleFmBridgeHelper, launchAppleFmBridge } from "./bridge-process.js";

// Exact JSON bytes captured from the live Swift `foundation-bridge` (v0.1.1).
// These fixtures are the drift tripwire: if the Swift bridge changes its wire,
// updating these to match must go through the versioned wire schema.
const WIRE_FIXTURES = {
  health: { ready: true, model: "apple-foundation-model", modelId: "apple-foundation-model", message: "Apple Foundation Models is available.", platform: "macOS", version: "0.1.1" },
  models: { data: [{ id: "apple-foundation-model", ownedBy: "apple" }] },
  chatCompletion: {
    id: "apple-fm-abc",
    model: "apple-foundation-model",
    choices: [{ finishReason: "stop", index: 0, message: { role: "assistant", content: "hello" } }],
    usage: { completionTokens: 3, promptTokens: 9, totalTokens: 12, truth: "estimated" },
  },
  sessionCreate: { session: { id: "apple_fm_session_1e6fd7bc-98e9-4bff-ab6e-e2439811065d" } },
  snapshotFrame: { content: "Hi", finishReason: "stop", output: "Hi", sequence: 0 },
  completedFrame: { content: "Hi", model: "apple-foundation-model", output: "Hi", usage: { completionTokens: 8, promptTokens: 3, totalTokens: 11, truth: "estimated" } },
  toolCallbackPayload: { session_token: "t", tool_name: "read_file", arguments: { generation_id: "g", content: { path: "README.md" }, is_complete: true } },
  toolCallbackResult: { output: "file contents" },
  errorEnvelope: { error: "not_found", message: "No route for GET /nope" },
} as const;

describe("Apple FM bridge wire contract (v0.2)", () => {
  test("exposes a stable version and an endpoint manifest", () => {
    expect(APPLE_FM_BRIDGE_WIRE_VERSION).toBe("openagents.apple_fm.bridge.wire.v0.2");
    expect(APPLE_FM_BRIDGE_WIRE_CONTRACT.endpoints.map((e) => e.path)).toContain("/health");
    expect(APPLE_FM_BRIDGE_WIRE_CONTRACT.endpoints.map((e) => e.path)).toContain("/v1/sessions");
  });

  test("decodes every captured wire fixture through the frozen schema", () => {
    expect(S.decodeUnknownSync(AppleFmHealthResponse)(WIRE_FIXTURES.health).ready).toBe(true);
    expect(S.decodeUnknownSync(AppleFmWireModelsResponse)(WIRE_FIXTURES.models).data[0].ownedBy).toBe("apple");
    expect(S.decodeUnknownSync(AppleFmChatCompletionResponse)(WIRE_FIXTURES.chatCompletion).choices.length).toBe(1);
    expect(S.decodeUnknownSync(AppleFmWireSessionCreateResponse)(WIRE_FIXTURES.sessionCreate).session.id).toContain("apple_fm_session_");
    expect(S.decodeUnknownSync(AppleFmWireStreamSnapshotFrame)(WIRE_FIXTURES.snapshotFrame).sequence).toBe(0);
    expect(S.decodeUnknownSync(AppleFmWireStreamCompletedFrame)(WIRE_FIXTURES.completedFrame).usage.truth).toBe("estimated");
    expect(S.decodeUnknownSync(AppleFmWireToolCallbackPayload)(WIRE_FIXTURES.toolCallbackPayload).arguments.is_complete).toBe(true);
    expect(S.decodeUnknownSync(AppleFmWireToolCallbackResult)(WIRE_FIXTURES.toolCallbackResult).output).toBe("file contents");
    expect(S.decodeUnknownSync(AppleFmWireErrorResponse)(WIRE_FIXTURES.errorEnvelope).error).toBe("not_found");
  });

  test("rejects a wire-shape drift (deliberate mismatch fails decode)", () => {
    // snake -> camel drift on the tool callback must be caught.
    expect(() => S.decodeUnknownSync(AppleFmWireToolCallbackPayload)({ sessionToken: "t", tool_name: "read_file", arguments: { generation_id: "g", content: {}, is_complete: true } })).toThrow();
    // wrong type on a required field.
    expect(() => S.decodeUnknownSync(AppleFmWireModelsResponse)({ data: "not-an-array" })).toThrow();
    // missing required snapshot field.
    expect(() => S.decodeUnknownSync(AppleFmWireStreamSnapshotFrame)({ content: "x", output: "x" })).toThrow();
  });

  test("decodes responses served by a CI-safe fake bridge over HTTP", async () => {
    const server = Runtime.serve({
      port: 0,
      fetch: async (request) => {
        const url = new URL(request.url);
        if (url.pathname === "/health") return Response.json(WIRE_FIXTURES.health);
        if (url.pathname === "/v1/models") return Response.json(WIRE_FIXTURES.models);
        if (url.pathname === "/v1/sessions") return Response.json(WIRE_FIXTURES.sessionCreate);
        return Response.json(WIRE_FIXTURES.errorEnvelope, { status: 404 });
      },
    });
    await server.ready;
    try {
      const baseUrl = `http://127.0.0.1:${server.port}`;
      const health = await (await fetch(new URL("/health", baseUrl))).json();
      expect(S.decodeUnknownSync(AppleFmHealthResponse)(health).ready).toBe(true);
      const models = await (await fetch(new URL("/v1/models", baseUrl))).json();
      expect(S.decodeUnknownSync(AppleFmWireModelsResponse)(models).data[0].id).toBe("apple-foundation-model");
      const session = await (await fetch(new URL("/v1/sessions", baseUrl), { method: "POST" })).json();
      expect(S.decodeUnknownSync(AppleFmWireSessionCreateResponse)(session).session.id).toContain("apple_fm_session_");
    } finally {
      server.stop(true);
    }
  });
});

// --- Real-bridge conformance (admitted Apple Silicon Mac, opt-in) ------------
// Off in the default sweep: it launches a real Swift bridge child process, which
// is environment-sensitive (outbound loopback sockets) and needs a device.
// Enable explicitly with OPENAGENTS_APPLE_FM_REAL_BRIDGE=1 on an admitted Mac.
// The standalone script `scripts/apple-fm-wire-conformance.mjs.ts` is the
// canonical admitted-Mac proof and is not subject to the test-worker sandbox.
const realBridgeAvailable =
  process.env.OPENAGENTS_APPLE_FM_REAL_BRIDGE === "1" &&
  process.platform === "darwin" &&
  process.arch === "arm64" &&
  discoverAppleFmBridgeHelper({}) !== null;

describe("Apple FM bridge wire contract — real bridge", () => {
  test.runIf(realBridgeAvailable)("validates live /health, /v1/models, chat, session + SSE frames", async () => {
    const handle = await Effect.runPromise(launchAppleFmBridge({ adoptIfHealthy: false }));
    try {
      const base = handle.baseUrl;
      const health = await (await fetch(`${base}/health`)).json();
      expect(S.decodeUnknownSync(AppleFmHealthResponse)(health).ready).toBe(true);

      const models = await (await fetch(`${base}/v1/models`)).json();
      expect(S.decodeUnknownSync(AppleFmWireModelsResponse)(models).data[0].id).toBe("apple-foundation-model");

      const chat = await (
        await fetch(`${base}/v1/chat/completions`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ model: "apple-foundation-model", messages: [{ role: "user", content: "Reply with exactly: ok" }] }),
        })
      ).json();
      const chatDecoded = S.decodeUnknownSync(AppleFmChatCompletionResponse)(chat);
      expect(chatDecoded.choices[0].message.role).toBe("assistant");
      expect(chatDecoded.usage?.truth).toBe("estimated");

      const session = S.decodeUnknownSync(AppleFmWireSessionCreateResponse)(
        await (await fetch(`${base}/v1/sessions`, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ prompt: "hi" }) })).json(),
      );
      const sse = await (
        await fetch(`${base}/v1/sessions/${session.session.id}/responses/stream`, {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ prompt: "say hi" }),
        })
      ).text();
      const frames = sse
        .split("\n")
        .filter((line) => line.startsWith("data:"))
        .map((line) => JSON.parse(line.slice(5).trim()));
      expect(() => S.decodeUnknownSync(AppleFmWireStreamSnapshotFrame)(frames[0])).not.toThrow();
      expect(() => S.decodeUnknownSync(AppleFmWireStreamCompletedFrame)(frames[frames.length - 1])).not.toThrow();
    } finally {
      handle.stop();
    }
  });
});
