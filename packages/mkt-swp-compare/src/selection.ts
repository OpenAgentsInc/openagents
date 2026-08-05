/**
 * Order selection discipline (openagents#9318 §7, MKT-SWP §4.4).
 *
 * An Order commits the exact Quote event ID and may choose only from the
 * Quote's finite `selectable` lists. It must not restate a different price,
 * amount, asset, fee, route, or expiry and call it acceptance — the draft
 * type here makes restatement unrepresentable (the draft carries only the
 * quote event id and the bounded selection), and every out-of-list choice
 * is refused with `swp_order_selection_invalid` before anything is signed.
 *
 * Acceptance discipline (issue §2): a `firm` Quote DECLARES that a
 * conforming timely Order is accepted under stated preconditions — the
 * declaration is displayed as a declaration, not proven here. An
 * `indicative` Quote is accepted only by a provider `Status state=accepted`
 * signed by the quoting provider. Silence, relay acceptance, an invoice, or
 * an address is never acceptance.
 *
 * Behaviour contract (expiry half):
 * `openagents_web.swap_compare.quote_expiry_enforced.v1`.
 */
import type { SwpErrorIdentifier } from "@openagentsinc/swap-i18n";

import { quoteExpiryState } from "./expiry.js";
import { quoteConformance, type CompareQuote } from "./model.js";

/** The requester's bounded choices. Every field optional; omission inherits. */
export interface OrderSelectionInput {
  readonly inputAmountSats?: bigint;
  readonly feePayer?: string;
  readonly confirmationPolicy?: string;
  readonly publicReceiptConsent?: string;
}

/**
 * A refusal names exactly one §17 identifier plus which field failed, so
 * the surface can point at the offending control rather than a toast.
 */
export interface OrderSelectionRefusal {
  readonly ok: false;
  readonly error: SwpErrorIdentifier;
  readonly field: keyof OrderSelectionInput | "quote" | null;
  readonly detail: string;
}

/** An accepted draft: the exact Quote id plus only the bounded selection. */
export interface OrderDraft {
  readonly ok: true;
  readonly quoteEventId: string;
  readonly selection: OrderSelectionInput;
}

export type OrderSelectionResult = OrderDraft | OrderSelectionRefusal;

const refuse = (
  error: SwpErrorIdentifier,
  field: OrderSelectionRefusal["field"],
  detail: string,
): OrderSelectionRefusal => ({ ok: false, error, field, detail });

/**
 * Build an Order draft against one Quote, refusing locally (never relying
 * on a provider) when the Quote is expired, nonconforming, or the selection
 * steps outside the Quote's finite lists.
 */
export const selectOrder = (
  quote: CompareQuote,
  selection: OrderSelectionInput,
  nowSeconds: number,
): OrderSelectionResult => {
  const expiry = quoteExpiryState(quote, nowSeconds);
  if (expiry.state === "expired") {
    return refuse(
      "swp_quote_expired",
      "quote",
      `quote became unusable at ${expiry.expiredAtSeconds} (via ${expiry.via})`,
    );
  }

  const conformance = quoteConformance(quote);
  const firstIssue = conformance[0];
  if (firstIssue !== undefined) {
    return refuse(firstIssue.error, "quote", firstIssue.detail);
  }

  const selectable = quote.selectable;
  const selectedKeys = (
    Object.keys(selection) as (keyof OrderSelectionInput)[]
  ).filter(key => selection[key] !== undefined);
  if (selectable === null) {
    const firstSelected = selectedKeys[0];
    if (firstSelected !== undefined) {
      return refuse(
        "swp_order_selection_invalid",
        firstSelected,
        "quote permits no Order selection",
      );
    }
    return { ok: true, quoteEventId: quote.eventId, selection: {} };
  }

  if (selection.inputAmountSats !== undefined) {
    const range = selectable.inputAmountRangeSats;
    if (range === null) {
      return refuse(
        "swp_order_selection_invalid",
        "inputAmountSats",
        "quote offers no input-amount range",
      );
    }
    if (
      selection.inputAmountSats < range.minSats ||
      selection.inputAmountSats > range.maxSats
    ) {
      return refuse(
        "swp_order_selection_invalid",
        "inputAmountSats",
        "input amount outside the range the quote offers",
      );
    }
  }
  if (
    selection.feePayer !== undefined &&
    !selectable.feePayers.includes(selection.feePayer)
  ) {
    return refuse(
      "swp_order_selection_invalid",
      "feePayer",
      "fee payer is not in the quote's finite list",
    );
  }
  if (
    selection.confirmationPolicy !== undefined &&
    !selectable.confirmationPolicies.includes(selection.confirmationPolicy)
  ) {
    return refuse(
      "swp_order_selection_invalid",
      "confirmationPolicy",
      "confirmation policy is not in the quote's finite list",
    );
  }
  if (
    selection.publicReceiptConsent !== undefined &&
    !selectable.publicReceiptConsent.includes(selection.publicReceiptConsent)
  ) {
    return refuse(
      "swp_order_selection_invalid",
      "publicReceiptConsent",
      "receipt consent is broader than the quote permits",
    );
  }

  return { ok: true, quoteEventId: quote.eventId, selection };
};

/**
 * Evidence that could be mistaken for acceptance of an Order against an
 * indicative Quote. Only one member is acceptance; the others exist so the
 * refusal can name what was observed.
 */
export type AcceptanceEvidence =
  | {
      readonly kind: "provider_status";
      /** Status `state` value as signed. */
      readonly state: string;
      readonly statusEventId: string;
      readonly signerPubkey: string;
    }
  | { readonly kind: "relay_ok" }
  | { readonly kind: "invoice_received" }
  | { readonly kind: "address_received" }
  | { readonly kind: "silence" };

export type AcceptanceState =
  | {
      readonly accepted: true;
      /** Firm: the Quote's own declaration. Indicative: the Status event. */
      readonly via: "firm_quote_declaration" | "provider_status_accepted";
      readonly statusEventId: string | null;
    }
  | {
      readonly accepted: false;
      readonly reason:
        | "awaiting_provider_status"
        | "evidence_is_not_acceptance"
        | "status_signer_mismatch"
        | "status_state_not_accepted";
    };

/**
 * Is this Order accepted, given the Quote's class and the observed
 * evidence? For a firm Quote a conforming timely Order is accepted by the
 * Quote's own declaration (which the surface renders AS a declaration —
 * it proves neither the declaration nor capacity). For an indicative Quote
 * only a provider `Status state=accepted` signed by the quoting provider
 * counts; every other observation is typed non-acceptance.
 */
export const orderAcceptance = (
  quote: CompareQuote,
  evidence: AcceptanceEvidence | null,
): AcceptanceState => {
  if (quote.quoteClass === "firm") {
    return {
      accepted: true,
      via: "firm_quote_declaration",
      statusEventId: null,
    };
  }
  if (evidence === null) {
    return { accepted: false, reason: "awaiting_provider_status" };
  }
  if (evidence.kind !== "provider_status") {
    return { accepted: false, reason: "evidence_is_not_acceptance" };
  }
  if (evidence.signerPubkey !== quote.providerPubkey) {
    return { accepted: false, reason: "status_signer_mismatch" };
  }
  if (evidence.state !== "accepted") {
    return { accepted: false, reason: "status_state_not_accepted" };
  }
  return {
    accepted: true,
    via: "provider_status_accepted",
    statusEventId: evidence.statusEventId,
  };
};
