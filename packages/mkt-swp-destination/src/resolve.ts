/**
 * Deferred-destination resolution (issue #9317 §7): LNURL-pay endpoints,
 * lightning addresses, and BOLT12 offers resolve to a concrete invoice at
 * order time, with a bounded timeout, and their own min/max violations are
 * reported as the *destination's* limits — a distinct discriminant from
 * the protocol's Offering limits.
 *
 * The transport is an injected port; this package performs no network IO.
 * The resolved invoice is re-parsed through the shared parser and checked
 * against the requested amount, so a resolver can never smuggle in an
 * amountless, mismatched, or wrong-network invoice.
 */
import { Duration, Effect, Schema } from "effect";

import { parseBolt11 } from "./bolt11.js";
import type {
  BitcoinNetwork,
  Bolt11InvoiceDestination,
  DeferredDestination,
  DestinationParseFailure,
} from "./model.js";

/** Boltz resolves deferred destinations with a 25 s ceiling; so do we. */
export const DEFAULT_RESOLVE_TIMEOUT_MILLIS = 25_000;

export type ResolutionOutcome =
  | { readonly outcome: "resolved"; readonly invoice: Bolt11InvoiceDestination }
  | {
      /**
       * The destination's own limits were violated. These are the
       * destination service's bounds, not the protocol's; the caller must
       * label them as such (issue #9317 §7).
       */
      readonly outcome: "destination_limits";
      readonly requestedMsat: bigint;
      readonly minMsat: bigint;
      readonly maxMsat: bigint;
    }
  | { readonly outcome: "timeout"; readonly timeoutMillis: number }
  | {
      readonly outcome: "failed";
      readonly reason: "transport" | "payload_invalid" | "offer_unsupported";
    }
  | {
      /** The resolver returned an invoice that fails the local parse. */
      readonly outcome: "invoice_invalid";
      readonly failure: DestinationParseFailure;
    }
  | {
      /** The resolved invoice does not carry the requested amount. */
      readonly outcome: "invoice_amount_mismatch";
      readonly requestedMsat: bigint;
      readonly invoiceMsat: bigint;
    };

export class DeferredResolveTransportError extends Schema.TaggedErrorClass<DeferredResolveTransportError>()(
  "DeferredResolveTransportError",
  { detail: Schema.String },
) {}

export interface LnurlPayParams {
  readonly callbackUrl: string;
  readonly minSendableMsat: bigint;
  readonly maxSendableMsat: bigint;
}

/**
 * Transport port the host supplies. LNURL-pay is two round trips (params,
 * then invoice); BOLT12 offers need a node-side invoice_request exchange
 * that the browser host may not support — `requestOfferInvoice` may be
 * absent, which resolves as `offer_unsupported`.
 */
export interface DeferredResolutionTransport {
  readonly fetchLnurlPayParams: (
    url: string,
  ) => Effect.Effect<LnurlPayParams, DeferredResolveTransportError>;
  readonly fetchLnurlInvoice: (
    callbackUrl: string,
    amountMsat: bigint,
  ) => Effect.Effect<string, DeferredResolveTransportError>;
  readonly requestOfferInvoice?: (
    offer: string,
    amountMsat: bigint,
  ) => Effect.Effect<string, DeferredResolveTransportError>;
}

export interface ResolveDeferredRequest {
  readonly deferred: DeferredDestination;
  readonly amountMsat: bigint;
  readonly network: BitcoinNetwork;
  readonly nowSeconds: number;
  readonly timeoutMillis?: number;
}

const checkInvoice = (
  raw: string,
  request: ResolveDeferredRequest,
): ResolutionOutcome => {
  const parsed = parseBolt11(raw.trim().toLowerCase(), {
    network: request.network,
    nowSeconds: request.nowSeconds,
  });
  if (!parsed.ok) {
    return { outcome: "invoice_invalid", failure: parsed.failure };
  }
  if (parsed.invoice.amountMsat !== request.amountMsat) {
    return {
      outcome: "invoice_amount_mismatch",
      requestedMsat: request.amountMsat,
      invoiceMsat: parsed.invoice.amountMsat,
    };
  }
  return { outcome: "resolved", invoice: parsed.invoice };
};

const resolveLnurl = (
  url: string,
  request: ResolveDeferredRequest,
  transport: DeferredResolutionTransport,
): Effect.Effect<ResolutionOutcome> =>
  Effect.gen(function* () {
    const params = yield* transport.fetchLnurlPayParams(url);
    if (
      request.amountMsat < params.minSendableMsat ||
      request.amountMsat > params.maxSendableMsat
    ) {
      return {
        outcome: "destination_limits",
        requestedMsat: request.amountMsat,
        minMsat: params.minSendableMsat,
        maxMsat: params.maxSendableMsat,
      } as const;
    }
    const raw = yield* transport.fetchLnurlInvoice(
      params.callbackUrl,
      request.amountMsat,
    );
    return checkInvoice(raw, request);
  }).pipe(
    Effect.catchTag("DeferredResolveTransportError", () =>
      Effect.succeed({ outcome: "failed", reason: "transport" } as const),
    ),
  );

/**
 * Resolve a deferred destination to a concrete, locally checked invoice.
 * Every path is a typed outcome; the effect never fails.
 */
export const resolveDeferredDestination = (
  request: ResolveDeferredRequest,
  transport: DeferredResolutionTransport,
): Effect.Effect<ResolutionOutcome> => {
  const timeoutMillis = request.timeoutMillis ?? DEFAULT_RESOLVE_TIMEOUT_MILLIS;
  const resolve: Effect.Effect<ResolutionOutcome> = (() => {
    switch (request.deferred.kind) {
      case "deferred_lnurl":
        return resolveLnurl(request.deferred.url, request, transport);
      case "deferred_lightning_address":
        return resolveLnurl(request.deferred.resolveUrl, request, transport);
      case "deferred_bolt12_offer": {
        const requestOfferInvoice = transport.requestOfferInvoice;
        if (requestOfferInvoice === undefined) {
          return Effect.succeed({
            outcome: "failed",
            reason: "offer_unsupported",
          } as const);
        }
        return requestOfferInvoice(
          request.deferred.offer,
          request.amountMsat,
        ).pipe(
          Effect.map((raw) => checkInvoice(raw, request)),
          Effect.catchTag("DeferredResolveTransportError", () =>
            Effect.succeed({ outcome: "failed", reason: "transport" } as const),
          ),
        );
      }
    }
  })();

  return resolve.pipe(
    Effect.timeoutOrElse({
      duration: Duration.millis(timeoutMillis),
      orElse: () =>
        Effect.succeed({ outcome: "timeout", timeoutMillis } as const),
    }),
  );
};
