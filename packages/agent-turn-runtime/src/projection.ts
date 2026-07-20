import { Schema as S } from "effect";

import {
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
export const deriveSafeProjection = (input: SafeProjectionInput): SafeTurnProjection =>
  decodeSafeProjection({
    schema: SAFE_TURN_PROJECTION_SCHEMA_LITERAL,
    threadRef: input.record.threadRef,
    requestRef: input.record.requestRef,
    ...(input.record.providerTurnRef === null ? {} : { providerTurnRef: input.record.providerTurnRef }),
    cardState: cardStateForLifecycle(input.record.state),
    ...(input.candidate === undefined ? {} : { candidate: input.candidate }),
    dataDestination: input.dataDestination,
    usageTruth: input.usageTruth,
    localOnly: input.localOnly,
    updatedAt: input.updatedAt,
    messageChain: input.messageChain ?? [],
    evidenceRefs: input.evidenceRefs ?? [],
  });

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
