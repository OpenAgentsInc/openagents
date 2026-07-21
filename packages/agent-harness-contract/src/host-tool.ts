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

// ---------------------------------------------------------------------------
// Registered host tools (RLM-03 #9139) — wire form only. The Effect AI Tool
// authoring form and the HistoryRecall handler Layer live in
// `@openagentsinc/history-corpus` so this package stays free of corpus deps.
// STREAM-07 projects that Tool onto {@link historyRecallHostToolSpec}.
// ---------------------------------------------------------------------------

/** Wire name for the history-recall host tool (every lane + Stack B kernel). */
export const HISTORY_RECALL_TOOL_NAME = "history_recall" as const;

/**
 * Model-facing description. Recall is an explicit tool call with hard caps;
 * the answer is an untrusted cited candidate, never authority.
 */
export const HISTORY_RECALL_TOOL_DESCRIPTION =
  "Ask a structural or lexical question over owner-local conversation history " +
  "instead of relying on the bounded context window. Returns cited cursor " +
  "spans, a required honesty record (what was scanned / which caps hit), and " +
  "zero model calls for the deterministic tier. The answer is an untrusted " +
  "cited candidate — never a route decision, verification verdict, or public claim. " +
  "Scope and caps are required so a model cannot request an unbounded traversal.";

/**
 * Constrained JSON Schema for `history_recall` input. Scope is thread / run /
 * thread_set; question kinds mirror HistoryRecall Tier D; caps truncate.
 */
export const historyRecallHostToolInputJsonSchema = {
  type: "object",
  additionalProperties: false,
  required: ["scope", "question"],
  properties: {
    scope: {
      oneOf: [
        {
          type: "object",
          additionalProperties: false,
          required: ["_tag", "threadId"],
          properties: {
            _tag: { const: "Thread" },
            threadId: { type: "string", minLength: 1 },
          },
        },
        {
          type: "object",
          additionalProperties: false,
          required: ["_tag", "runRef", "threadIds"],
          properties: {
            _tag: { const: "Run" },
            runRef: { type: "string", minLength: 1 },
            threadIds: {
              type: "array",
              items: { type: "string", minLength: 1 },
              maxItems: 64,
            },
          },
        },
        {
          type: "object",
          additionalProperties: false,
          required: ["_tag", "threadIds"],
          properties: {
            _tag: { const: "ThreadSet" },
            threadIds: {
              type: "array",
              items: { type: "string", minLength: 1 },
              maxItems: 64,
            },
          },
        },
      ],
    },
    question: {
      oneOf: [
        {
          type: "object",
          additionalProperties: false,
          required: ["_tag", "pattern"],
          properties: {
            _tag: { const: "Grep" },
            pattern: { type: "string", minLength: 1, maxLength: 500 },
            caseSensitive: { type: "boolean" },
          },
        },
        {
          type: "object",
          additionalProperties: false,
          required: ["_tag", "fromSequence", "toSequence"],
          properties: {
            _tag: { const: "CursorSlice" },
            turnId: { type: "string" },
            fromSequence: { type: "number" },
            toSequence: { type: "number" },
          },
        },
        {
          type: "object",
          additionalProperties: false,
          required: ["_tag", "fromObservedAt", "toObservedAt"],
          properties: {
            _tag: { const: "TimeSlice" },
            fromObservedAt: { type: "string" },
            toObservedAt: { type: "string" },
          },
        },
        {
          type: "object",
          additionalProperties: false,
          required: ["_tag", "limit"],
          properties: {
            _tag: { const: "KeyTurns" },
            limit: { type: "number", minimum: 1, maximum: 200 },
          },
        },
        {
          type: "object",
          additionalProperties: false,
          required: ["_tag", "turnId"],
          properties: {
            _tag: { const: "TurnSummary" },
            turnId: { type: "string", minLength: 1 },
          },
        },
      ],
    },
    caps: {
      type: "object",
      additionalProperties: false,
      properties: {
        maxSpans: { type: "number", minimum: 1, maximum: 200 },
        maxEntriesScanned: { type: "number", minimum: 1, maximum: 100_000 },
        maxCharsPerSpan: { type: "number", minimum: 1, maximum: 4_000 },
      },
    },
  },
} as const;

/** Canonical wire registration for the `history_recall` host tool. */
export const historyRecallHostToolSpec: HarnessHostToolSpec = {
  name: HISTORY_RECALL_TOOL_NAME,
  description: HISTORY_RECALL_TOOL_DESCRIPTION,
  inputJsonSchema: historyRecallHostToolInputJsonSchema,
};

/**
 * The registered harness host-tool catalog. Today only `history_recall`
 * (RLM-03). Hosts hand this list (or a subset) to `promptTurn({ tools })`.
 * Adding a tool here is the wire-form registration; authoring/handlers stay
 * beside their owning package.
 */
export const REGISTERED_HARNESS_HOST_TOOLS: ReadonlyArray<HarnessHostToolSpec> = [
  historyRecallHostToolSpec,
];

/**
 * Stack B / turn-policy capability ref for `history_recall`. Hosts that expose
 * host tools on a turn policy or capability surface use this exact ref so
 * policy and the wire name stay aligned.
 */
export const HISTORY_RECALL_TURN_POLICY_CAPABILITY = "host_tool.history_recall" as const;
