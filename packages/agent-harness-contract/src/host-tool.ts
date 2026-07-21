import { Schema as S } from "effect";

/**
 * A host-executed tool made available to the runtime for one turn. The runtime
 * calls it, the harness emits a `tool.call` stream event, the host executes the
 * tool and submits the result back through the prompt-control handle. This is
 * the neutral, adapter-agnostic spec; a bridge adapter registers these on an
 * in-sandbox MCP server, an ACP adapter forwards them over the peer protocol.
 */
export const HarnessHostToolSpec = S.Struct({
  /** Tool name the runtime sees and calls. */
  name: S.NonEmptyString,
  /** Human/model-facing description of what the tool does. */
  description: S.String,
  /**
   * JSON Schema for the tool input. Kept as opaque JSON so adapters can hand it
   * to whatever native tool-registration mechanism their runtime uses (MCP,
   * ACP, SDK) without the contract owning a schema dialect.
   */
  inputJsonSchema: S.Json,
});
export interface HarnessHostToolSpec extends S.Schema.Type<typeof HarnessHostToolSpec> {}

/**
 * Result the host submits for a host-tool call the runtime made. `isError`
 * lets the host report a tool failure the runtime should see, distinct from a
 * transport failure.
 */
export const HarnessHostToolResult = S.Struct({
  toolCallId: S.NonEmptyString,
  /** JSON result payload handed back to the runtime. */
  output: S.Json,
  isError: S.optionalKey(S.Boolean),
});
export interface HarnessHostToolResult extends S.Schema.Type<typeof HarnessHostToolResult> {}

/**
 * Decision the host submits for an adapter-native (built-in) tool approval the
 * runtime is waiting on. `allow-once`/`deny` are per-call; `allow-session`
 * remembers the approval for the rest of the session where the adapter
 * supports it.
 */
export const HARNESS_TOOL_APPROVAL_DECISIONS = ["allow-once", "allow-session", "deny"] as const;
export type HarnessToolApprovalDecision = (typeof HARNESS_TOOL_APPROVAL_DECISIONS)[number];
export const HarnessToolApprovalDecisionSchema = S.Literals(HARNESS_TOOL_APPROVAL_DECISIONS);

// STREAM-07 (#9135): `HarnessHostToolSpec` is the JSON Schema WIRE form only.
// The typed AUTHORING form is an Effect AI `Tool`/`Toolkit`
// (`effect/unstable/ai`); `toolkit-bridge.ts` projects a `Tool` onto this spec
// (`harnessHostToolSpecFromTool`), resolves a host-tool call through the
// Toolkit handler Layer (`resolveHostToolCall`), and composes `needsApproval`
// with the decisions above through the one canonical `RuntimeInteraction`
// approval path (`hostToolApprovalInteractionPayload`,
// `applyHostToolApprovalDecision`).
