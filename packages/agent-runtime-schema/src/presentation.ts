import { Schema as S } from "effect";

import { EvidenceRef } from "./artifact.js";
import { TurnProviderCandidate, TurnDataDestination } from "./provider.js";
import {
  brandedTurnRef,
  MAX_TURN_FAILURE_REASON_CHARS,
  MAX_TURN_OUTPUT_CHARS,
  ProviderTurnRef,
  TurnRequestRef,
  TurnThreadRef,
  TurnTimestamp,
  TurnUsageTruth,
} from "./turn.js";

/**
 * AFS-00 frozen safe-projection contract.
 *
 * This module freezes the UI-neutral turn projection that Desktop, web, and
 * mobile renderers decode to equivalent facts. A projection is safe: it carries
 * no helper path, loopback URL, token, raw file content, raw tool data, raw
 * command output, local path, or private transcript data. A card is only a
 * projection. It is never proof, acceptance, delivery, or release.
 *
 * Compatibility rules are the shared AFS-00 rules recorded in `turn.ts`.
 */
export const SAFE_TURN_PROJECTION_SCHEMA_LITERAL = "openagents.agent_turn_projection.v1" as const;

/**
 * The eight distinct concepts of the turn system, as one frozen typed
 * vocabulary. These terms are not interchangeable. A later concept never
 * silently stands in for an earlier one.
 *
 * - `recommendation`: an advisory model signal. It can suggest a candidate. It
 *   has no authority.
 * - `decision`: a host-derived admitted route. Only the host creates it. It
 *   selects a lane from the owner-bound set.
 * - `action`: a real effect performed by an existing host service, for example
 *   a proposal apply, a task run, a debug step, a Git operation, or a provider
 *   turn start.
 * - `card`: a bounded UI projection of a turn or agent item. It shows running,
 *   done, refused, failed, or cancelled state. It is display only.
 * - `evidence`: a recorded lifecycle, decision, output, check, or receipt
 *   reference. It is the factual record an audit reads.
 * - `acceptance`: an owner or admitted-policy disposition that admits a
 *   candidate or a proposal. It is not release.
 * - `delivery`: a completed handoff of an accepted change or result to its
 *   target, for example an applied proposal or a committed change.
 * - `release`: an evidence-gated product transition. Only a release gate makes
 *   it. A card, an acceptance, or a delivery never becomes a release by itself.
 */
export const TurnStageKind = S.Literals([
  "recommendation",
  "decision",
  "action",
  "card",
  "evidence",
  "acceptance",
  "delivery",
  "release",
]);
export type TurnStageKind = typeof TurnStageKind.Type;
export const turnStageKinds: ReadonlyArray<TurnStageKind> = [
  "recommendation",
  "decision",
  "action",
  "card",
  "evidence",
  "acceptance",
  "delivery",
  "release",
];

/** The bounded card state a renderer can show. */
export const AgentCardState = S.Literals([
  "queued",
  "running",
  "done",
  "refused",
  "failed",
  "cancelled",
]);
export type AgentCardState = typeof AgentCardState.Type;

/** A safe, bounded message-chain entry. It carries labels and counts, not raw data. */
export const SafeMessageChainEntry = S.Struct({
  entryRef: brandedTurnRef("SafeMessageChainEntryRef"),
  role: S.Literals(["user", "assistant", "tool", "system"]),
  text: S.String.check(S.isMaxLength(MAX_TURN_OUTPUT_CHARS)),
  toolLabel: S.optionalKey(S.String.check(S.isMaxLength(120))),
  fileChangeCount: S.optionalKey(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))),
  commandOutputByteCount: S.optionalKey(S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))),
});
export type SafeMessageChainEntry = typeof SafeMessageChainEntry.Type;

/**
 * The safe turn projection. It is the one shape all three surfaces decode. A
 * card cannot show running before a host start receipt exists.
 */
export const SafeTurnProjection = S.Struct({
  schema: S.Literal(SAFE_TURN_PROJECTION_SCHEMA_LITERAL),
  threadRef: TurnThreadRef,
  requestRef: TurnRequestRef,
  providerTurnRef: S.optionalKey(ProviderTurnRef),
  cardState: AgentCardState,
  /**
   * A bounded, public-safe reason for a terminal `failed`, `refused`, or
   * `cancelled` card. It is a short control-plane label (for example
   * `session_failed: delegate lane stopped`), never a raw provider error,
   * command output, path, or token. Absent for non-terminal and `done` cards.
   */
  failureReason: S.optionalKey(S.String.check(S.isMaxLength(MAX_TURN_FAILURE_REASON_CHARS))),
  candidate: S.optionalKey(TurnProviderCandidate),
  dataDestination: TurnDataDestination,
  usageTruth: TurnUsageTruth,
  localOnly: S.Boolean,
  updatedAt: TurnTimestamp,
  messageChain: S.Array(SafeMessageChainEntry).check(S.isMaxLength(256)),
  evidenceRefs: S.Array(EvidenceRef).check(S.isMaxLength(32)),
});
export type SafeTurnProjection = typeof SafeTurnProjection.Type;
