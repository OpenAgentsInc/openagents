import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { runSettlementSmoke } from "../src/programs/smokeSettlement.js";

describe("lightning-ops smoke:settlement", () => {
  it.effect("mock mode emits machine-readable settlement IDs and proof references", () =>
    Effect.gen(function* () {
      const summary = yield* runSettlementSmoke({ mode: "mock" });

      expect(summary.processed).toBeGreaterThan(0);
      expect(summary.settlements.length).toBeGreaterThan(0);
      expect(summary.correlationRefs.length).toBe(summary.settlements.length);
      expect(summary.settlements[0]?.settlementId).toBeDefined();
      expect(summary.settlements[0]?.paymentProofRef.startsWith("lightning_preimage:")).toBe(true);
      expect(summary.settlements.some((row) => row.existed)).toBe(true);
    }),
  );
});
