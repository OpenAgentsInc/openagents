import { Effect, Schema as S, Stream } from "effect";
import { KhalaRuntimeFinishReason, KhalaRuntimeUsage } from "@openagentsinc/agent-runtime-schema";
import { HarnessCapabilityUnsupported } from "./capability.ts";
import type { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type { HarnessCursor, HarnessStreamEvent } from "./stream.ts";
import type {
  HarnessHostToolResult,
  HarnessHostToolSpec,
  HarnessToolApprovalDecision,
} from "./host-tool.ts";

/**
 * Typed failure of a turn. `failureClass` stays a string so adapters can carry
 * their own operator-facing classes (including the mandatory account-capacity
 * classes `account_exhausted` / `account_rate_limited` / auth-health) without
 * this contract enumerating every runtime's vocabulary — it aligns with
 * `@openagentsinc/harness-conformance` `HarnessFailureClass` rather than
 * duplicating it.
 */
export class HarnessTurnError extends S.TaggedErrorClass<HarnessTurnError>()(
  "AgentHarness.TurnError",
  {
    harnessId: S.String,
    sessionId: S.String,
    turnId: S.String,
    failureClass: S.String,
    detail: S.optionalKey(S.String),
    cause: S.optionalKey(S.Defect()),
  },
) {}

/** Summary a turn resolves to once its event stream completes. */
export const HarnessTurnResult = S.Struct({
  turnId: S.NonEmptyString,
  finishReason: KhalaRuntimeFinishReason,
  usage: S.optionalKey(KhalaRuntimeUsage),
  /** The `sequence` of the last event delivered — the cursor the next slice resumes from. */
  lastCursor: S.Number,
});
export interface HarnessTurnResult extends S.Schema.Type<typeof HarnessTurnResult> {}

/**
 * Live control handle for one in-flight turn. Events arrive on `events`; the
 * turn ends when the stream completes (or fails), and `done` resolves with the
 * result summary. While the turn runs the host feeds tool results, built-in
 * tool approvals, and mid-turn user messages back in, and may interrupt.
 */
export interface HarnessPromptControl {
  readonly turnId: string;
  /**
   * The turn's stream of neutral events. Completes when the turn ends. Consuming
   * it under `Effect` interruption cancels the turn (interruption replaces the
   * AI SDK `abortSignal`).
   */
  readonly events: Stream.Stream<HarnessStreamEvent, HarnessTurnError>;
  /** Resolves when the turn ends. Fails with {@link HarnessTurnError} on turn failure. */
  readonly done: Effect.Effect<HarnessTurnResult, HarnessTurnError>;
  /** Submit a host-tool result for a `tool.call` the runtime made. */
  readonly submitToolResult: (
    result: HarnessHostToolResult,
  ) => Effect.Effect<void, HarnessTurnError>;
  /** Submit an approval decision for an adapter-native built-in tool call. */
  readonly submitToolApproval: (
    toolCallId: string,
    decision: HarnessToolApprovalDecision,
  ) => Effect.Effect<void, HarnessTurnError | HarnessCapabilityUnsupported>;
  /** Inject a user message into the running turn where the runtime supports it. */
  readonly submitUserMessage: (
    text: string,
  ) => Effect.Effect<void, HarnessTurnError | HarnessCapabilityUnsupported>;
  /** Interrupt the turn. Idempotent; resolves once the runtime has been told to stop. */
  readonly interrupt: () => Effect.Effect<void>;
}

/** Input for `promptTurn`. */
export interface HarnessPromptTurnOptions {
  readonly turnId: string;
  /** Fresh input for this turn. The harness session owns its own history. */
  readonly prompt: string;
  /**
   * Free-form session instructions. The framework supplies the same value every
   * turn; the adapter applies it once (prepended to the first user message of a
   * fresh session) and never re-applies it on a resumed session.
   */
  readonly instructions?: string;
  /** Host-executed tools available to the runtime for this turn. */
  readonly tools?: ReadonlyArray<HarnessHostToolSpec>;
}

/** Input for `continueTurn`: continue the in-flight turn without a new prompt. */
export interface HarnessContinueTurnOptions {
  /** Host tools for the continued turn (needed when the adapter re-drives it). */
  readonly tools?: ReadonlyArray<HarnessHostToolSpec>;
}

/**
 * An active harness session: the unit of state continuity across turns (one
 * sandbox/workspace, one conversation history, one running runtime).
 *
 * Lifecycle is caller-owned and explicit — `detach`/`stop` return durable
 * resume state precisely so a session can OUTLIVE the current process (the
 * durable turn journal persists it). This is a deliberate divergence from an
 * auto-`Scope`-destroyed resource: scope-exit must not silently destroy a
 * session the journal still owns. `destroy` is the only teardown that discards
 * the underlying runtime/sandbox.
 */
export interface HarnessSession {
  readonly sessionId: string;
  /** Whether this session was created from `resumeFrom` / `continueFrom`. */
  readonly isResume: boolean;
  /** The model id the runtime is configured to use, when the adapter knows it. */
  readonly modelId?: string;

  /** Run one prompt turn. */
  readonly promptTurn: (
    options: HarnessPromptTurnOptions,
  ) => Effect.Effect<HarnessPromptControl, HarnessTurnError>;

  /**
   * Continue the in-flight turn without a new prompt (slice continuation).
   * Lossless when the live turn is reachable; degraded `rerun` (recomputed tail
   * after the continuation cursor) when it is not. Adapters that cannot continue
   * fail with `CapabilityUnsupported("continue_turn")`.
   */
  readonly continueTurn: (
    options: HarnessContinueTurnOptions,
  ) => Effect.Effect<HarnessPromptControl, HarnessTurnError | HarnessCapabilityUnsupported>;

  /**
   * Freeze the active turn at a precise cursor while keeping the runtime alive,
   * returning the continuation state. Adapters that cannot suspend fail with
   * `CapabilityUnsupported("suspend_turn")`.
   */
  readonly suspendTurn: () => Effect.Effect<HarnessContinuationState, HarnessCapabilityUnsupported>;

  /**
   * Trigger the runtime's own context compaction. The runtime owns compaction;
   * this is only the trigger, surfaced later as a `compaction.recorded` event.
   * Adapters whose transport cannot compact fail with
   * `CapabilityUnsupported("compact")`.
   */
  readonly compact: (
    customInstructions?: string,
  ) => Effect.Effect<void, HarnessCapabilityUnsupported>;

  /**
   * Detach from the runtime without tearing it down, returning resume state a
   * different process can pass to `start({ resumeFrom })`. Adapters that cannot
   * park fail with `CapabilityUnsupported("detach")`.
   */
  readonly detach: () => Effect.Effect<HarnessResumeState, HarnessCapabilityUnsupported>;

  /** Stop the runtime, returning resume state where the runtime persists it. */
  readonly stop: () => Effect.Effect<HarnessResumeState>;

  /** Destroy the runtime/sandbox. Idempotent. */
  readonly destroy: () => Effect.Effect<void>;
}

/** The current cursor of a session's active turn, if one is running. */
export type HarnessActiveCursor = HarnessCursor | undefined;
