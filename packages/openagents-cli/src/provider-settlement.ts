/**
 * Provider mode: the settlement gate.
 *
 * ## What died, and what the corpse still teaches
 *
 * Pylon's provider loop -- NIP-89 announce, quote, execute on your own agent,
 * publish an output-only result, record a msat earning -- was deleted whole on
 * 2026-07-14 by VP-1 (`21e82ce8`, "retire money sites and wallet authority"),
 * along with `apps/pylon/src/provider-nip90.ts`, `labor-market.ts`, and
 * `multi-earning-ledger.ts`. It was not removed because it broke. It was
 * removed because payout and settlement were not part of the accepted MVP, and
 * `INVARIANTS.md` still says any revival needs a fresh design that does not
 * quietly restore custody.
 *
 * One piece survived the purge, uncalled, at
 * `apps/pylon/src/coordinator/labor-job-state.ts`. Its transition table reads:
 *
 *     accept: { quoted: "accepted" }
 *     start:  { accepted: "in_progress" }
 *     deliver:{ in_progress: "delivered" }
 *     settle: { delivered: "settled" }
 *
 * `settle` is legal straight out of `delivered`, and `delivered` is set by the
 * provider. That machine pays a claim. The doctrine the Pylon arc actually
 * earned -- a lease is not an earning claim, only a settlement receipt is --
 * cannot be expressed in it.
 *
 * ## What this is
 *
 * The corrected gate, on the surviving protocol substrate. `packages/nip90`
 * kept and grew the NIP-LBR lane through the purge: `lbr.ts` (request, quote,
 * acceptance, result), `lbr-bond.ts`, and `lbr-closeout.ts`, whose
 * `LbrLaborCloseout` is a content-addressed, public-safe receipt binding one
 * complete labor lifecycle. That receipt -- not the provider's own submission
 * -- is the thing this module will pay against.
 *
 * The closeout is read structurally rather than imported: `@openagentsinc/cli`
 * ships as a plain-tsc npm package with a deliberately small dependency set,
 * and `@openagentsinc/nip90` pulls a git-tarball dependency that does not
 * belong in it. {@link LaborCloseoutReceipt} mirrors only the public-safe
 * fields the gate reads, and every one of them is a field `lbr-closeout.ts`
 * already produces.
 *
 * ## What this refuses to be
 *
 * It moves no money and holds no key. A settled decision is an accrual record:
 * `payout_rail` is `not_connected` and `custody` is `none`. Outbound payout
 * stays on the MDK/Nexus bridge, and settlement authority stays in the platform
 * receipt systems, exactly as `docs/nips/LBR.md` says. The number here is what
 * a verified job is owed, recorded so a ledger can accrue while the rail stays
 * a separate, explicit owner decision.
 *
 * Presence is not an input. `settleLease` takes a lease and a closeout receipt.
 * Being online, advertising capacity, or holding a live lease reaches neither
 * argument, so a provider that never earns a closeout earns zero no matter how
 * long it is up.
 */

/**
 * A lease: the buyer's grant of one job to one provider at one price.
 *
 * `price_msats` is what the job is worth once it verifies. Holding the lease
 * earns none of it. `provider` is the provider's public key; deriving it is the
 * identity seam's job, not this module's.
 */
export interface ProviderLease {
  readonly job_id: string;
  readonly lane: string;
  readonly provider: string;
  readonly price_msats: number;
  readonly expires_at: string;
}

/**
 * The public-safe fields of a NIP-LBR closeout receipt this gate reads.
 *
 * A structural mirror of `LbrLaborCloseout` from
 * `packages/nip90/src/lbr-closeout.ts`; the names are that module's names so a
 * receipt produced there is accepted here without translation.
 */
export interface LaborCloseoutReceipt {
  /** `lbr-closeout:<requestId>:<digest>`. */
  readonly receiptRef: string;
  readonly requestId: string;
  readonly requesterPubkey: string;
  readonly providerPubkey: string;
  readonly quotedAmountMsats: number;
  /** What was run to check the work. Empty means nothing checked it. */
  readonly verificationCommandRef: string;
  /** The evidence that check produced. */
  readonly testRef: string;
  /** The platform's own closeout. Settlement authority lives there, not here. */
  readonly platformCloseoutRef: string;
  /** SHA-256 over the canonical projection, making the receipt dereferenceable. */
  readonly digest: string;
  readonly settled_at: string;
}

/** Why a settlement did not happen. Each one is a distinct, nameable failure. */
export type SettlementRefusal =
  | "price_not_payable"
  | "no_closeout"
  | "closeout_job_mismatch"
  | "closeout_provider_mismatch"
  | "self_dealt"
  | "work_not_verified"
  | "no_settlement_authority"
  | "receipt_not_addressable"
  | "lease_expired"
  | "price_mismatch";

export interface SettlementDecision {
  readonly schema: "openagents.provider_settlement.v1";
  readonly job_id: string;
  readonly state: "settled" | "unsettled";
  /** What the verified job is owed. Zero on every path but a clean receipt. */
  readonly earned_msats: number;
  readonly reason: string;
  readonly refusal?: SettlementRefusal;
  /** No rail is wired. A settled decision accrues; it does not pay. */
  readonly payout_rail: "not_connected";
  readonly custody: "none";
  readonly receipt_ref?: string;
}

const DIGEST_PATTERN = /^[a-f0-9]{64}$/i;

const unsettled = (
  job_id: string,
  refusal: SettlementRefusal,
  reason: string,
): SettlementDecision => ({
  schema: "openagents.provider_settlement.v1",
  job_id,
  state: "unsettled",
  earned_msats: 0,
  reason,
  refusal,
  payout_rail: "not_connected",
  custody: "none",
});

const blank = (value: string | undefined): boolean =>
  typeof value !== "string" || value.trim().length === 0;

const parsedTime = (value: string): number | undefined => {
  const at = Date.parse(value);
  return Number.isNaN(at) ? undefined : at;
};

/**
 * Decide what one leased job earns.
 *
 * The gates run in the order a reader would check them by hand, so a refusal
 * names the first thing that is actually wrong rather than the last. Only the
 * final branch returns a non-zero amount, and reaching it needs a closeout
 * receipt that exists, names this job and this provider, was not issued by the
 * provider to itself, carries both a verification command and the evidence it
 * produced, carries the platform's own closeout, is content-addressable, landed
 * inside the lease window, and prices the job exactly as the lease did.
 */
export const settleLease = (
  lease: ProviderLease,
  closeout?: LaborCloseoutReceipt,
): SettlementDecision => {
  if (!Number.isFinite(lease.price_msats) || lease.price_msats <= 0) {
    return unsettled(
      lease.job_id,
      "price_not_payable",
      `The lease prices this job at ${lease.price_msats} msats, so there is nothing to settle.`,
    );
  }

  if (closeout === undefined) {
    return unsettled(
      lease.job_id,
      "no_closeout",
      "No closeout receipt covers this job. A lease is not an earning claim and a submission is not a receipt, so this earns nothing.",
    );
  }

  if (closeout.requestId !== lease.job_id) {
    return unsettled(
      lease.job_id,
      "closeout_job_mismatch",
      `The receipt closes out job ${closeout.requestId}, not the leased job ${lease.job_id}.`,
    );
  }

  if (closeout.providerPubkey !== lease.provider) {
    return unsettled(
      lease.job_id,
      "closeout_provider_mismatch",
      `The receipt credits provider ${closeout.providerPubkey}, but the lease is held by ${lease.provider}.`,
    );
  }

  if (closeout.requesterPubkey === closeout.providerPubkey) {
    return unsettled(
      lease.job_id,
      "self_dealt",
      "The receipt names the same key as requester and provider. A provider cannot buy its own work into an earning.",
    );
  }

  if (blank(closeout.verificationCommandRef) || blank(closeout.testRef)) {
    return unsettled(
      lease.job_id,
      "work_not_verified",
      "The receipt carries no verification command and evidence pair, so nothing checked this work. Unverified work earns nothing.",
    );
  }

  if (blank(closeout.platformCloseoutRef)) {
    return unsettled(
      lease.job_id,
      "no_settlement_authority",
      "The receipt carries no platform closeout ref. Settlement authority stays in the platform receipt systems; the relay is only transport.",
    );
  }

  if (!DIGEST_PATTERN.test(closeout.digest)) {
    return unsettled(
      lease.job_id,
      "receipt_not_addressable",
      "The receipt digest is not a 32-byte hex hash, so the receipt cannot be dereferenced and re-verified.",
    );
  }

  const expiresAt = parsedTime(lease.expires_at);
  const settledAt = parsedTime(closeout.settled_at);
  if (expiresAt === undefined || settledAt === undefined) {
    return unsettled(
      lease.job_id,
      "lease_expired",
      "The lease window could not be read, so the closeout cannot be placed inside it.",
    );
  }
  if (settledAt > expiresAt) {
    return unsettled(
      lease.job_id,
      "lease_expired",
      `The job closed out at ${closeout.settled_at}, after the lease expired at ${lease.expires_at}.`,
    );
  }

  if (closeout.quotedAmountMsats !== lease.price_msats) {
    return unsettled(
      lease.job_id,
      "price_mismatch",
      `The receipt quotes ${closeout.quotedAmountMsats} msats but the lease priced the job at ${lease.price_msats} msats.`,
    );
  }

  return {
    schema: "openagents.provider_settlement.v1",
    job_id: lease.job_id,
    state: "settled",
    earned_msats: closeout.quotedAmountMsats,
    reason: `Verified by ${closeout.verificationCommandRef} with evidence ${closeout.testRef}, closed out by ${closeout.platformCloseoutRef}. Accrued, not paid: no payout rail is connected.`,
    payout_rail: "not_connected",
    custody: "none",
    receipt_ref: closeout.receiptRef,
  };
};
