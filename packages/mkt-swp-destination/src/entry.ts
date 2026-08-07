/**
 * The headless destination-entry state machine behind `DestinationField`
 * (issue #9317 §1, §3, §6). The SWAP-0 widget shell mounts this as a pure
 * reducer: events in, `{ state, intents }` out, no DOM and no IO.
 *
 * Laws enforced here (behaviour contracts, see the market-swap-destination
 * registry in `@openagentsinc/behavior-contracts`):
 *
 * - `openagents_web.swap_destination.amountless_invoice_refused.v1`
 *   An amountless invoice never binds; it is refused with the typed
 *   identifier `swp_invoice_invalid`.
 * - `openagents_web.swap_destination.invoice_amount_precedence.v1`
 *   An amount-locked invoice overrides the typed amount, and a unified QR
 *   carrying both a BIP-21 amount and an amount-bearing invoice yields
 *   exactly one effective amount.
 * - `openagents_web.swap_destination.amount_edit_clears_invoice.v1`
 *   Editing the amount after a concrete invoice was entered clears that
 *   invoice with an explicit notice; deferred destinations survive.
 *
 * Every async step (engine verdicts, deferred resolution) carries the
 * entry epoch it was computed for and is dropped when superseded, so a
 * stale result can never overwrite a newer field value.
 *
 * Verification always refers to a concrete artifact. A deferred
 * destination (LNURL, lightning address, BOLT12 offer) has nothing the
 * verifier port can check, so binding one requests no verification and the
 * funding gate stays blocked; when it resolves, the reducer requests
 * verification for the *resolved invoice* — the artifact the payment
 * actually goes to — and the gate opens only on that verdict (acceptance
 * audit gap 1 on issue #9317).
 */
import type {
  BitcoinNetwork,
  Bolt11InvoiceDestination,
  DeferredDestination,
  DestinationParseFailure,
  DestinationRail,
  OnchainAddressDestination,
  ParsedDestination,
} from "./model.js";
import { parseDestination } from "./parse.js";
import type { ResolutionOutcome } from "./resolve.js";
import type { DestinationVerdict } from "./verify.js";

const MSAT_PER_SAT = 1000n;

/** A destination bound to the field: always concrete about its rail. */
export type BoundDestination =
  | OnchainAddressDestination
  | Bolt11InvoiceDestination
  | DeferredDestination;

/**
 * What the verifier port can actually verify: script bytes for an address,
 * the complete invoice string for an invoice. A deferred destination is
 * never verifiable — its resolved invoice is.
 */
export type VerifiableDestination =
  | OnchainAddressDestination
  | Bolt11InvoiceDestination;

const isVerifiable = (
  destination: BoundDestination,
): destination is VerifiableDestination =>
  destination.kind === "onchain_address" ||
  destination.kind === "bolt11_invoice";

export type EntryNotice =
  | {
      readonly notice: "route_switched";
      readonly fromRail: DestinationRail;
      readonly toRail: DestinationRail;
    }
  | { readonly notice: "invoice_cleared_by_amount_edit" }
  | { readonly notice: "bip21_amount_suppressed" }
  | { readonly notice: "typed_amount_overridden_by_invoice" };

export const ENTRY_NOTICE_KINDS = [
  "route_switched",
  "invoice_cleared_by_amount_edit",
  "bip21_amount_suppressed",
  "typed_amount_overridden_by_invoice",
] as const;

export type VerificationState =
  | { readonly status: "idle" }
  | { readonly status: "pending"; readonly epoch: number }
  | { readonly status: "verified"; readonly epoch: number }
  | {
      readonly status: "failed";
      readonly epoch: number;
      readonly verdict: Extract<DestinationVerdict, { verdict: "fail" }>;
    };

export type ResolutionState =
  | { readonly status: "none" }
  | { readonly status: "pending"; readonly epoch: number }
  | {
      readonly status: "resolved";
      readonly epoch: number;
      readonly invoice: Bolt11InvoiceDestination;
    }
  | {
      readonly status: "failed";
      readonly epoch: number;
      readonly outcome: Exclude<ResolutionOutcome, { outcome: "resolved" }>;
    };

export type AmountSource = "typed" | "invoice" | "uri";

export interface DestinationEntryState {
  /** Monotonic staleness guard; bumped by every input-changing event. */
  readonly epoch: number;
  readonly rail: DestinationRail;
  readonly network: BitcoinNetwork;
  /** Verbatim field text. */
  readonly text: string;
  readonly bound: BoundDestination | null;
  readonly failure: DestinationParseFailure | null;
  readonly typedAmountMsat: bigint | null;
  readonly amountSource: AmountSource;
  /** Transient notices for this transition (toast/inline surface). */
  readonly notices: readonly EntryNotice[];
  readonly verification: VerificationState;
  readonly resolution: ResolutionState;
}

export type EntryEvent =
  | {
      readonly type: "input";
      readonly source: "typing" | "paste" | "qr";
      readonly text: string;
      /** Injected clock, seconds since epoch. */
      readonly nowSeconds: number;
    }
  | { readonly type: "amount_edited"; readonly amountMsat: bigint | null }
  | {
      readonly type: "rail_changed";
      readonly rail: DestinationRail;
      readonly network: BitcoinNetwork;
      readonly nowSeconds: number;
    }
  | { readonly type: "clear" }
  | {
      readonly type: "verdict";
      readonly epoch: number;
      readonly verdict: DestinationVerdict;
    }
  | { readonly type: "resolution_started"; readonly epoch: number }
  | {
      readonly type: "resolution";
      readonly epoch: number;
      readonly outcome: ResolutionOutcome;
    };

export type EntryIntent =
  | { readonly intent: "switch_rail"; readonly rail: DestinationRail }
  | {
      readonly intent: "request_verification";
      readonly epoch: number;
      /**
       * Always a concrete artifact the verifier port has an operation for.
       * For a deferred destination this is the invoice it resolved to.
       */
      readonly destination: VerifiableDestination;
      /**
       * The session's expected payment hash (the comparand the verifier
       * port declares), threaded from `EntryConfig` so it has a producer
       * inside the reducer's contract. `null` when the session has not
       * bound one yet — the engine then verifies everything except the
       * hash correlation.
       */
      readonly expectedPaymentHashHex: string | null;
    };

export interface EntryConfig {
  /**
   * SWAP-1 reachability: whether any live Offering serves the direction a
   * pasted destination would switch to. Route switching is refused (with
   * `route_unreachable`) when this returns false.
   */
  readonly isRailReachable: (rail: DestinationRail) => boolean;
  /**
   * The swap session's expected payment hash, when the session has bound
   * one (MKT-SWP §7.2). The shell supplies it; every
   * `request_verification` intent carries it to the verifier port.
   */
  readonly expectedPaymentHashHex?: string | null;
}

export interface EntryTransition {
  readonly state: DestinationEntryState;
  readonly intents: readonly EntryIntent[];
}

export const initialEntryState = (
  rail: DestinationRail,
  network: BitcoinNetwork,
): DestinationEntryState => ({
  epoch: 0,
  rail,
  network,
  text: "",
  bound: null,
  failure: null,
  typedAmountMsat: null,
  amountSource: "typed",
  notices: [],
  verification: { status: "idle" },
  resolution: { status: "none" },
});

/**
 * The single effective amount, in millisatoshis (issue #9317 §6). An
 * amount-locked invoice (typed directly, via unified QR, or via deferred
 * resolution) always wins over the typed amount.
 */
export const effectiveAmountMsat = (
  state: DestinationEntryState,
): bigint | null => {
  if (state.bound?.kind === "bolt11_invoice") return state.bound.amountMsat;
  if (state.resolution.status === "resolved") {
    return state.resolution.invoice.amountMsat;
  }
  return state.typedAmountMsat;
};

/**
 * The concrete invoice funding would pay, if any: the bound invoice, or
 * the invoice a deferred destination resolved to at the current epoch.
 */
export const concreteInvoice = (
  state: DestinationEntryState,
): Bolt11InvoiceDestination | null => {
  if (state.bound?.kind === "bolt11_invoice") return state.bound;
  if (
    state.resolution.status === "resolved" &&
    state.resolution.epoch === state.epoch
  ) {
    return state.resolution.invoice;
  }
  return null;
};

/**
 * Funding eligibility as this component can see it: a destination is bound,
 * the engine verdict for the *current* epoch is a pass, that verdict covers
 * a concrete artifact (a deferred destination is eligible only once its
 * resolved invoice has its own current-epoch pass), and no concrete invoice
 * has expired against the injected clock. This is a necessary condition
 * only — the full verify-before-fund checklist lives behind the engine
 * boundary (SWAP-0/SWAP-3) and this surface never substitutes for it.
 */
export const fundingGate = (
  state: DestinationEntryState,
  nowSeconds: number,
): "blocked" | "eligible" => {
  if (state.bound === null) return "blocked";
  if (
    state.verification.status !== "verified" ||
    state.verification.epoch !== state.epoch
  ) {
    return "blocked";
  }
  const invoice = concreteInvoice(state);
  if (!isVerifiable(state.bound) && invoice === null) {
    // Deferred destination not yet resolved: the pass, however it landed,
    // cannot cover the artifact the payment goes to.
    return "blocked";
  }
  if (invoice !== null && invoice.expiresAtSeconds <= nowSeconds) {
    // An invoice that expired while the field sat open stops being
    // fundable, without waiting for a re-parse (audit gap 4).
    return "blocked";
  }
  return "eligible";
};

interface Candidate {
  readonly destination: BoundDestination;
  readonly rail: DestinationRail;
  readonly bip21AmountSuppressed: boolean;
}

/** Choose the concrete leg of a parse result for the field's rail. */
const chooseCandidate = (
  destination: ParsedDestination,
  rail: DestinationRail,
): Candidate => {
  if (destination.kind !== "unified") {
    return {
      destination,
      rail: destination.rail,
      bip21AmountSuppressed: false,
    };
  }
  if (rail === "lightning" && destination.lightning !== null) {
    return {
      destination: destination.lightning,
      rail: "lightning",
      bip21AmountSuppressed: destination.bip21AmountSuppressed,
    };
  }
  return {
    destination: destination.onchain,
    rail: "chain",
    bip21AmountSuppressed: false,
  };
};

const failedTransition = (
  state: DestinationEntryState,
  text: string,
  failure: DestinationParseFailure,
): EntryTransition => ({
  state: {
    ...state,
    epoch: state.epoch + 1,
    text,
    bound: null,
    failure,
    notices: [],
    verification: { status: "idle" },
    resolution: { status: "none" },
  },
  intents: [],
});

const requestVerification = (
  epoch: number,
  destination: VerifiableDestination,
  config: EntryConfig,
): EntryIntent => ({
  intent: "request_verification",
  epoch,
  destination,
  expectedPaymentHashHex: config.expectedPaymentHashHex ?? null,
});

const bindCandidate = (
  state: DestinationEntryState,
  text: string,
  candidate: Candidate,
  switched: boolean,
  config: EntryConfig,
): EntryTransition => {
  const epoch = state.epoch + 1;
  const notices: EntryNotice[] = [];
  if (switched) {
    notices.push({
      notice: "route_switched",
      fromRail: state.rail,
      toRail: candidate.rail,
    });
  }
  if (candidate.bip21AmountSuppressed) {
    notices.push({ notice: "bip21_amount_suppressed" });
  }

  let typedAmountMsat = state.typedAmountMsat;
  let amountSource = state.amountSource;
  if (candidate.destination.kind === "bolt11_invoice") {
    if (
      state.typedAmountMsat !== null &&
      state.typedAmountMsat !== candidate.destination.amountMsat
    ) {
      notices.push({ notice: "typed_amount_overridden_by_invoice" });
    }
    amountSource = "invoice";
  } else if (
    candidate.destination.kind === "onchain_address" &&
    candidate.destination.bip21 !== null &&
    candidate.destination.bip21.amountSats !== null
  ) {
    typedAmountMsat = candidate.destination.bip21.amountSats * MSAT_PER_SAT;
    amountSource = "uri";
  }

  // Only a concrete artifact can be verified; a deferred destination's
  // verification is requested when its resolved invoice arrives.
  const verifiable = isVerifiable(candidate.destination);
  const nextState: DestinationEntryState = {
    ...state,
    epoch,
    rail: candidate.rail,
    text,
    bound: candidate.destination,
    failure: null,
    typedAmountMsat,
    amountSource,
    notices,
    verification: verifiable
      ? { status: "pending", epoch }
      : { status: "idle" },
    resolution: { status: "none" },
  };
  const intents: EntryIntent[] = [
    ...(switched
      ? [{ intent: "switch_rail", rail: candidate.rail } as const]
      : []),
    ...(isVerifiable(candidate.destination)
      ? [requestVerification(epoch, candidate.destination, config)]
      : []),
  ];
  return { state: nextState, intents };
};

const reduceInput = (
  state: DestinationEntryState,
  config: EntryConfig,
  text: string,
  nowSeconds: number,
): EntryTransition => {
  const result = parseDestination(text, {
    rail: state.rail,
    network: state.network,
    nowSeconds,
  });
  if (!result.ok) return failedTransition(state, text, result.failure);

  const candidate = chooseCandidate(result.destination, state.rail);
  if (candidate.rail === state.rail) {
    return bindCandidate(state, text, candidate, false, config);
  }
  // Paste-driven route switching (issue #9317 §3): reconfigure the
  // direction rather than refuse, subject to SWAP-1 reachability.
  if (!config.isRailReachable(candidate.rail)) {
    return failedTransition(state, text, {
      mode: "route_unreachable",
      swpError: null,
      requiredRail: candidate.rail,
    });
  }
  return bindCandidate(state, text, candidate, true, config);
};

export const reduceEntryEvent = (
  state: DestinationEntryState,
  event: EntryEvent,
  config: EntryConfig,
): EntryTransition => {
  switch (event.type) {
    case "input":
      return reduceInput(state, config, event.text, event.nowSeconds);

    case "amount_edited": {
      const epoch = state.epoch + 1;
      if (state.bound?.kind === "bolt11_invoice") {
        // A concrete invoice's amount is locked; editing the amount clears
        // the invoice with an explicit notice (issue #9317 §6).
        return {
          state: {
            ...state,
            epoch,
            text: "",
            bound: null,
            failure: null,
            typedAmountMsat: event.amountMsat,
            amountSource: "typed",
            notices: [{ notice: "invoice_cleared_by_amount_edit" }],
            verification: { status: "idle" },
            resolution: { status: "none" },
          },
          intents: [],
        };
      }
      if (state.bound !== null && isVerifiable(state.bound)) {
        // The epoch bump invalidates the old verdict; re-request one so an
        // amount edit never strands the gate blocked with nothing in
        // flight (audit gap 3).
        return {
          state: {
            ...state,
            epoch,
            typedAmountMsat: event.amountMsat,
            amountSource: "typed",
            notices: [],
            verification: { status: "pending", epoch },
            resolution: { status: "none" },
          },
          intents: [requestVerification(epoch, state.bound, config)],
        };
      }
      // Deferred destinations bind their amount at order time and survive;
      // any pending resolution (and the verification of a resolved
      // invoice) belongs to the superseded amount and is discarded.
      return {
        state: {
          ...state,
          epoch,
          typedAmountMsat: event.amountMsat,
          amountSource: "typed",
          notices: [],
          verification: { status: "idle" },
          resolution: { status: "none" },
        },
        intents: [],
      };
    }

    case "rail_changed": {
      const cleared: DestinationEntryState = {
        ...state,
        epoch: state.epoch + 1,
        rail: event.rail,
        network: event.network,
        text: "",
        bound: null,
        failure: null,
        notices: [],
        verification: { status: "idle" },
        resolution: { status: "none" },
      };
      if (state.text === "") return { state: cleared, intents: [] };
      // Re-parse the existing text under the new direction; keep it only
      // when it still fits (Boltz preserves a still-valid on-chain address
      // across asset changes and clears everything else).
      const result = parseDestination(state.text, {
        rail: event.rail,
        network: event.network,
        nowSeconds: event.nowSeconds,
      });
      if (!result.ok) return { state: cleared, intents: [] };
      const candidate = chooseCandidate(result.destination, event.rail);
      if (candidate.rail !== event.rail) return { state: cleared, intents: [] };
      return bindCandidate(cleared, state.text, candidate, false, config);
    }

    case "clear":
      return {
        state: {
          ...initialEntryState(state.rail, state.network),
          epoch: state.epoch + 1,
          typedAmountMsat: state.typedAmountMsat,
        },
        intents: [],
      };

    case "verdict": {
      // Staleness guard: a superseded verdict never lands (issue #9317 §3).
      if (event.epoch !== state.epoch) return { state, intents: [] };
      // A verdict only lands against an outstanding request: an unrequested
      // pass (e.g. one computed for a deferred destination rather than a
      // concrete artifact) can never set `verified`.
      if (
        state.verification.status !== "pending" ||
        state.verification.epoch !== event.epoch
      ) {
        return { state, intents: [] };
      }
      if (event.verdict.verdict === "pass") {
        return {
          state: {
            ...state,
            verification: { status: "verified", epoch: event.epoch },
          },
          intents: [],
        };
      }
      return {
        state: {
          ...state,
          verification: {
            status: "failed",
            epoch: event.epoch,
            verdict: event.verdict,
          },
        },
        intents: [],
      };
    }

    case "resolution_started": {
      if (event.epoch !== state.epoch) return { state, intents: [] };
      return {
        state: { ...state, resolution: { status: "pending", epoch: event.epoch } },
        intents: [],
      };
    }

    case "resolution": {
      // Staleness guard: resolution for superseded input is dropped.
      if (event.epoch !== state.epoch) return { state, intents: [] };
      if (event.outcome.outcome === "resolved") {
        // The resolved invoice is the artifact the payment actually goes
        // to. Any verdict obtained so far did not cover it, so
        // verification restarts here: the gate cannot open until the
        // engine passes *this invoice* (audit gap 1).
        return {
          state: {
            ...state,
            resolution: {
              status: "resolved",
              epoch: event.epoch,
              invoice: event.outcome.invoice,
            },
            verification: { status: "pending", epoch: event.epoch },
          },
          intents: [
            requestVerification(event.epoch, event.outcome.invoice, config),
          ],
        };
      }
      return {
        state: {
          ...state,
          resolution: {
            status: "failed",
            epoch: event.epoch,
            outcome: event.outcome,
          },
        },
        intents: [],
      };
    }
  }
};
