import { Schema } from "effect"

import type { FullAutoCandidateWorkItem } from "./full-auto-objective-selection.ts"
import {
  FULL_AUTO_SELF_CLAIM_SCHEMA,
  FULL_AUTO_SELF_CLAIM_SCOPE_LIMIT,
  FullAutoSelfClaimSchema,
  type FullAutoSelfClaim,
} from "./full-auto-run-registry.ts"
import { deriveFullAutoVerificationSpec } from "./full-auto-verification.ts"

/**
 * HANDS-6 (#9184): the INITIATIVE policy for an autonomy-enabled Full Auto run.
 *
 * The weakness this fixes was surfaced live: a no-direction Full Auto run read
 * the docs/roadmap/authority programs, correctly identified the next valuable
 * work, then STOPPED without acting -- reasoning "I did not mutate code because
 * the live issue ledger has no open claim." That default is wrong. The claim
 * protocol only makes a live GitHub issue the NORMAL cross-session ledger; it
 * explicitly says an exact accepted plan/work packet is the ledger when a
 * feature issue cannot exist. A self-selected, owner-priority-aligned,
 * host-verifiable action is a valid basis to act -- the run self-claims (a
 * local/relay work-packet claim, NOT a new GitHub issue) and PROCEEDS.
 *
 * This module is a PURE decision function plus the self-claim constructor. It
 * deliberately holds no dispatch, spend, release, or public-claim authority and
 * self-amplifies nothing: it only decides whether initiative is admissible and
 * builds the honest claim record. The completion gate (HANDS-2, host
 * verification) still admits completion, owner Stop/override still halts, and
 * owner-priority selection (HANDS-1) still bounds WHAT can be selected.
 *
 * The KEY behavioral delta: the pre-existence of an open GitHub issue/claim is
 * NOT consulted as a reason to proceed. It is not an input to this function.
 */

const decodeFullAutoSelfClaim = Schema.decodeUnknownSync(FullAutoSelfClaimSchema)

export type FullAutoInitiativeHoldReason =
  | "autonomy_disabled"
  | "owner_halt"
  | "no_candidate"
  | "not_host_verifiable"
  | "conflicting_active_claim"

export type FullAutoInitiativeDecision =
  | Readonly<{
      /** Autonomy self-selected an owner-priority, host-verifiable action with
       * no conflicting claim: record the self-claim and act on a bounded unit. */
      action: "proceed"
      selfClaim: FullAutoSelfClaim
      rationale: string
    }>
  | Readonly<{
      /** Initiative is not admissible right now. `reason` is typed and legible;
       * a hold is never a silent stop and never a "no open GitHub issue" stop. */
      action: "hold"
      reason: FullAutoInitiativeHoldReason
      rationale: string
    }>

/**
 * Whether a self-selected candidate is HOST-VERIFIABLE: its named verification
 * resolves, through the SAME engine HANDS-2 uses, to a runnable check (a
 * command or a present evidence ref), not `none`. Initiative is gated on this
 * so nothing self-selects work whose completion the host cannot later verify --
 * initiative and the completion gate stay on one truth.
 */
export const isFullAutoCandidateHostVerifiable = (candidate: FullAutoCandidateWorkItem): boolean =>
  deriveFullAutoVerificationSpec(`verify: ${candidate.verification}`).kind !== "none"

export type FullAutoInitiativeInput = Readonly<{
  /** Initiative applies ONLY to an autonomy-enabled run. A default (non-autonomy)
   * run always holds with `autonomy_disabled` -- its behavior is unchanged. */
  autonomyEnabled: boolean
  /** Owner Stop/override signal. When true, NOTHING self-selects work. */
  ownerHalt: boolean
  /** The self-selected owner-priority candidate (HANDS-1 objective selection).
   * Null when selection produced no proposal. */
  candidate: FullAutoCandidateWorkItem | null
  /** A CONFLICTING active claim already owns this scope/contract (an unreleased
   * prior claim or another lane). The mere ABSENCE of a GitHub issue is NOT a
   * conflict and is never consulted here. */
  conflictingActiveClaim: boolean
  runRef: string
  now?: () => Date
  mintClaimRef?: () => string
}>

const mintDefaultClaimRef = (runRef: string, now: () => Date): string => {
  const random = Math.random().toString(36).slice(2, 10)
  return `claim.self.${runRef}.${now().getTime().toString(36)}.${random}`
}

/**
 * Build the honest, relay-ready self-claim for a self-selected candidate. It
 * records the legitimacy basis (`self_selected`, never a GitHub issue), the
 * bounded scope, the real owner-priority citations (never invented), and the
 * named host-runnable verification the completion gate enforces.
 */
export const makeFullAutoSelfClaim = (
  candidate: FullAutoCandidateWorkItem,
  options: Readonly<{ runRef: string; now?: () => Date; mintClaimRef?: () => string; ledger?: "local" | "relay" }>,
): FullAutoSelfClaim => {
  const now = options.now ?? (() => new Date())
  const claimRef = options.mintClaimRef?.() ?? mintDefaultClaimRef(options.runRef, now)
  return decodeFullAutoSelfClaim({
    schema: FULL_AUTO_SELF_CLAIM_SCHEMA,
    claimRef,
    runRef: options.runRef,
    scope: candidate.title.slice(0, FULL_AUTO_SELF_CLAIM_SCOPE_LIMIT),
    basis: "self_selected",
    verification: candidate.verification,
    citedRefs: candidate.citedRefs,
    ledger: options.ledger ?? "local",
    claimedAt: now().toISOString(),
  })
}

/**
 * Decide whether an autonomy run may TAKE INITIATIVE now. The order of holds is
 * deliberate: an owner halt beats everything; then a real candidate must exist;
 * then it must be host-verifiable (the HANDS-2 tie); then no conflicting claim
 * may already own the scope. Only then does it PROCEED and self-claim. The
 * absence of an open GitHub issue is nowhere in this ladder -- that is the fix.
 */
export const decideFullAutoInitiative = (input: FullAutoInitiativeInput): FullAutoInitiativeDecision => {
  if (!input.autonomyEnabled) {
    return {
      action: "hold",
      reason: "autonomy_disabled",
      rationale:
        "Initiative applies only to an autonomy-enabled run; default Full Auto behavior is unchanged.",
    }
  }
  if (input.ownerHalt) {
    return {
      action: "hold",
      reason: "owner_halt",
      rationale: "The owner requested Stop or override; initiative never overrides an owner halt.",
    }
  }
  if (input.candidate === null) {
    return {
      action: "hold",
      reason: "no_candidate",
      rationale: "No self-selected owner-priority candidate is available to act on.",
    }
  }
  if (!isFullAutoCandidateHostVerifiable(input.candidate)) {
    return {
      action: "hold",
      reason: "not_host_verifiable",
      rationale:
        "The candidate has no host-runnable verification; the host could not gate its completion, so initiative does not proceed.",
    }
  }
  if (input.conflictingActiveClaim) {
    return {
      action: "hold",
      reason: "conflicting_active_claim",
      rationale: "A conflicting active claim already owns this scope; initiative yields rather than colliding.",
    }
  }
  const selfClaim = makeFullAutoSelfClaim(input.candidate, {
    runRef: input.runRef,
    ...(input.now === undefined ? {} : { now: input.now }),
    ...(input.mintClaimRef === undefined ? {} : { mintClaimRef: input.mintClaimRef }),
  })
  return {
    action: "proceed",
    selfClaim,
    rationale:
      "Autonomy self-selected an owner-priority, host-verifiable action with no conflicting claim; the absence of an open GitHub issue is not a reason to stop, so the run self-claims and proceeds to a bounded verified unit.",
  }
}
