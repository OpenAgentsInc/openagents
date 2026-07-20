import { Schema as S } from "effect";

import {
  MAX_TURN_FAILURE_REASON_CHARS,
  RECEIPT_SCHEMA_LITERAL,
  SAFE_TURN_PROJECTION_SCHEMA_LITERAL,
  SafeTurnProjection,
  TurnReceipt,
  type AgentCardState,
  type RouteDecisionRef,
  type SafeMessageChainEntry,
  type TurnDataDestination,
  type TurnLifecycleState,
  type TurnProviderCandidate,
  type TurnReceiptDecision,
  type TurnUsageTruth,
} from "@openagentsinc/agent-runtime-schema";

import type { TurnStateRecord } from "./turn-state.js";

/**
 * AFS-01 safe projection and receipt derivation.
 *
 * A card is only a projection of a turn record. It is never proof, acceptance,
 * delivery, or release. A receipt is evidence, not release authority. Both are
 * derived deterministically from the driver-neutral record, so a renderer reload
 * reconstructs the terminal card from persisted state without replaying an
 * action.
 */

const decodeSafeProjection = S.decodeUnknownSync(SafeTurnProjection);
const decodeReceipt = S.decodeUnknownSync(TurnReceipt);

/** Deterministic lifecycle-to-card map. It never shows running without dispatch. */
export const cardStateForLifecycle = (state: TurnLifecycleState): AgentCardState => {
  switch (state) {
    case "accepted":
    case "routing":
      return "queued";
    case "dispatching":
    case "streaming":
      return "running";
    case "completed":
      return "done";
    case "refused":
      return "refused";
    case "failed":
      return "failed";
    case "cancelled":
      return "cancelled";
  }
};

/** Deterministic terminal-state-to-receipt-decision map. */
export const receiptDecisionForState = (state: TurnLifecycleState): TurnReceiptDecision => {
  switch (state) {
    case "completed":
      return "accepted";
    case "cancelled":
      return "cancelled";
    case "failed":
      return "failed";
    default:
      return "rejected";
  }
};

/**
 * Bound a raw failure/refusal reason into the safe, public-safe label the card
 * shows. It collapses control characters and whitespace (so a multi-line raw
 * error can never structurally leak) and truncates to the schema bound. It is
 * the single redaction boundary a reason string crosses before a projection.
 */
const CONTROL_CHARS = new RegExp("[\\u0000-\\u001f\\u007f]+", "g");
export const safeFailureReasonText = (raw: string): string =>
  raw
    .replaceAll(CONTROL_CHARS, " ")
    .replaceAll(/\s+/g, " ")
    .trim()
    .slice(0, MAX_TURN_FAILURE_REASON_CHARS);

/**
 * The bounded, public-safe reason for a terminal card, derived purely from the
 * record. A `failed` card shows the stored failure reason; a `refused` card
 * shows its typed refusal reason; a `cancelled` card shows an honest cancelled
 * line. Every other state has no reason.
 */
export const terminalFailureReason = (record: TurnStateRecord): string | undefined => {
  switch (record.state) {
    case "failed":
      return record.failureReason === null ? "failed" : safeFailureReasonText(record.failureReason);
    case "refused":
      return record.refusalReason === null ? "refused" : record.refusalReason;
    case "cancelled":
      return "cancelled";
    default:
      return undefined;
  }
};

export interface SafeProjectionInput {
  readonly record: TurnStateRecord;
  readonly dataDestination: TurnDataDestination;
  readonly usageTruth: TurnUsageTruth;
  readonly localOnly: boolean;
  readonly updatedAt: string;
  readonly candidate?: TurnProviderCandidate;
  readonly messageChain?: ReadonlyArray<SafeMessageChainEntry>;
  readonly evidenceRefs?: ReadonlyArray<string>;
}

/** Derive the one safe projection all three surfaces decode to equivalent facts. */
export const deriveSafeProjection = (input: SafeProjectionInput): SafeTurnProjection => {
  const failureReason = terminalFailureReason(input.record);
  return decodeSafeProjection({
    schema: SAFE_TURN_PROJECTION_SCHEMA_LITERAL,
    threadRef: input.record.threadRef,
    requestRef: input.record.requestRef,
    ...(input.record.providerTurnRef === null ? {} : { providerTurnRef: input.record.providerTurnRef }),
    cardState: cardStateForLifecycle(input.record.state),
    ...(failureReason === undefined ? {} : { failureReason }),
    ...(input.candidate === undefined ? {} : { candidate: input.candidate }),
    dataDestination: input.dataDestination,
    usageTruth: input.usageTruth,
    localOnly: input.localOnly,
    updatedAt: input.updatedAt,
    messageChain: input.messageChain ?? [],
    evidenceRefs: input.evidenceRefs ?? [],
  });
};

export interface TurnReceiptInput {
  readonly record: TurnStateRecord;
  readonly routeDecisionRef: RouteDecisionRef;
  readonly usageTruth: TurnUsageTruth;
  readonly evidenceRefs?: ReadonlyArray<string>;
}

/** Derive the turn receipt from a terminal record. Evidence, never release. */
export const buildTurnReceipt = (input: TurnReceiptInput): TurnReceipt =>
  decodeReceipt({
    schema: RECEIPT_SCHEMA_LITERAL,
    requestRef: input.record.requestRef,
    routeDecisionRef: input.routeDecisionRef,
    ...(input.record.providerTurnRef === null ? {} : { providerTurnRef: input.record.providerTurnRef }),
    ...(input.record.candidateRef === null ? {} : { candidateRef: input.record.candidateRef }),
    decision: receiptDecisionForState(input.record.state),
    usageTruth: input.usageTruth,
    evidenceRefs: input.evidenceRefs ?? [],
  });
