/**
 * Render-ready view model for the `DestinationField` (rollout plan §3.2).
 * The SWAP-0 shell (and any surface rendering the exported session
 * view-model) maps this straight to markup; no presentation decisions are
 * left implicit in the reducer state.
 */
import {
  concreteInvoice,
  effectiveAmountMsat,
  fundingGate,
  type DestinationEntryState,
} from "./entry.js";
import {
  DESTINATION_FAILURE_MESSAGES,
  ENTRY_NOTICE_MESSAGES,
  SWP_DESTINATION_ERROR_MESSAGES,
  type DestinationMessage,
} from "./messages.js";
import type { QrAvailability } from "./qr.js";

export interface InvoiceSummaryView {
  readonly amountSats: bigint;
  readonly expiresAtSeconds: number;
  readonly expiresInSeconds: number;
  readonly routeHintCount: number;
}

export interface DeferredSummaryView {
  readonly kind:
    | "deferred_lnurl"
    | "deferred_lightning_address"
    | "deferred_bolt12_offer";
  readonly display: string;
}

export type DestinationValidityView =
  | { readonly state: "empty" }
  | {
      readonly state: "invalid";
      readonly mode: string;
      readonly message: DestinationMessage;
      /** MKT-SWP §17 identifier when the refusal carries one. */
      readonly swpError: string | null;
    }
  | { readonly state: "valid" }
  | { readonly state: "verifying" }
  | {
      readonly state: "verification_failed";
      readonly swpError: string;
      readonly message: DestinationMessage;
    };

export interface DestinationFieldView {
  /** Which field the direction calls for (teardown §3.2: never a label). */
  readonly fieldKind: "address" | "invoice";
  readonly text: string;
  readonly validity: DestinationValidityView;
  readonly invoice: InvoiceSummaryView | null;
  readonly deferred: DeferredSummaryView | null;
  readonly effectiveAmountMsat: bigint | null;
  readonly notices: readonly DestinationMessage[];
  readonly qrScanVisible: boolean;
  /** Necessary-condition gate only; the engine owns verify-before-fund. */
  readonly fundingGate: "blocked" | "eligible";
}

const validityOf = (
  state: DestinationEntryState,
  nowSeconds: number,
): DestinationValidityView => {
  if (state.failure !== null) {
    if (state.failure.mode === "empty") return { state: "empty" };
    return {
      state: "invalid",
      mode: state.failure.mode,
      message: DESTINATION_FAILURE_MESSAGES[state.failure.mode],
      swpError: state.failure.swpError,
    };
  }
  if (state.bound === null) return { state: "empty" };
  // An invoice that expired while the field sat open is invalid now, not
  // valid-until-reparse (audit gap 4). The gate blocks on the same clock.
  const invoice = concreteInvoice(state);
  if (invoice !== null && invoice.expiresAtSeconds <= nowSeconds) {
    return {
      state: "invalid",
      mode: "invoice_expired",
      message: DESTINATION_FAILURE_MESSAGES.invoice_expired,
      swpError: null,
    };
  }
  if (state.verification.status === "failed") {
    const error = state.verification.verdict.error;
    return {
      state: "verification_failed",
      swpError: error,
      message: SWP_DESTINATION_ERROR_MESSAGES[error],
    };
  }
  if (
    state.bound.kind === "onchain_address" ||
    state.bound.kind === "bolt11_invoice"
  ) {
    // A concrete destination renders `valid` only when its current-epoch
    // verdict is a pass. Anything else — pending, or a verdict dropped by
    // the staleness guard — is still being verified; the field never
    // renders `valid` while the gate is blocked with the verdict missing
    // (audit gap 3).
    return state.verification.status === "verified" &&
      state.verification.epoch === state.epoch
      ? { state: "valid" }
      : { state: "verifying" };
  }
  // Deferred destination: parse-valid until resolution starts; once it
  // resolves, the resolved invoice's own verification decides.
  if (state.resolution.status === "pending") return { state: "verifying" };
  if (state.resolution.status === "resolved") {
    return state.verification.status === "verified" &&
      state.verification.epoch === state.epoch
      ? { state: "valid" }
      : { state: "verifying" };
  }
  return { state: "valid" };
};

export const destinationFieldView = (
  state: DestinationEntryState,
  qrAvailability: QrAvailability,
  nowSeconds: number,
): DestinationFieldView => {
  const bound = state.bound;
  const invoice =
    bound?.kind === "bolt11_invoice"
      ? {
          amountSats: bound.amountMsat / 1000n,
          expiresAtSeconds: bound.expiresAtSeconds,
          expiresInSeconds: Math.max(0, bound.expiresAtSeconds - nowSeconds),
          routeHintCount: bound.routeHintCount,
        }
      : null;
  const deferred =
    bound !== null &&
    (bound.kind === "deferred_lnurl" ||
      bound.kind === "deferred_lightning_address" ||
      bound.kind === "deferred_bolt12_offer")
      ? {
          kind: bound.kind,
          display:
            bound.kind === "deferred_lightning_address"
              ? bound.address
              : bound.kind === "deferred_lnurl"
                ? bound.url
                : bound.offer,
        }
      : null;

  return {
    fieldKind: state.rail === "chain" ? "address" : "invoice",
    text: state.text,
    validity: validityOf(state, nowSeconds),
    invoice,
    deferred,
    effectiveAmountMsat: effectiveAmountMsat(state),
    notices: state.notices.map((n) => ENTRY_NOTICE_MESSAGES[n.notice]),
    qrScanVisible: qrAvailability === "available",
    fundingGate: fundingGate(state, nowSeconds),
  };
};
