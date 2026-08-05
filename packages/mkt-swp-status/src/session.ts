/**
 * The session projection: two authored lanes folded against the §9 state
 * machine, the §11 evidence rungs, the §8 timeout ladder, and §15 Closes.
 *
 * This is the exported projection that is the ONLY definition of
 * terminality (issue #9321 scope 6), and it is deterministic in the SET of
 * records: arrival order never changes the result, duplicates are
 * idempotent, and a resubscription replay after reconnect converges to the
 * identical view.
 */
import { ladderView, type LadderView, type TimeoutLadder } from "./ladder.js";
import { foldLane, type LaneProjection } from "./lane.js";
import type {
  CloseRecord,
  EvidenceRung,
  ParticipantRole,
  StatusClaim,
  StatusState,
  SwapEvidence,
  SwapFlow,
} from "./model.js";
import { claimVerdict, provenRungView, type RungView } from "./rungs.js";
import {
  admittedSignerFor,
  allowedSuccessors,
  classifySwpState,
  HAPPY_PATH,
} from "./states.js";
import { closesView, type ClosesView } from "./terminal.js";

export type InvalidClaimReason =
  | "swp_status_signer_invalid"
  | "swp_status_transition_invalid"
  | "swp_settlement_overclaim";

export interface RetainedClaim {
  readonly claim: StatusClaim;
  readonly disposition:
    | { readonly kind: "advanced" }
    | { readonly kind: "observation" }
    | { readonly kind: "invalid"; readonly reason: InvalidClaimReason }
    | {
        readonly kind: "unproven";
        readonly reason: "swp_settlement_overclaim";
        readonly requiredRung: EvidenceRung;
        /** What the claim's own evidence classes actually prove. */
        readonly provenRung: EvidenceRung | null;
      };
}

export interface SessionInput {
  readonly flow: SwapFlow;
  readonly sessionId: string;
  readonly orderId: string;
  /** Any order, any duplication: the fold is set-deterministic. */
  readonly statuses: readonly StatusClaim[];
  readonly evidence?: readonly SwapEvidence[];
  readonly closes?: readonly CloseRecord[];
  readonly ladder?: TimeoutLadder;
  readonly currentHeight?: number;
  readonly estimateTimeForHeight?: (height: number) => number;
}

export interface SwapProgressView {
  readonly flow: SwapFlow;
  readonly lanes: {
    readonly requester: LaneProjection;
    readonly provider: LaneProjection;
  };
  /**
   * The last swp_state that a valid, admitted, evidence-satisfied claim
   * established. Null before any valid claim.
   */
  readonly lastValidSwpState: string | null;
  /** The §9 base state derived from lastValidSwpState. */
  readonly baseState: StatusState | null;
  /**
   * The display key for the headline. `unresolved` displays as unresolved
   * (its base state is `failed`, but that is not what the user is shown),
   * and a claimed-but-unproven terminal displays as the claim it is.
   */
  readonly displayKey: `swap.status.display.${string}`;
  /** The narrowest rung the exact evidence proves, with attribution. */
  readonly rung: RungView;
  /** Every claim retained with its disposition — nothing is silently dropped. */
  readonly retained: readonly RetainedClaim[];
  /**
   * Progress integrity: `unknown_gap` / `unknown_fork` when either lane has
   * a break. A gap renders as unknown-with-explanation, never as progress.
   */
  readonly integrity: "intact" | "unknown_gap" | "unknown_fork" | "unknown_chain_break";
  readonly ladder: LadderView | null;
  readonly closes: ClosesView;
  /** THE terminality answer for this session. */
  readonly watchTerminal: boolean;
}

const laneBreakKind = (lane: LaneProjection): SwapProgressView["integrity"] | null =>
  lane.integrity.kind === "gap"
    ? "unknown_gap"
    : lane.integrity.kind === "fork"
      ? "unknown_fork"
      : lane.integrity.kind === "previous_mismatch"
        ? "unknown_chain_break"
        : null;

export function projectSession(input: SessionInput): SwapProgressView {
  const relevant = input.statuses.filter(
    (status) => status.sessionId === input.sessionId && status.orderId === input.orderId,
  );
  const requesterLane = foldLane("requester", relevant);
  const providerLane = foldLane("provider", relevant);
  const evidence = input.evidence ?? [];
  const rung = provenRungView(evidence);

  // Deterministic dual-lane advancement over the contiguous valid prefixes.
  const retained: RetainedClaim[] = [];
  const pointers: Record<ParticipantRole, number> = { requester: 0, provider: 0 };
  const lanes: Record<ParticipantRole, LaneProjection> = {
    requester: requesterLane,
    provider: providerLane,
  };
  let lastValid: string | null = null;
  const canonicalOrder = HAPPY_PATH[input.flow];

  const disposition = (claim: StatusClaim): RetainedClaim => {
    const classified = classifySwpState(claim.swpState);
    if (!classified.ok) {
      return {
        claim,
        disposition: { kind: "invalid", reason: "swp_status_transition_invalid" },
      };
    }
    // The carried base `state` tag must agree with the §9 derivation.
    if (claim.baseState !== undefined && claim.baseState !== classified.base) {
      return {
        claim,
        disposition: { kind: "invalid", reason: "swp_status_transition_invalid" },
      };
    }
    const admitted = admittedSignerFor(input.flow, claim.swpState);
    if (admitted !== "either_observation" && admitted !== claim.role) {
      return {
        claim,
        disposition: { kind: "invalid", reason: "swp_status_signer_invalid" },
      };
    }
    const legal =
      lastValid === null
        ? claim.swpState === canonicalOrder[0] || claim.swpState === "rejected"
        : allowedSuccessors(input.flow, lastValid).includes(claim.swpState);
    if (!legal) {
      // A Status that skips a required action is retained as an invalid
      // claim and does not advance the session (§9.5). A repeat of the
      // current state is an observation, not an invalid claim.
      if (claim.swpState === lastValid) return { claim, disposition: { kind: "observation" } };
      return {
        claim,
        disposition: { kind: "invalid", reason: "swp_status_transition_invalid" },
      };
    }
    const verdict = claimVerdict(claim.swpState, evidence);
    if (verdict.kind === "unproven") {
      return {
        claim,
        disposition: {
          kind: "unproven",
          reason: "swp_settlement_overclaim",
          requiredRung: verdict.requiredRung,
          provenRung: verdict.provenRung,
        },
      };
    }
    if (admitted === "either_observation" && verdict.kind === "no_evidence_required") {
      // An observation without an evidence requirement may advance the
      // narrative state (e.g. funding_required) — it promotes no rung.
      return { claim, disposition: { kind: "advanced" } };
    }
    return { claim, disposition: { kind: "advanced" } };
  };

  for (;;) {
    const nextRequester = requesterLane.validClaims[pointers.requester];
    const nextProvider = providerLane.validClaims[pointers.provider];
    if (nextRequester === undefined && nextProvider === undefined) break;

    const candidates: readonly (readonly [ParticipantRole, StatusClaim])[] = [
      ...(nextRequester ? ([["requester", nextRequester]] as const) : []),
      ...(nextProvider ? ([["provider", nextProvider]] as const) : []),
    ];

    const judged = candidates.map(([role, claim]) => ({
      role,
      claim,
      result: disposition(claim),
    }));
    const advancing = judged.filter(({ result }) => result.disposition.kind === "advanced");

    if (advancing.length === 0) {
      // Nothing advances: consume every candidate as retained non-advancing.
      for (const { role, result } of judged) {
        retained.push(result);
        pointers[role] += 1;
      }
      continue;
    }

    // Deterministic tiebreak: the claim earlier in the canonical flow order
    // applies first; terminal/recovery states (not on the happy path) sort
    // after happy-path states, then by state name.
    advancing.sort((left, right) => {
      const li = canonicalOrder.indexOf(left.claim.swpState);
      const ri = canonicalOrder.indexOf(right.claim.swpState);
      const ln = li < 0 ? canonicalOrder.length : li;
      const rn = ri < 0 ? canonicalOrder.length : ri;
      if (ln !== rn) return ln - rn;
      return left.claim.swpState < right.claim.swpState ? -1 : 1;
    });
    const winner = advancing[0]!;
    retained.push(winner.result);
    pointers[winner.role] += 1;
    lastValid = winner.claim.swpState;
  }

  // Claims beyond the valid prefix (after a gap/fork/chain break) stay
  // visible as claims but never advance: retain them as observations or
  // invalid, judged against the frozen lastValid.
  for (const role of ["requester", "provider"] as const) {
    const lane = lanes[role];
    for (const slot of lane.slots) {
      for (const record of slot.records) {
        const consumed = lane.validClaims.slice(0, pointers[role]);
        if (consumed.some((claim) => claim.id === record.id)) continue;
        const classified = classifySwpState(record.swpState);
        retained.push({
          claim: record,
          disposition: classified.ok
            ? { kind: "observation" }
            : { kind: "invalid", reason: "swp_status_transition_invalid" },
        });
      }
    }
  }

  const integrity =
    laneBreakKind(requesterLane) === "unknown_fork" || laneBreakKind(providerLane) === "unknown_fork"
      ? "unknown_fork"
      : (laneBreakKind(requesterLane) ?? laneBreakKind(providerLane) ?? "intact");

  const baseState = lastValid === null ? null : (() => {
    const classified = classifySwpState(lastValid);
    return classified.ok ? classified.base : null;
  })();

  const displayKey: SwapProgressView["displayKey"] =
    integrity !== "intact"
      ? `swap.status.display.${integrity}`
      : lastValid === null
        ? "swap.status.display.no_valid_status"
        : lastValid === "unresolved"
          ? "swap.status.display.unresolved"
          : `swap.status.display.${lastValid}`;

  const closes = closesView(input.closes ?? []);

  return {
    flow: input.flow,
    lanes: { requester: requesterLane, provider: providerLane },
    lastValidSwpState: lastValid,
    baseState,
    displayKey,
    rung,
    retained,
    integrity,
    ladder: input.ladder
      ? ladderView(
          input.flow,
          input.ladder,
          input.currentHeight ?? null,
          input.estimateTimeForHeight,
        )
      : null,
    closes,
    watchTerminal: closes.watchTerminal,
  };
}
