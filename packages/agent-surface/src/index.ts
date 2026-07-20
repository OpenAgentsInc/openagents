import { Schema as S } from "effect";

import {
  MAX_TURN_OUTPUT_CHARS,
  SafeMessageChainEntry,
  type AgentCardState,
  type SafeTurnProjection,
  type TurnDataDestination,
  type TurnProviderCandidate,
  type TurnStageKind,
  type TurnUsageTruth,
} from "@openagentsinc/agent-runtime-schema";

/**
 * `@openagentsinc/agent-surface` — the UI-neutral surface projectors (AFS-00
 * reservation, AFS-04 implementation).
 *
 * AFS-04 adds the first real projectors that turn canonical turn facts into safe
 * cards and message chains. This package owns pure projectors and surface-intent
 * helpers. It must not own schemas, renderers, or providers. It imports its
 * schemas from `@openagentsinc/agent-runtime-schema`; it must not define a second
 * wire contract.
 *
 * The redaction boundary lives here as CODE, not only as a schema shape. A
 * provider run reports an observed activity, and this module selects ONLY the
 * safe, bounded fields (role, bounded message text, a bounded tool LABEL, a file-
 * change COUNT, and a command-output BYTE COUNT). It never reads a raw command
 * argument, raw command output, a local path, a token, or a secret. An observed
 * activity may carry those fields; the projector structurally ignores every
 * field it does not name, so raw content can never reach a card or inspector.
 */
export const AGENT_SURFACE_PACKAGE = "@openagentsinc/agent-surface" as const;

const decodeMessageChainEntry = S.decodeUnknownSync(SafeMessageChainEntry);

/**
 * A safe, bounded card model a renderer shows for one turn or delegated agent.
 * It is display only. It is never proof, acceptance, delivery, or release. A
 * card cannot show `running` before a host start receipt: `cardState` is derived
 * deterministically from the driver-neutral projection, whose lifecycle only
 * reaches a running state after the provider start receipt exists.
 */
export interface SafeAgentCard {
  readonly requestRef: string;
  readonly threadRef: string;
  readonly providerTurnRef: string | null;
  readonly cardState: AgentCardState;
  readonly stage: TurnStageKind;
  readonly provider: TurnProviderCandidate | null;
  readonly dataDestination: TurnDataDestination;
  readonly usageTruth: TurnUsageTruth;
  readonly localOnly: boolean;
  readonly updatedAt: string;
  readonly messageCount: number;
}

/**
 * The projector surface reserved by AFS-00. A projector reads a safe turn
 * projection and emits a bounded card. The projector is a pure function; it
 * acquires no schema, renderer, or provider authority.
 */
export interface SafeCardProjector {
  readonly project: (projection: SafeTurnProjection) => {
    readonly cardState: AgentCardState;
    readonly stage: TurnStageKind;
  };
}

/** The AFS-04 card is always a `card` stage: a bounded UI projection, never an action or a release. */
const CARD_STAGE: TurnStageKind = "card";

/** Build the AFS-00 `SafeCardProjector`. It derives the bounded card facts only. */
export const makeSafeCardProjector = (): SafeCardProjector => ({
  project: (projection) => ({ cardState: projection.cardState, stage: CARD_STAGE }),
});

/** Project a safe turn projection into the richer bounded card model. */
export const projectSafeAgentCard = (projection: SafeTurnProjection): SafeAgentCard => ({
  requestRef: projection.requestRef,
  threadRef: projection.threadRef,
  providerTurnRef: projection.providerTurnRef ?? null,
  cardState: projection.cardState,
  stage: CARD_STAGE,
  provider: projection.candidate ?? null,
  dataDestination: projection.dataDestination,
  usageTruth: projection.usageTruth,
  localOnly: projection.localOnly,
  updatedAt: projection.updatedAt,
  messageCount: projection.messageChain.length,
});

/** True when a card is in a terminal (non-live) state. */
export const isTerminalCardState = (state: AgentCardState): boolean =>
  state === "done" || state === "refused" || state === "failed" || state === "cancelled";

/** True when a card may show a live running/queued affordance. */
export const isLiveCardState = (state: AgentCardState): boolean =>
  state === "queued" || state === "running";

/**
 * A pre-safe observed activity from a provider run. The projector reads ONLY the
 * named safe fields. Every other property — a raw command, raw output text, a
 * local path, a token, or any secret — is structurally ignored and can never
 * reach a card or inspector. The caller (a provider adapter) selects the safe
 * fields; this projector is the final gate that bounds and decodes them.
 */
export interface ObservedAgentActivity {
  readonly role: SafeMessageChainEntry["role"];
  /** A bounded message text (agent/user/system prose or a reasoning summary). */
  readonly text?: string;
  /** A bounded tool LABEL, never the tool arguments (for example `shell`, `apply_patch`). */
  readonly toolLabel?: string;
  /** The number of files a change touched, never the file contents or paths. */
  readonly fileChangeCount?: number;
  /** The byte COUNT of command output, never the output text. */
  readonly commandOutputByteCount?: number;
}

/** Deterministic, stable entry ref for a projected message-chain entry. */
const entryRef = (requestRef: string, index: number): string => `${requestRef}.chain.${index}`;

const boundedText = (value: string): string => value.slice(0, MAX_TURN_OUTPUT_CHARS);

const boundedLabel = (value: string): string => value.slice(0, 120);

const safeCount = (value: number): number => (Number.isFinite(value) && value >= 0 ? Math.floor(value) : 0);

/**
 * Build ONE safe message-chain entry from an observed activity. It reads only
 * the named safe fields and bounds each. Any other field on `input` is ignored.
 * The result is decoded through the frozen `SafeMessageChainEntry` schema, so it
 * cannot carry a field outside the safe shape.
 */
export const buildSafeMessageChainEntry = (
  requestRef: string,
  index: number,
  input: ObservedAgentActivity,
): SafeMessageChainEntry =>
  decodeMessageChainEntry({
    entryRef: entryRef(requestRef, index),
    role: input.role,
    text: input.text === undefined ? "" : boundedText(input.text),
    ...(input.toolLabel === undefined ? {} : { toolLabel: boundedLabel(input.toolLabel) }),
    ...(input.fileChangeCount === undefined ? {} : { fileChangeCount: safeCount(input.fileChangeCount) }),
    ...(input.commandOutputByteCount === undefined
      ? {}
      : { commandOutputByteCount: safeCount(input.commandOutputByteCount) }),
  });

/**
 * Project a list of observed activities into a bounded safe message chain. The
 * chain is capped at the frozen retained-segment bound (256). It is the redacted,
 * owner-only message chain the right pane renders.
 */
export const projectSafeMessageChain = (
  requestRef: string,
  activities: ReadonlyArray<ObservedAgentActivity>,
): ReadonlyArray<SafeMessageChainEntry> =>
  activities.slice(0, 256).map((activity, index) => buildSafeMessageChainEntry(requestRef, index, activity));

/**
 * The safe message chain already carried by a turn projection. Every entry is
 * already the frozen safe shape; this is the read-side helper the inspector uses.
 */
export const safeMessageChainOf = (
  projection: SafeTurnProjection,
): ReadonlyArray<SafeMessageChainEntry> => projection.messageChain;
