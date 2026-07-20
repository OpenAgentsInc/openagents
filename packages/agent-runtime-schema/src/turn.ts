import { Schema as S } from "effect";

/**
 * AFS-00 frozen turn contract.
 *
 * This module freezes the shared, provider-neutral turn vocabulary for the
 * Apple FM router to full-agent-system program. It owns the turn intent, task
 * class, lifecycle and terminal states, refusal reasons, usage truth, the
 * shared reference primitives, and the frozen size and retained-state bounds.
 *
 * Compatibility rules for every AFS-00 schema literal in this package:
 *
 * - A schema literal names one immutable contract version, for example
 *   `openagents.agent_turn.v1`.
 * - A change is compatible inside one version only when it adds an optional
 *   field or widens an unconstrained value. A decoder for the same version must
 *   still accept every earlier payload of that version.
 * - Adding a union case, removing a field, making an optional field required,
 *   tightening a bound, or renaming a value is a breaking change. It needs a new
 *   version literal and a new module export. It must not mutate the frozen
 *   version.
 * - A persisted or wire identifier keeps its exact spelling across a migration.
 *   A migration adds a new decoder path; it does not reinterpret an old one.
 */
export const TURN_SCHEMA_LITERAL = "openagents.agent_turn.v1" as const;
export const TURN_INTENT_SCHEMA_LITERAL = "openagents.agent_turn_intent.v1" as const;

/**
 * Frozen turn bounds. These values come from the current Apple FM IPC contract
 * and the local turn journal. They are the maximum input, context, output,
 * event, and retained-state limits for version one.
 */
/** Maximum authoritative turn input characters (Apple FM IPC start-turn bound). */
export const MAX_TURN_INPUT_CHARS = 4000 as const;
/** Maximum renderer-prepared input characters before submission (safety margin). */
export const MAX_RENDERER_PREPARED_INPUT_CHARS = 3900 as const;
/** Maximum single-turn assistant output characters (Apple FM result text bound). */
export const MAX_TURN_OUTPUT_CHARS = 8192 as const;
/**
 * Maximum characters in a bounded, public-safe turn failure reason. A failure
 * reason is a short control-plane label (for example `session_failed: delegate
 * lane stopped`), never a raw provider error, command output, path, or token.
 */
export const MAX_TURN_FAILURE_REASON_CHARS = 240 as const;
/** Maximum characters in a bounded context slice presented to a provider. */
export const MAX_TURN_CONTEXT_CHARS = 64_000 as const;
/** Maximum characters in one persisted turn event text field. */
export const MAX_TURN_EVENT_TEXT_CHARS = 32_000 as const;
/** Maximum retained turn records in a local journal window. */
export const MAX_RETAINED_TURN_RECORDS = 128 as const;
/** Maximum assistant segments retained for one turn. */
export const MAX_TURN_ASSISTANT_SEGMENTS = 256 as const;
/** Maximum history messages carried across a provider switch. */
export const MAX_TURN_CONTEXT_HISTORY_MESSAGES = 32 as const;
/** Maximum blocker references recorded on a refusal or failure. */
export const MAX_TURN_BLOCKER_REFS = 8 as const;
/** Maximum candidates in an owner-bound ordered route set. */
export const MAX_OWNER_BOUND_CANDIDATES = 5 as const;

/** Shared strict reference base used by every branded turn reference. */
export const brandedTurnRef = <const Brand extends string>(brand: Brand) =>
  S.String.check(
    S.isMinLength(1),
    S.isMaxLength(256),
    S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
  ).pipe(S.brand(brand));

/** ISO-8601 UTC timestamp used across the turn contract. */
export const TurnTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
);
export type TurnTimestamp = typeof TurnTimestamp.Type;

export const TurnThreadRef = brandedTurnRef("TurnThreadRef");
export type TurnThreadRef = typeof TurnThreadRef.Type;

export const TurnRequestRef = brandedTurnRef("TurnRequestRef");
export type TurnRequestRef = typeof TurnRequestRef.Type;

export const ProviderTurnRef = brandedTurnRef("ProviderTurnRef");
export type ProviderTurnRef = typeof ProviderTurnRef.Type;

export const RunRef = brandedTurnRef("RunRef");
export type RunRef = typeof RunRef.Type;

export const DebugRef = brandedTurnRef("DebugRef");
export type DebugRef = typeof DebugRef.Type;

export const SourceControlRef = brandedTurnRef("SourceControlRef");
export type SourceControlRef = typeof SourceControlRef.Type;

/**
 * Generation identity for a turn. A known generation carries a monotonic
 * counter. An unknown generation records why the counter was not observed. An
 * adapter must not translate one state into the other.
 */
export const TurnGeneration = S.Union([
  S.Struct({
    state: S.Literal("known"),
    value: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)),
  }),
  S.Struct({
    state: S.Literal("unknown"),
    reason: S.Literals(["not_observed", "provider_unsupported", "not_applicable"]),
  }),
]);
export type TurnGeneration = typeof TurnGeneration.Type;

/**
 * An Editor anchor names a document position for a completion, next-edit, or
 * proposal intent. The reference is document-scoped, never an absolute path.
 */
export const EditorAnchor = S.Struct({
  documentRef: brandedTurnRef("EditorDocumentRef"),
  generation: TurnGeneration,
  line: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
  column: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(1)),
});
export type EditorAnchor = typeof EditorAnchor.Type;

/**
 * The task class of one turn. It is derived deterministically from the turn
 * intent. Policy uses it to filter the owner-bound candidate set.
 */
export const TurnTaskClass = S.Literals([
  "local_answer",
  "completion",
  "next_edit",
  "propose_edit",
  "explain_failure",
  "explain_debug",
  "draft_commit_message",
  "route_recommendation",
  "delegate",
]);
export type TurnTaskClass = typeof TurnTaskClass.Type;

export const turnTaskClasses: ReadonlyArray<TurnTaskClass> = [
  "local_answer",
  "completion",
  "next_edit",
  "propose_edit",
  "explain_failure",
  "explain_debug",
  "draft_commit_message",
  "route_recommendation",
  "delegate",
];

const boundedTurnText = S.String.check(S.isMinLength(1), S.isMaxLength(MAX_TURN_INPUT_CHARS));
const boundedInstruction = S.String.check(S.isMinLength(1), S.isMaxLength(MAX_TURN_INPUT_CHARS));
const boundedObjective = S.String.check(S.isMinLength(1), S.isMaxLength(MAX_TURN_INPUT_CHARS));

/**
 * The typed turn intent. The renderer can send one intent. It cannot select a
 * provider, build an authoritative prompt, or claim an action. Each intent maps
 * to exactly one task class.
 */
export const TurnIntent = S.Union([
  S.TaggedStruct("Ask", { text: boundedTurnText }),
  S.TaggedStruct("Complete", { anchor: EditorAnchor }),
  S.TaggedStruct("NextEdit", { anchor: EditorAnchor }),
  S.TaggedStruct("ProposeEdit", { instruction: boundedInstruction }),
  S.TaggedStruct("ExplainFailure", { runRef: RunRef }),
  S.TaggedStruct("ExplainDebug", { debugRef: DebugRef }),
  S.TaggedStruct("DraftCommitMessage", { sourceControlRef: SourceControlRef }),
  S.TaggedStruct("RecommendRoute", { objective: boundedObjective }),
]);
export type TurnIntent = typeof TurnIntent.Type;

/**
 * Deterministic map from a turn intent tag to its task class. Policy, not a
 * model, owns this mapping.
 */
export const turnIntentTaskClass: Readonly<Record<TurnIntent["_tag"], TurnTaskClass>> = {
  Ask: "local_answer",
  Complete: "completion",
  NextEdit: "next_edit",
  ProposeEdit: "propose_edit",
  ExplainFailure: "explain_failure",
  ExplainDebug: "explain_debug",
  DraftCommitMessage: "draft_commit_message",
  RecommendRoute: "route_recommendation",
};

/**
 * Honest usage truth for one turn. Apple FM local inference reports estimated
 * usage. A provider that returns exact counts reports exact. A lane that cannot
 * report usage reports unknown.
 */
export const TurnUsageTruth = S.Literals(["exact", "estimated", "unknown"]);
export type TurnUsageTruth = typeof TurnUsageTruth.Type;
export const turnUsageTruths: ReadonlyArray<TurnUsageTruth> = ["exact", "estimated", "unknown"];

/**
 * The full turn lifecycle state set. `accepted`, `routing`, `dispatching`, and
 * `streaming` are non-terminal. `completed`, `refused`, `failed`, and
 * `cancelled` are terminal.
 */
export const TurnLifecycleState = S.Literals([
  "accepted",
  "routing",
  "dispatching",
  "streaming",
  "completed",
  "refused",
  "failed",
  "cancelled",
]);
export type TurnLifecycleState = typeof TurnLifecycleState.Type;

export const TurnTerminalState = S.Literals(["completed", "refused", "failed", "cancelled"]);
export type TurnTerminalState = typeof TurnTerminalState.Type;
export const turnTerminalStates: ReadonlyArray<TurnTerminalState> = [
  "completed",
  "refused",
  "failed",
  "cancelled",
];

/**
 * Every turn refusal reason. A refusal keeps the user input and shows the exact
 * reason. It never silently changes provider. Decode failure and action-claim
 * output never dispatch.
 */
export const TurnRefusalReason = S.Literals([
  "route_closed_no_candidate",
  "provider_unavailable",
  "provider_unauthorized",
  "provider_unadmitted",
  "unsupported_platform",
  "not_ready",
  "helper_missing",
  "helper_unreachable",
  "malformed_output",
  "oversized_output",
  "empty_output",
  "action_claim_rejected",
  "decode_failed",
  "privacy_blocked",
  "cost_blocked",
  "cancelled_before_start",
]);
export type TurnRefusalReason = typeof TurnRefusalReason.Type;
export const turnRefusalReasons: ReadonlyArray<TurnRefusalReason> = [
  "route_closed_no_candidate",
  "provider_unavailable",
  "provider_unauthorized",
  "provider_unadmitted",
  "unsupported_platform",
  "not_ready",
  "helper_missing",
  "helper_unreachable",
  "malformed_output",
  "oversized_output",
  "empty_output",
  "action_claim_rejected",
  "decode_failed",
  "privacy_blocked",
  "cost_blocked",
  "cancelled_before_start",
];

/** A bounded blocker-reference list attached to a refusal or failure. */
export const TurnBlockerRefs = S.Array(
  S.String.check(S.isMinLength(1), S.isMaxLength(120), S.isPattern(/^[a-z0-9_.]+$/)),
).check(S.isMaxLength(MAX_TURN_BLOCKER_REFS));
export type TurnBlockerRefs = typeof TurnBlockerRefs.Type;

/** Programmatic record of the AFS-00 schema compatibility rules. */
export const AFS_SCHEMA_COMPATIBILITY_RULES: ReadonlyArray<string> = [
  "A schema literal names one immutable contract version.",
  "Inside a version, only optional-field additions and value widenings are compatible.",
  "A new union case, removed field, newly required field, or tighter bound needs a new version.",
  "A persisted or wire identifier keeps its exact spelling across a migration.",
  "A migration adds a new decoder path and does not reinterpret an old one.",
];
