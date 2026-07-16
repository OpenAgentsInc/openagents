import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import fc from "fast-check";
import { describe, expect, it } from "vite-plus/test";

import {
  CURSOR_ACP_EXTENSIONS,
  CURSOR_ACP_PROFILE,
  decodeCursorAcpExtensionEnvelope,
  decodeCursorListAvailableModelsResponse,
} from "./extensions/cursor.ts";
import {
  decodeGrokAcpExtensionEnvelope,
  GROK_ACP_EXTENSIONS,
  GROK_ACP_PROFILE,
} from "./extensions/grok.ts";
import { STABLE_METHOD_MANIFEST, UNSTABLE_METHOD_MANIFEST } from "./generated/methods.ts";
import {
  compileAcpDefinitionCodecs,
  decodeAcpDefinition,
  decodeAcpJsonRpcEnvelope,
  decodeAcpMethodPayload,
  getAcpDefinitionNames,
  parseAcpNativeEnvelope,
  serializeAcpNativeEnvelope,
} from "./runtime.ts";
describe("Agent Client Protocol schema-v1.19.0 authority", () => {
  it("keeps stable and unstable method inventories physically separate", () => {
    const stableTypes = readFileSync(
      resolve(import.meta.dirname, "generated", "stable-types.ts"),
      "utf8",
    );
    expect(stableTypes).not.toMatch(/\bproviders\??:/);
    expect(stableTypes).not.toMatch(/\bfork\??:/);
    expect(stableTypes).not.toContain("@agentclientprotocol/sdk");
    expect(STABLE_METHOD_MANIFEST.members).toHaveLength(23);
    expect(
      STABLE_METHOD_MANIFEST.members.some((member) => String(member.method) === "session/fork"),
    ).toBe(false);
    expect(
      UNSTABLE_METHOD_MANIFEST.members.find((member) => member.method === "session/fork"),
    ).toMatchObject({
      stability: "unstable",
      supportState: "profile-gated",
    });
  });

  it("compiles a runtime codec for every stable and unstable definition", () => {
    expect(compileAcpDefinitionCodecs("stable")).toBe(getAcpDefinitionNames("stable").length);
    expect(compileAcpDefinitionCodecs("unstable")).toBe(getAcpDefinitionNames("unstable").length);
  });

  it("decodes the documented Grok initialize response and preserves future fields", () => {
    const payload = {
      protocolVersion: 1,
      agentInfo: { name: "grok", version: "0.1.0" },
      agentCapabilities: {},
      authMethods: [{ id: "cached_token", name: "Cached token" }],
      futurePeerField: { opaque: true },
    };
    const decoded = decodeAcpMethodPayload({
      lane: "stable",
      direction: "client-to-agent",
      method: "initialize",
      phase: "result",
      payload,
    });
    expect(decoded).toMatchObject({
      _tag: "Decoded",
      native: { raw: payload, retention: "private-native" },
    });
  });

  it("decodes Grok and Cursor authentication samples without promoting peer extensions", () => {
    for (const payload of [
      { methodId: "cached_token", _meta: { headless: true } },
      { methodId: "xai.api_key", _meta: { headless: true } },
      { methodId: "cursor_login" },
    ]) {
      expect(
        decodeAcpMethodPayload({
          lane: "stable",
          direction: "client-to-agent",
          method: "authenticate",
          phase: "params",
          payload,
        }),
      ).toMatchObject({ _tag: "Decoded", native: { raw: payload } });
    }

    expect(
      decodeAcpMethodPayload({
        lane: "stable",
        direction: "client-to-agent",
        method: "initialize",
        phase: "result",
        payload: {
          protocolVersion: 1,
          agentInfo: { name: "cursor-agent", version: "1.0.0" },
          agentCapabilities: {},
          authMethods: [{ id: "cursor_login", name: "Log in to Cursor" }],
        },
      }),
    ).toMatchObject({ _tag: "Decoded" });
  });

  it("decodes session creation and update samples used by Grok and Cursor", () => {
    expect(
      decodeAcpMethodPayload({
        lane: "stable",
        direction: "client-to-agent",
        method: "session/new",
        phase: "params",
        payload: { cwd: "/workspace", mcpServers: [] },
      }),
    ).toMatchObject({ _tag: "Decoded" });

    expect(
      decodeAcpMethodPayload({
        lane: "stable",
        direction: "agent-to-client",
        method: "session/update",
        phase: "params",
        payload: {
          sessionId: "s-1",
          update: {
            sessionUpdate: "agent_message_chunk",
            content: { type: "text", text: "hello" },
          },
        },
      }),
    ).toMatchObject({ _tag: "Decoded" });
  });

  it("retains unknown variants privately while refusing canonical typed behavior", () => {
    const raw = {
      sessionId: "s-1",
      update: { sessionUpdate: "future_vendor_update", secret: "must-not-enter-detail" },
    };
    const decoded = decodeAcpMethodPayload({
      lane: "stable",
      direction: "agent-to-client",
      method: "session/update",
      phase: "params",
      payload: raw,
    });
    expect(decoded).toMatchObject({
      _tag: "DecodeFailure",
      reason: "invalid_payload",
      native: { raw },
    });
    raw.update.secret = "mutated-after-decode";
    if (decoded["_tag"] === "DecodeFailure") {
      expect(decoded.detail).not.toContain("must-not-enter-detail");
      expect(parseAcpNativeEnvelope(serializeAcpNativeEnvelope(decoded.native))).toEqual(
        decoded.native,
      );
      expect(decoded.native.raw).toEqual({
        sessionId: "s-1",
        update: { sessionUpdate: "future_vendor_update", secret: "must-not-enter-detail" },
      });
    }
  });

  it("keeps native evidence immutable and independent from decoded values", () => {
    const decoded = decodeAcpDefinition("stable", "AgentCapabilities", { loadSession: true });
    expect(decoded).toMatchObject({ _tag: "Decoded" });
    if (decoded["_tag"] === "Decoded") {
      (decoded.value as { loadSession: boolean }).loadSession = false;
      expect(decoded.native.raw).toEqual({ loadSession: true });
      expect(Object.isFrozen(decoded.native.raw)).toBe(true);
      expect(parseAcpNativeEnvelope(serializeAcpNativeEnvelope(decoded.native))).toEqual(
        decoded.native,
      );
    }
  });

  it("validates complete JSON-RPC request, notification, result, and error envelopes", () => {
    expect(
      decodeAcpJsonRpcEnvelope({
        lane: "stable",
        direction: "client-to-agent",
        message: {
          jsonrpc: "2.0",
          id: 1,
          method: "initialize",
          params: {
            protocolVersion: 1,
            clientCapabilities: {},
            clientInfo: { name: "openagents", version: "0.1.0" },
          },
        },
      }),
    ).toMatchObject({ _tag: "Decoded" });

    expect(
      decodeAcpJsonRpcEnvelope({
        lane: "stable",
        direction: "client-to-agent",
        message: { jsonrpc: "2.0", method: "session/cancel", params: { sessionId: "s-1" } },
      }),
    ).toMatchObject({ _tag: "Decoded" });

    expect(
      decodeAcpJsonRpcEnvelope({
        lane: "stable",
        direction: "client-to-agent",
        expectedMethod: "session/prompt",
        message: { jsonrpc: "2.0", id: 2, result: { stopReason: "end_turn" } },
      }),
    ).toMatchObject({ _tag: "Decoded" });

    expect(
      decodeAcpJsonRpcEnvelope({
        lane: "stable",
        direction: "client-to-agent",
        expectedMethod: "session/prompt",
        message: { jsonrpc: "2.0", id: 2, error: { code: -32_000, message: "auth required" } },
      }),
    ).toMatchObject({ _tag: "Decoded" });
  });

  it("rejects malformed envelopes and unsupported wire versions", () => {
    expect(
      decodeAcpJsonRpcEnvelope({
        lane: "stable",
        direction: "client-to-agent",
        message: { jsonrpc: "2.0", method: "initialize", params: { protocolVersion: 1 } },
      }),
    ).toMatchObject({ _tag: "DecodeFailure", reason: "malformed_envelope" });
    expect(
      decodeAcpJsonRpcEnvelope({
        lane: "stable",
        direction: "client-to-agent",
        message: { jsonrpc: "1.0", id: 1, method: "initialize", params: {} },
      }),
    ).toMatchObject({ _tag: "DecodeFailure", reason: "malformed_envelope" });
    expect(
      decodeAcpJsonRpcEnvelope({
        lane: "stable",
        direction: "client-to-agent",
        message: { jsonrpc: "2.0", id: null, error: { code: -32_700, message: "parse error" } },
      }),
    ).toMatchObject({ _tag: "Decoded" });
    expect(
      decodeAcpJsonRpcEnvelope({
        lane: "stable",
        direction: "client-to-agent",
        message: { jsonrpc: "2.0", id: 1.5, method: "initialize", params: {} },
      }),
    ).toMatchObject({ _tag: "DecodeFailure", reason: "malformed_envelope" });
    expect(
      decodeAcpMethodPayload({
        lane: "stable",
        direction: "client-to-agent",
        method: "initialize",
        phase: "params",
        payload: {
          protocolVersion: 2,
          clientCapabilities: {},
          clientInfo: { name: "openagents", version: "0.1.0" },
        },
      }),
    ).toMatchObject({ _tag: "DecodeFailure", reason: "unsupported_protocol_version" });
    expect(
      decodeAcpDefinition("stable", "InitializeRequest", { protocolVersion: 2 }),
    ).toMatchObject({ _tag: "DecodeFailure", reason: "unsupported_protocol_version" });
    expect(
      decodeAcpJsonRpcEnvelope({
        lane: "stable",
        direction: "client-to-agent",
        expectedMethod: "initialize",
        message: { jsonrpc: "2.0", id: 1, result: {}, error: { code: 1, message: "both" } },
      }),
    ).toMatchObject({ _tag: "DecodeFailure", reason: "malformed_envelope" });
  });

  it("returns secret-safe structured failures for malformed payloads", () => {
    const decoded = decodeAcpDefinition("stable", "AuthenticateRequest", {
      methodId: 42,
      apiKey: "xai-secret",
    });
    expect(decoded).toMatchObject({ _tag: "DecodeFailure", reason: "invalid_payload" });
    if (decoded["_tag"] === "DecodeFailure") {
      expect(decoded.detail).not.toContain("xai-secret");
      expect(decoded.native.raw).toEqual({ methodId: 42, apiKey: "xai-secret" });
    }
  });

  it("keeps vendor extension direction outside the stable root manifest", () => {
    expect(GROK_ACP_EXTENSIONS.askUserQuestion).toBe("x.ai/ask_user_question");
    expect(CURSOR_ACP_EXTENSIONS).toEqual({
      inboundRequests: ["cursor/ask_question", "cursor/create_plan"],
      inboundNotifications: ["cursor/update_todos"],
      outboundRequests: ["cursor/list_available_models"],
    });
    expect(
      STABLE_METHOD_MANIFEST.members.some(
        (member) => member.method.startsWith("cursor/") || member.method.startsWith("x.ai/"),
      ),
    ).toBe(false);
    expect(GROK_ACP_PROFILE).toMatchObject({
      schemaRelease: "schema-v1.19.0",
      wireVersion: 1,
      profileVersion: 1,
      gate: "explicit-peer-profile",
    });
    expect(CURSOR_ACP_PROFILE.peer).toBe("cursor-agent");
    const grokMessage = {
      jsonrpc: "2.0",
      id: 7,
      method: "x.ai/ask_user_question",
      params: { prompt: "Continue?" },
    };
    expect(decodeGrokAcpExtensionEnvelope(grokMessage, undefined)).toMatchObject({
      _tag: "VendorExtensionFailure",
      reason: "peer_profile_required",
    });
    expect(decodeGrokAcpExtensionEnvelope(grokMessage, "grok-cli")).toMatchObject({
      _tag: "DecodedVendorExtension",
    });
    expect(
      decodeCursorAcpExtensionEnvelope({
        enabledPeer: "cursor-agent",
        direction: "agent-to-client",
        message: { jsonrpc: "2.0", method: "cursor/update_todos", params: { todos: [] } },
      }),
    ).toMatchObject({ _tag: "DecodedVendorExtension" });
  });

  it("validates Cursor model discovery recursively and rejects duplicates", () => {
    expect(
      decodeCursorListAvailableModelsResponse({
        models: [
          {
            value: "cursor-auto",
            name: "Auto",
            configOptions: [
              { id: "thinking", name: "Thinking", type: "boolean", currentValue: true },
            ],
          },
        ],
      }),
    ).toMatchObject({ models: [{ value: "cursor-auto", name: "Auto" }] });
    expect(
      decodeCursorListAvailableModelsResponse({
        models: [
          { value: "duplicate", name: "One" },
          { value: "duplicate", name: "Two" },
        ],
      }),
    ).toBeUndefined();
    expect(
      decodeCursorListAvailableModelsResponse({
        models: [{ value: "bad", name: "Bad", configOptions: [{ id: "missing-shape" }] }],
      }),
    ).toBeUndefined();
  });

  it("accepts and rejects representative values across stable codec families", () => {
    const families: ReadonlyArray<readonly [string, unknown, unknown]> = [
      ["ContentBlock", { type: "text", text: "hello" }, { type: "future", text: "hello" }],
      ["StopReason", "end_turn", "future_reason"],
      ["AuthMethod", { id: "cached_token", name: "Cached token" }, { id: 42 }],
      ["SessionMode", { id: "code", name: "Code" }, { id: "code" }],
      [
        "SessionConfigOption",
        { id: "safe", name: "Safe mode", type: "boolean", currentValue: true },
        { id: "safe", name: "Safe mode", type: "boolean" },
      ],
      ["AgentCapabilities", { loadSession: true }, { loadSession: "yes" }],
      ["Error", { code: -32_000, message: "failed" }, { code: "failed", message: "failed" }],
      [
        "WriteTextFileRequest",
        { sessionId: "s-1", path: "/tmp/a", content: "a" },
        { sessionId: "s-1", path: "/tmp/a" },
      ],
      [
        "CreateTerminalRequest",
        { sessionId: "s-1", command: "printf", args: ["ok"] },
        { sessionId: "s-1", command: 42 },
      ],
    ];
    for (const [definition, valid, invalid] of families) {
      expect(decodeAcpDefinition("stable", definition, valid)).toMatchObject({ _tag: "Decoded" });
      expect(decodeAcpDefinition("stable", definition, invalid)).toMatchObject({
        _tag: "DecodeFailure",
        reason: "invalid_payload",
      });
    }
  });

  it("property-checks arbitrary JSON evidence round-trips and version rejection", () => {
    fc.assert(
      fc.property(fc.json(), (encodedJson) => {
        const value: unknown = JSON.parse(encodedJson);
        const result = decodeAcpDefinition("stable", "ContentBlock", value);
        const roundTrip = parseAcpNativeEnvelope(serializeAcpNativeEnvelope(result.native));
        expect(roundTrip).toEqual(result.native);
      }),
      { numRuns: 100 },
    );
    fc.assert(
      fc.property(
        fc.integer().filter((version) => version !== 1),
        (protocolVersion) => {
          expect(
            decodeAcpDefinition("stable", "InitializeRequest", { protocolVersion }),
          ).toMatchObject({
            _tag: "DecodeFailure",
            reason: "unsupported_protocol_version",
          });
        },
      ),
      { numRuns: 100 },
    );
  });

  it("records the exact official SDK decision and source digest", () => {
    const source = JSON.parse(
      readFileSync(
        resolve(import.meta.dirname, "..", "upstream", "schema-v1.19.0", "SOURCE.json"),
        "utf8",
      ),
    ) as {
      sdk: { version: string; schemaLane: string; schemaSha256: string };
    };
    expect(source.sdk).toMatchObject({
      version: "1.2.1",
      schemaLane: "unstable",
      schemaSha256: "8bdfd8347ce8bd2c8620b71bfd5460625f91c7db47a51268bb42b67014ea5b1f",
    });
  });
});
