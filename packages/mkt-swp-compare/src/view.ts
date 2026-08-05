/**
 * Render-ready view models for the comparison table and the
 * verify-before-fund checklist (openagents#9318 §1, §6).
 *
 * The SWAP-0 shell maps these straight to markup. Presentation decisions
 * that carry protocol meaning are made HERE and tested:
 *
 * - each (quote class, reservation class) commitment gets a distinct badge
 *   token, so a firm/hard row cannot be styled identically to an
 *   indicative/none row (the token is the render key that picks the style);
 * - each reservation proof class gets its own label;
 * - the custody strip carries all six §6 dimensions and BOTH duration
 *   bounds — never one collapsed score;
 * - expired and nonconforming quotes are retained in the table but marked
 *   unusable (`selectable: false`), and reservation forks stay visible and
 *   attributable on every implicated row.
 *
 * Behaviour contracts:
 * `openagents_web.swap_compare.firm_indicative_distinct.v1`,
 * `openagents_web.swap_compare.reservation_proof_class_distinct.v1`.
 */
import { custodyStrip, type CustodyStrip } from "./custody.js";
import { quoteExpiryState, type QuoteExpiryState } from "./expiry.js";
import {
  COMMITMENT_MESSAGES,
  CUSTODY_STRIP_MESSAGES,
  PROOF_CLASS_MESSAGES,
  QUOTE_EXPIRED_MESSAGE,
  QUOTE_NONCONFORMING_MESSAGE,
  RESERVATION_FORK_MESSAGE,
  VERIFY_CHECK_MESSAGES,
  type CompareMessage,
  type CommitmentKey,
} from "./messages.js";
import {
  quoteConformance,
  type CompareQuote,
  type CustodyDisclosure,
  type FeeLine,
  type QuoteConformanceIssue,
} from "./model.js";
import { rankQuotes } from "./ranking.js";
import {
  forkedQuoteEventIds,
  type ReservationFork,
} from "./reservation.js";
import {
  fundingGate,
  VERIFY_CHECK_IDS,
  type FundingGate,
  type VerifyBeforeFundReport,
  type VerifyCheckId,
} from "./verify.js";

/** The commitment badge: distinct token per commitment, never flattened. */
export interface CommitmentBadgeView {
  readonly quoteClass: CompareQuote["quoteClass"];
  readonly reservationClass: CompareQuote["reservationClass"];
  /**
   * The render key that selects the badge style. Distinct for every
   * (quoteClass, reservationClass) pair by construction.
   */
  readonly token: CommitmentKey;
  readonly label: CompareMessage;
  /** Proof-class line, present exactly when the Quote reserves capacity. */
  readonly proof: {
    readonly proofClass: NonNullable<CompareQuote["reservation"]>["proofClass"];
    readonly label: CompareMessage;
  } | null;
}

/** The custody strip: class + all six dimensions + both bounds. */
export interface CustodyStripView {
  readonly strip: CustodyStrip;
  readonly label: CompareMessage;
  /** The full §6 disclosure — the table renders dimensions, not a score. */
  readonly disclosure: CustodyDisclosure;
  /** Wall-clock ESTIMATE (never presented as consensus authority). */
  readonly maximumCustodyDurationSeconds: number;
  /** The exact height-based bound, displayed alongside the estimate. */
  readonly maximumCustodyHeightBound: bigint;
}

export type RowUsability =
  | { readonly usable: true }
  | {
      readonly usable: false;
      readonly reason: "expired" | "nonconforming" | "reservation_fork";
      readonly notice: CompareMessage;
    };

export interface CompareRowView {
  readonly eventId: string;
  readonly provider: {
    readonly pubkey: string;
    /** Freshness: seconds since the Quote was signed. */
    readonly ageSeconds: number;
  };
  readonly outputAmountSats: bigint;
  readonly inputAmountSats: bigint;
  readonly fees: readonly FeeLine[];
  readonly commitment: CommitmentBadgeView;
  readonly custody: CustodyStripView;
  readonly expiry: QuoteExpiryState;
  readonly conformance: readonly QuoteConformanceIssue[];
  /** Forks implicating this Quote — retained and attributable. */
  readonly forks: readonly ReservationFork[];
  readonly usability: RowUsability;
}

export interface CompareTableView {
  /** Ranked best-execution first under the documented ranking rule. */
  readonly rows: readonly CompareRowView[];
  readonly nowSeconds: number;
}

export const commitmentBadge = (quote: CompareQuote): CommitmentBadgeView => {
  const token: CommitmentKey = `${quote.quoteClass}:${quote.reservationClass}`;
  return {
    quoteClass: quote.quoteClass,
    reservationClass: quote.reservationClass,
    token,
    label: COMMITMENT_MESSAGES[token],
    proof:
      quote.reservation === null
        ? null
        : {
            proofClass: quote.reservation.proofClass,
            label: PROOF_CLASS_MESSAGES[quote.reservation.proofClass],
          },
  };
};

export const custodyStripView = (
  custody: CustodyDisclosure,
): CustodyStripView => {
  const strip = custodyStrip(custody);
  return {
    strip,
    label: CUSTODY_STRIP_MESSAGES[strip.stripClass],
    disclosure: custody,
    maximumCustodyDurationSeconds: custody.maximumCustodyDurationSeconds,
    maximumCustodyHeightBound: custody.maximumCustodyHeightBound,
  };
};

const rowUsability = (
  expiry: QuoteExpiryState,
  conformance: readonly QuoteConformanceIssue[],
  forked: boolean,
): RowUsability => {
  if (expiry.state === "expired") {
    return { usable: false, reason: "expired", notice: QUOTE_EXPIRED_MESSAGE };
  }
  if (conformance.length > 0) {
    return {
      usable: false,
      reason: "nonconforming",
      notice: QUOTE_NONCONFORMING_MESSAGE,
    };
  }
  if (forked) {
    return {
      usable: false,
      reason: "reservation_fork",
      notice: RESERVATION_FORK_MESSAGE,
    };
  }
  return { usable: true };
};

/**
 * Build the comparison table for one RFQ's competing Quotes. Expired,
 * nonconforming, and forked Quotes are retained (evidence stays visible)
 * but marked unusable; the fund/order path never sees them because
 * `selection.ts` refuses them independently — the view flag is UX, the
 * refusal is the enforcement.
 */
export const compareTableView = (
  quotes: readonly CompareQuote[],
  options: {
    readonly nowSeconds: number;
    readonly forks?: readonly ReservationFork[];
  },
): CompareTableView => {
  const forks = options.forks ?? [];
  const forked = forkedQuoteEventIds(forks);
  const rows = rankQuotes(quotes, options.nowSeconds).map(
    (quote): CompareRowView => {
      const expiry = quoteExpiryState(quote, options.nowSeconds);
      const conformance = quoteConformance(quote);
      const rowForks = forks.filter(fork =>
        fork.memberQuoteEventIds.includes(quote.eventId),
      );
      return {
        eventId: quote.eventId,
        provider: {
          pubkey: quote.providerPubkey,
          ageSeconds: Math.max(0, options.nowSeconds - quote.createdAtSeconds),
        },
        outputAmountSats: quote.outputAmountSats,
        inputAmountSats: quote.inputAmountSats,
        fees: quote.fees,
        commitment: commitmentBadge(quote),
        custody: custodyStripView(quote.custody),
        expiry,
        conformance,
        forks: rowForks,
        usability: rowUsability(expiry, conformance, forked.has(quote.eventId)),
      };
    },
  );
  return { rows, nowSeconds: options.nowSeconds };
};

/** One rendered verify-checklist row: status + label + typed identifier. */
export interface VerifyChecklistRowView {
  readonly id: VerifyCheckId;
  readonly status: "unresolved" | "pass" | "fail";
  readonly label: CompareMessage;
  /** §17 identifier when failed; the row names its failure, never generic. */
  readonly error: string | null;
}

export interface VerifyChecklistView {
  readonly rows: readonly VerifyChecklistRowView[];
  /** The gate in front of the fund action — see `fundingGate`. */
  readonly gate: FundingGate;
}

/**
 * Render the engine's verify-before-fund report as explicit pass/fail rows
 * with the funding gate derived fail-closed. A null report renders every
 * row unresolved and the gate disabled.
 */
export const verifyChecklistView = (
  report: VerifyBeforeFundReport | null,
  currentEpoch: number,
): VerifyChecklistView => {
  const byId = new Map(
    (report?.rows ?? []).map(row => [row.id, row] as const),
  );
  const rows = VERIFY_CHECK_IDS.map((id): VerifyChecklistRowView => {
    const row = byId.get(id);
    if (row === undefined || (report !== null && report.epoch !== currentEpoch)) {
      return {
        id,
        status: "unresolved",
        label: VERIFY_CHECK_MESSAGES[id],
        error: null,
      };
    }
    return {
      id,
      status: row.status,
      label: VERIFY_CHECK_MESSAGES[id],
      error: row.status === "fail" ? row.error : null,
    };
  });
  return { rows, gate: fundingGate(report, currentEpoch) };
};
