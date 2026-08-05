/**
 * Local message table for the status/progress surface, shaped for the
 * shared `@openagentsinc/swap-i18n` table. The MKT-SWP §17 identifiers
 * (`swp_status_gap`, `swp_status_fork`, `swp_settlement_overclaim`, ...)
 * already render through `swap-i18n`'s error table; this file carries only
 * the status-surface copy that is not a §17 identifier.
 *
 * Copy laws (issue #9321):
 * - A gap reads as UNKNOWN with an explanation. It never reads as
 *   "in progress".
 * - A fork is loud: both conflicting claims are named as retained.
 * - A claim is attributed: "the provider's signed claim", never "the swap
 *   is settled".
 * - `unresolved` reads as unresolved — explicitly not failed and not
 *   complete.
 */
import { swpErrorMessageKey, type SwpErrorMessageKey } from "@openagentsinc/swap-i18n";

export interface StatusMessage {
  readonly key: `swap.status.${string}`;
  readonly message: string;
}

export const STATUS_MESSAGES = {
  "swap.status.lane.requester": "Your signed status stream",
  "swap.status.lane.provider": "The provider's signed status stream",
  "swap.status.gap_unknown":
    "Status {seq} from the {role} is missing. Progress at this step is unknown — missing is not the same as in progress.",
  "swap.status.gap_not_closed_by_later":
    "Later status records do not close this gap. Only the exact missing record can.",
  "swap.status.fork_detected":
    "The {role} signed two different status records at the same sequence number. Both are shown below; neither is trusted, and this stream cannot advance the swap.",
  "swap.status.claim_attribution":
    "This is the {role}'s signed claim. It is evidence of what they say, not settlement.",
  "swap.status.evidence_attribution": "Verified by {source} at rung {rung}.",
  "swap.status.claimed_not_proved":
    "The {role} claims {state}, but the evidence on hand proves only {rung}. The display stays at what is proved.",
  "swap.status.unresolved_explainer":
    "Unresolved means the final disposition of funds is unknown. It is not a failure and not a completion. Keep this device's swap data and keep watching.",
  "swap.status.time_estimate":
    "Estimated time. Block height {height} is the binding deadline; the clock is only an estimate.",
  "swap.status.stop_trusting_claims":
    "A timeout boundary has passed. Counterparty status claims are no longer trusted for progress; your own exit path governs from here.",
  "swap.status.rung.pledged": "Claimed by a signer",
  "swap.status.rung.reserved": "Capacity reserved",
  "swap.status.rung.measured": "Observed on the rail",
  "swap.status.rung.verified": "Re-derived by a verifier",
  "swap.status.rung.paid": "Payment proved",
  "swap.status.rung.settled": "Final under the quoted rules",
  "swap.status.exit.none_needed": "Nothing for you to do.",
  "swap.status.exit.claim": "Claim your funds now — your claim window is open.",
  "swap.status.exit.refund": "Broadcast your refund. Your exit package works without the coordinator.",
  "swap.status.exit.rescue":
    "Use your rescue key to recover this swap. It does not depend on the coordinator.",
  "swap.status.exit.dispute": "Open the dispute process named in the quote.",
  "swap.status.exit.keep_watching": "Keep this page and your swap data. Watching continues.",
  "swap.status.terminal.completed": "Completed — both legs are final under the quoted rules.",
  "swap.status.terminal.rejected": "Rejected before the order became effective. No funds moved.",
  "swap.status.terminal.cancelled": "Cancelled by mutual consent. No irreversible effect remains.",
  "swap.status.terminal.expired": "Expired. Nothing remains funded.",
  "swap.status.terminal.failed": "Failed. The loss accounting below itemises exactly what happened.",
  "swap.status.terminal.refunded": "Refunded — every funded principal reached a verified refund.",
  "swap.status.terminal.disputed": "Disputed. A contested effect remains under the dispute process.",
  "swap.status.terminal.unresolved":
    "Unresolved — the final state of some principal or record is unknown. This is not failed and not complete.",
  "swap.status.loss.unknown_value": "unknown",
  "swap.status.loss.fees_note": "Fees are listed separately. They are never folded into principal.",
  "swap.status.ladder.funding_window": "Funding accepted until block {height}",
  "swap.status.ladder.cooperative_claim_window":
    "Provider must complete its claim by block {height}",
  "swap.status.ladder.refund_valid": "Your refund becomes valid at block {height}",
  "swap.status.ladder.provider_lock_window": "Provider must lock funds by block {height}",
  "swap.status.ladder.requester_claim_window": "Claim your funds before block {height}",
  "swap.status.ladder.provider_refund_window": "Provider refund path opens at block {height}",
  "swap.status.ladder.hold_invoice_expiry": "Held payment expires at block {height}",
  "swap.status.ladder.destination_claim_window":
    "Claim the destination before block {height}",
  "swap.status.ladder.source_refund_valid": "Your source refund becomes valid at block {height}",
} as const satisfies Record<`swap.status.${string}`, string>;

export type StatusMessageKey = keyof typeof STATUS_MESSAGES;

/** The shared-table key for a §17 identifier surfaced on this view. */
export const statusErrorKey = swpErrorMessageKey;
export type { SwpErrorMessageKey };
