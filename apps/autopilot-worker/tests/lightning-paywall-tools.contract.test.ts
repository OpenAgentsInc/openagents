import { describe, expect, it } from "vitest";
import { Schema } from "effect";

import { toolContracts } from "../src/tools";

const baseOutput = {
  status: "ok" as const,
  denyCode: null,
  denyReason: null,
  errorCode: null,
  errorMessage: null,
  httpStatus: 200,
  requestId: "req_1",
  paywall: {
    paywallId: "pw_1",
    ownerId: "owner_1",
    name: "Demo",
    status: "active" as const,
    fixedAmountMsats: 1000,
    routeCount: 1,
  },
  paywalls: [],
  settlements: [],
  nextCursor: null,
  receipt: {
    params_hash: "sha256:params",
    output_hash: "sha256:output",
    latency_ms: 2,
    side_effects: [
      {
        kind: "http_request" as const,
        target: "https://openagents.test/api/lightning/paywalls",
        method: "POST",
        status_code: 200,
        changed: true,
        detail: null,
      },
    ],
  },
};

describe("lightning paywall tool contracts", () => {
  it("validates schema contracts for all hosted paywall tools", () => {
    const contracts = [
      {
        name: "lightning_paywall_create" as const,
        contract: toolContracts.lightning_paywall_create,
        input: {
          name: "Demo Paywall",
          description: "Hosted paywall",
          status: "active",
          policy: {
            pricingMode: "fixed",
            fixedAmountMsats: 1000,
            maxPerRequestMsats: 2000,
            allowedHosts: ["api.example.com"],
            blockedHosts: [],
            quotaPerMinute: 10,
            quotaPerDay: 100,
            killSwitch: false,
          },
          routes: [
            {
              hostPattern: "api.example.com",
              pathPattern: "/premium",
              upstreamUrl: "https://upstream.example.com/premium",
              protocol: "https",
              timeoutMs: 5000,
              priority: 1,
            },
          ],
          metadata: { source: "test" },
        },
      },
      {
        name: "lightning_paywall_update" as const,
        contract: toolContracts.lightning_paywall_update,
        input: {
          paywallId: "pw_1",
          name: "Renamed",
          description: "Updated",
          policy: {
            pricingMode: "fixed",
            fixedAmountMsats: 1500,
          },
        },
      },
      {
        name: "lightning_paywall_pause" as const,
        contract: toolContracts.lightning_paywall_pause,
        input: {
          paywallId: "pw_1",
          reason: "maintenance",
        },
      },
      {
        name: "lightning_paywall_resume" as const,
        contract: toolContracts.lightning_paywall_resume,
        input: {
          paywallId: "pw_1",
          reason: "back_online",
        },
      },
      {
        name: "lightning_paywall_get" as const,
        contract: toolContracts.lightning_paywall_get,
        input: {
          paywallId: "pw_1",
        },
      },
      {
        name: "lightning_paywall_list" as const,
        contract: toolContracts.lightning_paywall_list,
        input: {
          status: "active",
          limit: 20,
        },
      },
      {
        name: "lightning_paywall_settlement_list" as const,
        contract: toolContracts.lightning_paywall_settlement_list,
        input: {
          paywallId: "pw_1",
          limit: 10,
          beforeCreatedAtMs: 1_734_000_000_000,
        },
      },
    ];

    for (const entry of contracts) {
      const decodedInput = Schema.decodeUnknownSync(entry.contract.input)(entry.input);
      expect(decodedInput).toBeTruthy();

      const outputForTool =
        entry.name === "lightning_paywall_list"
          ? {
              ...baseOutput,
              paywall: null,
              paywalls: [baseOutput.paywall],
            }
          : entry.name === "lightning_paywall_settlement_list"
            ? {
                ...baseOutput,
                paywall: null,
                paywalls: [],
                settlements: [
                  {
                    settlementId: "set_1",
                    paywallId: "pw_1",
                    amountMsats: 1000,
                    paymentProofRef: "lightning_preimage:abcd",
                    createdAtMs: 1_734_000_000_001,
                  },
                ],
                nextCursor: 1_734_000_000_000,
              }
            : { ...baseOutput };

      const decodedOutput = Schema.decodeUnknownSync(entry.contract.output)(outputForTool);
      expect(decodedOutput).toBeTruthy();
      expect(decodedOutput.receipt.params_hash.startsWith("sha256:")).toBe(true);
      expect(decodedOutput.receipt.output_hash.startsWith("sha256:")).toBe(true);
    }
  });
});
