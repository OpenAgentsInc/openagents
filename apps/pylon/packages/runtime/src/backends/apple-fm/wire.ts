/**
 * The frozen, versioned Apple FM bridge WIRE contract.
 *
 * These Effect schemas mirror the EXACT JSON the Swift `foundation-bridge`
 * helper (`apps/pylon/swift/foundation-bridge/Sources/foundation-bridge/main.swift`)
 * accepts and emits on the loopback HTTP contract — field names and casing
 * included (`ownedBy` is camelCase; `tool_callback` / `session_token` /
 * `generation_id` / `is_complete` / `underlying_error` are snake_case exactly as
 * the Swift `CodingKeys` produce them). This module is the single source of
 * truth that both the Swift bridge and every TypeScript consumer are proven
 * against, so a wire-shape drift is caught by the conformance tests
 * (`wire-conformance.test.ts`) instead of silently breaking a caller.
 *
 * The internal, camelCased consumer schemas that the runtime client uses live in
 * `contract.ts`; several of those are byte-identical to the wire and are reused
 * here. This module adds the endpoints `contract.ts` did not previously freeze:
 * models, sessions (create + stream request), the SSE snapshot/completed frames,
 * the tool-callback request/result, and the error envelope.
 *
 * Bump `APPLE_FM_BRIDGE_WIRE_VERSION` (and the Swift `bridgeVersion`) together
 * whenever the wire changes.
 */

import { Schema as S } from "effect";
import {
  AppleFmFinishReason,
  AppleFmUnavailableReason,
  AppleFmUsageMeasurement,
} from "./contract.js";

/** Schema id + version for the whole loopback wire contract. */
export const APPLE_FM_BRIDGE_WIRE_VERSION = "openagents.apple_fm.bridge.wire.v0.2" as const;

// --- Models -----------------------------------------------------------------

export const AppleFmWireModelDescriptor = S.Struct({
  id: S.String,
  ownedBy: S.String,
});
export type AppleFmWireModelDescriptor = typeof AppleFmWireModelDescriptor.Type;

export const AppleFmWireModelsResponse = S.Struct({
  data: S.Array(AppleFmWireModelDescriptor),
});
export type AppleFmWireModelsResponse = typeof AppleFmWireModelsResponse.Type;

// --- Sessions ---------------------------------------------------------------

export const AppleFmWireSessionToolDescriptor = S.Struct({
  name: S.optional(S.String),
  description: S.optional(S.String),
});
export type AppleFmWireSessionToolDescriptor = typeof AppleFmWireSessionToolDescriptor.Type;

/** Snake-cased exactly as the Swift `ToolCallbackDescriptor.CodingKeys`. */
export const AppleFmWireToolCallbackDescriptor = S.Struct({
  url: S.optional(S.String),
  session_token: S.optional(S.String),
});
export type AppleFmWireToolCallbackDescriptor = typeof AppleFmWireToolCallbackDescriptor.Type;

export const AppleFmWireSessionCreateRequest = S.Struct({
  instructions: S.optional(S.String),
  tools: S.optional(S.Array(AppleFmWireSessionToolDescriptor)),
  tool_callback: S.optional(AppleFmWireToolCallbackDescriptor),
});
export type AppleFmWireSessionCreateRequest = typeof AppleFmWireSessionCreateRequest.Type;

export const AppleFmWireSessionDescriptor = S.Struct({ id: S.String });
export type AppleFmWireSessionDescriptor = typeof AppleFmWireSessionDescriptor.Type;

export const AppleFmWireSessionCreateResponse = S.Struct({
  session: AppleFmWireSessionDescriptor,
});
export type AppleFmWireSessionCreateResponse = typeof AppleFmWireSessionCreateResponse.Type;

export const AppleFmWireSessionStreamRequest = S.Struct({ prompt: S.String });
export type AppleFmWireSessionStreamRequest = typeof AppleFmWireSessionStreamRequest.Type;

// --- SSE frames (data payload of each `event:` line) -------------------------

export const AppleFmWireStreamSnapshotFrame = S.Struct({
  sequence: S.Number,
  content: S.String,
  output: S.String,
  finishReason: S.optional(AppleFmFinishReason),
});
export type AppleFmWireStreamSnapshotFrame = typeof AppleFmWireStreamSnapshotFrame.Type;

export const AppleFmWireStreamCompletedFrame = S.Struct({
  output: S.String,
  content: S.String,
  model: S.String,
  usage: AppleFmUsageMeasurement,
});
export type AppleFmWireStreamCompletedFrame = typeof AppleFmWireStreamCompletedFrame.Type;

// --- Tool callback (bridge -> controller loopback POST) ----------------------

export const AppleFmWireToolCallbackArguments = S.Struct({
  generation_id: S.String,
  content: S.Record(S.String, S.String),
  is_complete: S.Boolean,
});
export type AppleFmWireToolCallbackArguments = typeof AppleFmWireToolCallbackArguments.Type;

export const AppleFmWireToolCallbackPayload = S.Struct({
  session_token: S.String,
  tool_name: S.String,
  arguments: AppleFmWireToolCallbackArguments,
});
export type AppleFmWireToolCallbackPayload = typeof AppleFmWireToolCallbackPayload.Type;

export const AppleFmWireToolCallbackResult = S.Struct({
  output: S.optional(S.String),
  underlying_error: S.optional(S.String),
});
export type AppleFmWireToolCallbackResult = typeof AppleFmWireToolCallbackResult.Type;

// --- Error envelope ---------------------------------------------------------

export const AppleFmWireErrorResponse = S.Struct({
  error: S.String,
  message: S.String,
  unavailableReason: S.optional(AppleFmUnavailableReason),
});
export type AppleFmWireErrorResponse = typeof AppleFmWireErrorResponse.Type;

// --- Endpoint manifest ------------------------------------------------------

/**
 * Machine-readable index of every wire endpoint the bridge serves, naming the
 * request/response schema for each. Used by the conformance tests and by docs;
 * a new endpoint must be added here so the conformance sweep covers it.
 */
export const APPLE_FM_BRIDGE_WIRE_ENDPOINTS = [
  { method: "GET", path: "/health", response: "AppleFmHealthResponse" },
  { method: "GET", path: "/v1/models", response: "AppleFmWireModelsResponse" },
  {
    method: "POST",
    path: "/v1/chat/completions",
    request: "AppleFmChatCompletionRequest",
    response: "AppleFmChatCompletionResponse",
  },
  {
    method: "POST",
    path: "/v1/sessions",
    request: "AppleFmWireSessionCreateRequest",
    response: "AppleFmWireSessionCreateResponse",
  },
  {
    method: "POST",
    path: "/v1/sessions/{id}/responses/stream",
    request: "AppleFmWireSessionStreamRequest",
    response: "AppleFmWireStreamSnapshotFrame + AppleFmWireStreamCompletedFrame (SSE)",
  },
  {
    method: "POST",
    path: "(controller loopback tool callback)",
    request: "AppleFmWireToolCallbackPayload",
    response: "AppleFmWireToolCallbackResult",
  },
  { method: "*", path: "(error envelope)", response: "AppleFmWireErrorResponse" },
] as const;

/** The frozen wire contract descriptor (version + endpoint index). */
export const APPLE_FM_BRIDGE_WIRE_CONTRACT = {
  version: APPLE_FM_BRIDGE_WIRE_VERSION,
  endpoints: APPLE_FM_BRIDGE_WIRE_ENDPOINTS,
} as const;

// The reused wire schemas (`AppleFmHealthResponse`, `AppleFmChatCompletionRequest`,
// `AppleFmChatCompletionResponse`) are already exported from `contract.js`; import
// them from there to avoid a duplicate `export *` name in the package index.
