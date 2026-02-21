import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import { runObservabilitySmoke } from "../src/programs/smokeObservability.js";

describe("lightning-ops smoke:observability", () => {
  it.effect("mock mode emits complete hosted-node observability records with correlation IDs", () =>
    Effect.gen(function* () {
      const summary = yield* runObservabilitySmoke({
        mode: "mock",
        requestId: "smoke:observability:test",
      });

      expect(summary.executionPath).toBe("hosted-node");
      expect(summary.records.length).toBeGreaterThanOrEqual(4);
      expect(summary.missingFieldKeys).toEqual([]);
      expect(summary.correlation.requestIds.length).toBeGreaterThan(0);
      expect(summary.correlation.paymentProofRefs.length).toBeGreaterThan(0);

      const planes = new Set(summary.records.map((row) => row.plane));
      expect(planes.has("control")).toBe(true);
      expect(planes.has("gateway")).toBe(true);
      expect(planes.has("settlement")).toBe(true);
      expect(planes.has("ui")).toBe(true);

      for (const record of summary.records) {
        expect(record.executionPath).toBe("hosted-node");
        expect(record.desktopSessionId).toBeNull();
        expect(record.desktopRuntimeStatus).toBeNull();
        expect(record.walletState).toBeNull();
        expect(record.nodeSyncStatus).toBeNull();
      }
    }),
  );
});
