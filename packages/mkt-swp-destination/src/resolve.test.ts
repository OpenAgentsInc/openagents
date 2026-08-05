import { describe, expect, test } from "vite-plus/test";
import { Effect } from "effect";

import type { LightningAddressDestination } from "./model.js";
import {
  DeferredResolveTransportError,
  resolveDeferredDestination,
  type DeferredResolutionTransport,
  type LnurlPayParams,
} from "./resolve.js";
import { DEFAULT_TEST_TIMESTAMP, encodeTestInvoice } from "./testkit.js";

const NOW = DEFAULT_TEST_TIMESTAMP + 60;

const deferred: LightningAddressDestination = {
  kind: "deferred_lightning_address",
  rail: "lightning",
  address: "alice@pay.example.com",
  resolveUrl: "https://pay.example.com/.well-known/lnurlp/alice",
};

const params: LnurlPayParams = {
  callbackUrl: "https://pay.example.com/lnurlp/alice/callback",
  minSendableMsat: 1_000n,
  maxSendableMsat: 1_000_000_000_000n,
};

const transportWith = (
  invoice: string,
  overrides: Partial<DeferredResolutionTransport> = {},
): DeferredResolutionTransport => ({
  fetchLnurlPayParams: () => Effect.succeed(params),
  fetchLnurlInvoice: () => Effect.succeed(invoice),
  ...overrides,
});

const run = (
  transport: DeferredResolutionTransport,
  amountMsat: bigint,
  timeoutMillis?: number,
) =>
  Effect.runPromise(
    resolveDeferredDestination(
      {
        deferred,
        amountMsat,
        network: "regtest",
        nowSeconds: NOW,
        ...(timeoutMillis === undefined ? {} : { timeoutMillis }),
      },
      transport,
    ),
  );

describe("deferred destination resolution", () => {
  test("resolves to a locally checked concrete invoice", async () => {
    const invoice = encodeTestInvoice({ amount: "2500u" });
    const outcome = await run(transportWith(invoice), 250_000_000n);
    expect(outcome.outcome).toBe("resolved");
    if (outcome.outcome === "resolved") {
      expect(outcome.invoice.amountMsat).toBe(250_000_000n);
    }
  });

  test("reports the destination's own limits, not the protocol's", async () => {
    const outcome = await run(transportWith(encodeTestInvoice()), 500n);
    expect(outcome).toEqual({
      outcome: "destination_limits",
      requestedMsat: 500n,
      minMsat: params.minSendableMsat,
      maxMsat: params.maxSendableMsat,
    });
  });

  test("an invoice with a different amount is a typed mismatch", async () => {
    const invoice = encodeTestInvoice({ amount: "2500u" });
    const outcome = await run(transportWith(invoice), 100_000_000n);
    expect(outcome).toEqual({
      outcome: "invoice_amount_mismatch",
      requestedMsat: 100_000_000n,
      invoiceMsat: 250_000_000n,
    });
  });

  test("a resolver cannot smuggle in an amountless invoice", async () => {
    const invoice = encodeTestInvoice({ amount: null });
    const outcome = await run(transportWith(invoice), 250_000_000n);
    expect(outcome.outcome).toBe("invoice_invalid");
    if (outcome.outcome === "invoice_invalid") {
      expect(outcome.failure.mode).toBe("invoice_amountless");
      expect(outcome.failure.swpError).toBe("swp_invoice_invalid");
    }
  });

  test("a wrong-network invoice is refused with its discriminant", async () => {
    const invoice = encodeTestInvoice({ amount: "2500u", network: "mainnet" });
    const outcome = await run(transportWith(invoice), 250_000_000n);
    expect(outcome.outcome).toBe("invoice_invalid");
    if (outcome.outcome === "invoice_invalid") {
      expect(outcome.failure.mode).toBe("invoice_network_mismatch");
    }
  });

  test("transport failure is a typed outcome, never an exception", async () => {
    const outcome = await run(
      transportWith("", {
        fetchLnurlPayParams: () =>
          Effect.fail(new DeferredResolveTransportError({ detail: "boom" })),
      }),
      250_000_000n,
    );
    expect(outcome).toEqual({ outcome: "failed", reason: "transport" });
  });

  test("resolution is bounded by the timeout", async () => {
    const outcome = await run(
      transportWith("", {
        fetchLnurlPayParams: () =>
          Effect.succeed(params).pipe(Effect.delay("5 seconds")),
      }),
      250_000_000n,
      20,
    );
    expect(outcome).toEqual({ outcome: "timeout", timeoutMillis: 20 });
  });

  test("bolt12 offers without host support resolve as offer_unsupported", async () => {
    const outcome = await Effect.runPromise(
      resolveDeferredDestination(
        {
          deferred: {
            kind: "deferred_bolt12_offer",
            rail: "lightning",
            offer: "lno1qgsqvgnwgcg35z6ee2h3yczraddm72xrfua9uve2rlrm9deu7xyfzrcgq",
          },
          amountMsat: 1_000n,
          network: "regtest",
          nowSeconds: NOW,
        },
        transportWith(""),
      ),
    );
    expect(outcome).toEqual({ outcome: "failed", reason: "offer_unsupported" });
  });
});
