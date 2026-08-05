/**
 * Timeout-ladder rungs (MKT-SWP §8): what expires when, and what the user's
 * exit is at each rung.
 *
 * Expiry is block-height based, not wall-clock (teardown §2.4). Heights are
 * the authority; times are estimates and are flagged as estimates — an
 * estimated time is never converted into consensus authority. Once the
 * user's own unilateral exit window opens, counterparty status claims stop
 * being trusted for progress on that leg.
 */
import type { SwapFlow, UserExitKind } from "./model.js";

export interface SubmarineLadder {
  readonly kind: "submarine";
  /** Last height at which requester funding is accepted. */
  readonly hFund: number;
  /** Provider's last safe cooperative-claim height. */
  readonly hClaim: number;
  /** First height at which the requester refund path is valid. */
  readonly hRefund: number;
  readonly invoiceExpirationTime: number;
}

export interface ReverseLadder {
  readonly kind: "reverse";
  readonly hLockLast: number;
  readonly hUserClaim: number;
  readonly hProviderRefund: number;
  /** Shortest incoming Lightning HTLC expiry (height). */
  readonly hHoldExpiry: number;
}

export interface ChainLadder {
  readonly kind: "chain";
  readonly hDestinationRefund: number;
  readonly hSourceRefund: number;
}

export type TimeoutLadder = SubmarineLadder | ReverseLadder | ChainLadder;

export interface LadderRungDescriptor {
  readonly id: string;
  /** The height bound — the consensus authority for this rung. */
  readonly height: number;
  readonly actor: "requester" | "provider";
  /** Message key for what this window is. */
  readonly labelKey: `swap.status.ladder.${string}`;
  /** The user's exit while this window is open. */
  readonly exitWithin: UserExitKind;
  /** The user's exit once the boundary has passed. */
  readonly exitAfter: UserExitKind;
  /**
   * When true, crossing this boundary means counterparty status claims are
   * no longer trusted for progress: the user's unilateral path governs.
   */
  readonly crossingStopsTrustingClaims: boolean;
}

export function ladderRungs(ladder: TimeoutLadder): readonly LadderRungDescriptor[] {
  switch (ladder.kind) {
    case "submarine":
      return [
        {
          id: "funding_window",
          height: ladder.hFund,
          actor: "requester",
          labelKey: "swap.status.ladder.funding_window",
          exitWithin: "none_needed",
          exitAfter: "none_needed",
          crossingStopsTrustingClaims: false,
        },
        {
          id: "cooperative_claim_window",
          height: ladder.hClaim,
          actor: "provider",
          labelKey: "swap.status.ladder.cooperative_claim_window",
          exitWithin: "keep_watching",
          exitAfter: "refund",
          crossingStopsTrustingClaims: true,
        },
        {
          id: "refund_valid",
          height: ladder.hRefund,
          actor: "requester",
          labelKey: "swap.status.ladder.refund_valid",
          exitWithin: "keep_watching",
          exitAfter: "refund",
          crossingStopsTrustingClaims: true,
        },
      ];
    case "reverse":
      return [
        {
          id: "provider_lock_window",
          height: ladder.hLockLast,
          actor: "provider",
          labelKey: "swap.status.ladder.provider_lock_window",
          exitWithin: "keep_watching",
          exitAfter: "none_needed",
          crossingStopsTrustingClaims: false,
        },
        {
          id: "requester_claim_window",
          height: ladder.hUserClaim,
          actor: "requester",
          labelKey: "swap.status.ladder.requester_claim_window",
          exitWithin: "claim",
          exitAfter: "rescue",
          crossingStopsTrustingClaims: true,
        },
        {
          id: "provider_refund_window",
          height: ladder.hProviderRefund,
          actor: "provider",
          labelKey: "swap.status.ladder.provider_refund_window",
          exitWithin: "keep_watching",
          exitAfter: "keep_watching",
          crossingStopsTrustingClaims: true,
        },
        {
          id: "hold_invoice_expiry",
          height: ladder.hHoldExpiry,
          actor: "provider",
          labelKey: "swap.status.ladder.hold_invoice_expiry",
          exitWithin: "keep_watching",
          exitAfter: "keep_watching",
          crossingStopsTrustingClaims: true,
        },
      ];
    case "chain":
      return [
        {
          id: "destination_claim_window",
          height: ladder.hDestinationRefund,
          actor: "requester",
          labelKey: "swap.status.ladder.destination_claim_window",
          exitWithin: "claim",
          exitAfter: "keep_watching",
          crossingStopsTrustingClaims: true,
        },
        {
          id: "source_refund_valid",
          height: ladder.hSourceRefund,
          actor: "requester",
          labelKey: "swap.status.ladder.source_refund_valid",
          exitWithin: "keep_watching",
          exitAfter: "refund",
          crossingStopsTrustingClaims: true,
        },
      ];
  }
}

export interface LadderRungView extends LadderRungDescriptor {
  readonly status: "upcoming" | "active" | "passed";
  /** The user's exit given the current height. */
  readonly exitNow: UserExitKind;
  /** Wall-clock estimate for the boundary; ALWAYS an estimate, never authority. */
  readonly estimatedTime: number | null;
  readonly timeIsEstimate: true;
}

export interface LadderView {
  readonly rungs: readonly LadderRungView[];
  /** True once any crossed boundary opened the user's unilateral path. */
  readonly stopTrustingCounterpartyClaims: boolean;
  /** Height observation is required; without it no boundary is treated as crossed. */
  readonly currentHeight: number | null;
}

export function ladderView(
  flow: SwapFlow,
  ladder: TimeoutLadder,
  currentHeight: number | null,
  estimateTimeForHeight?: (height: number) => number,
): LadderView {
  if (
    (flow === "submarine" && ladder.kind !== "submarine") ||
    (flow === "reverse" && ladder.kind !== "reverse") ||
    (flow === "chain" && ladder.kind !== "chain")
  ) {
    throw new Error("timeout ladder does not match the swap flow");
  }
  const descriptors = ladderRungs(ladder);
  let stopTrusting = false;
  let previousPassed = true;
  const rungs = descriptors.map((descriptor): LadderRungView => {
    const passed = currentHeight !== null && currentHeight >= descriptor.height;
    if (passed && descriptor.crossingStopsTrustingClaims) stopTrusting = true;
    const status: LadderRungView["status"] = passed
      ? "passed"
      : previousPassed
        ? "active"
        : "upcoming";
    previousPassed = passed;
    return Object.assign({}, descriptor, {
      status,
      exitNow: passed ? descriptor.exitAfter : descriptor.exitWithin,
      estimatedTime: estimateTimeForHeight ? estimateTimeForHeight(descriptor.height) : null,
      timeIsEstimate: true as const,
    });
  });
  return { rungs, stopTrustingCounterpartyClaims: stopTrusting, currentHeight };
}
