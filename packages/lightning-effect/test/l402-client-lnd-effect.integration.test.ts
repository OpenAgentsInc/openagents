import { makeLndDeterministicLayer } from "@openagentsinc/lnd-effect/adapters";
import { Effect, Layer } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { makeSpendPolicyLayer } from "../src/layers/defaultSpendPolicy.js";
import { CredentialCacheInMemoryLayer } from "../src/layers/inMemoryCredentialCache.js";
import { L402ClientLiveLayer } from "../src/layers/l402ClientLive.js";
import { makeInvoicePayerLndEffectLayer } from "../src/lnd-effect/index.js";
import { L402ClientService } from "../src/services/l402Client.js";
import { L402TransportService } from "../src/services/l402Transport.js";

describe("l402 client + lnd-effect full flow", () => {
  it.effect("pays 402 challenge through lnd-effect and reuses cached credential", () => {
    const calls: Array<{ readonly auth: string | null; readonly status: number }> = [];

    const transportLayer = Layer.succeed(
      L402TransportService,
      L402TransportService.of({
        send: (request) =>
          Effect.sync(() => {
            const auth =
              request.headers?.Authorization ??
              request.headers?.authorization ??
              null;
            if (!auth || !auth.startsWith("L402 ")) {
              calls.push({ auth: null, status: 402 });
              return {
                status: 402,
                headers: {
                  "www-authenticate":
                    'L402 invoice="lnbcrt1invoice_full_flow", macaroon="mac_full_flow", amount_msats=2500',
                },
                body: '{"error":"payment_required"}',
              };
            }

            calls.push({ auth, status: 200 });
            return {
              status: 200,
              headers: { "content-type": "application/json" },
              body: '{"ok":true}',
            };
          }),
      }),
    );

    const baseLayer = Layer.mergeAll(
      CredentialCacheInMemoryLayer,
      makeSpendPolicyLayer({
        defaultMaxSpendMsats: 50_000,
      }),
      transportLayer,
      makeInvoicePayerLndEffectLayer({
        fallbackAmountMsats: "request_max",
      }).pipe(Layer.provide(makeLndDeterministicLayer())),
    );
    const clientLayer = L402ClientLiveLayer.pipe(Layer.provide(baseLayer));
    const layer = Layer.merge(baseLayer, clientLayer);

    return Effect.gen(function* () {
      const client = yield* L402ClientService;

      const first = yield* client.fetchWithL402({
        url: "https://api.example.com/full-flow",
        method: "GET",
        maxSpendMsats: 10_000,
      });

      expect(first.statusCode).toBe(200);
      expect(first.paid).toBe(true);
      expect(first.cacheStatus).toBe("miss");
      expect(first.paymentId).not.toBeNull();
      expect(first.proofReference).toMatch(/^preimage:/);
      expect(first.authorizationHeader).toMatch(/^L402 /);
      expect(calls).toHaveLength(2);
      expect(calls[0]?.status).toBe(402);
      expect(calls[1]?.status).toBe(200);

      const second = yield* client.fetchWithL402({
        url: "https://api.example.com/full-flow",
        method: "GET",
        maxSpendMsats: 10_000,
      });

      expect(second.statusCode).toBe(200);
      expect(second.paid).toBe(false);
      expect(second.cacheStatus).toBe("hit");
      expect(second.authorizationHeader).toBe(first.authorizationHeader);
      expect(calls).toHaveLength(3);
      expect(calls[2]?.auth).toBe(first.authorizationHeader);
    }).pipe(Effect.provide(layer));
  });
});
