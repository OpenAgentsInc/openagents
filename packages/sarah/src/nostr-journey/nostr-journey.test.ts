import { describe, expect, it } from "vite-plus/test";

import {
  SarahNostrSecretLeakError,
  assertSarahNostrPublicSafe,
} from "../nostr-identity/redaction.ts";
import {
  SARAH_NOSTR_JOURNEY_STEPS,
  runSarahNostrJourney,
  serializeSarahNostrJourneyReceipt,
  validateSarahNostrJourneyReceipt,
  SARAH_NOSTR_JOURNEY_RECEIPT_SCHEMA,
  decodeSarahNostrJourneyReceipt,
} from "./index.ts";

describe("SARAH-NR-09 Sarah Nostr journey harness", () => {
  it("runs simulated journey, emits a schema-valid public-safe receipt", async () => {
    const receipt = await runSarahNostrJourney({
      generatedAt: "2026-07-24T22:00:00.000Z",
    });

    expect(receipt.schema).toBe(SARAH_NOSTR_JOURNEY_RECEIPT_SCHEMA);
    expect(receipt.packet).toBe("SARAH-NR-09");
    expect(receipt.mode).toBe("simulated");
    expect(receipt.steps).toHaveLength(SARAH_NOSTR_JOURNEY_STEPS.length);
    expect(receipt.summary.automatedFailed).toBe(0);
    expect(receipt.summary.overall).toBe("simulated_green");
    expect(receipt.summary.humanResidual).toBeGreaterThan(0);
    expect(receipt.redaction.ok).toBe(true);
    expect(receipt.independentReviewer.status).toBe("pending");
    expect(receipt.independentReviewer.checklist.length).toBeGreaterThanOrEqual(6);

    const automated = receipt.steps.filter((s) => s.class === "automated");
    expect(automated.every((s) => s.status === "passed")).toBe(true);
    const human = receipt.steps.filter((s) => s.class === "human");
    expect(human.every((s) => s.status === "skipped_human")).toBe(true);

    assertSarahNostrPublicSafe(receipt);
    const json = serializeSarahNostrJourneyReceipt(receipt);
    // Field names and nsec1 material are forbidden; the word "nsec" may appear
    // in reviewer checklist prose that names the redaction rule.
    expect(json.includes('"privateKey"')).toBe(false);
    expect(json.includes("nsec1")).toBe(false);
    expect(json.includes('"mnemonic"')).toBe(false);

    const revalidated = validateSarahNostrJourneyReceipt(JSON.parse(json));
    expect(revalidated.summary.automatedPassed).toBe(receipt.summary.automatedPassed);
  });

  it("rejects receipts that contain secret-shaped fields", async () => {
    const receipt = await runSarahNostrJourney();
    const poisoned = {
      ...receipt,
      privateKey: "deadbeef",
    };
    expect(() => assertSarahNostrPublicSafe(poisoned)).toThrow(
      SarahNostrSecretLeakError,
    );
    expect(() => validateSarahNostrJourneyReceipt(poisoned)).toThrow();
  });

  it("rejects nsec-looking string leaves in receipt projections", () => {
    expect(() =>
      assertSarahNostrPublicSafe({
        schema: SARAH_NOSTR_JOURNEY_RECEIPT_SCHEMA,
        note: "nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
      }),
    ).toThrow(SarahNostrSecretLeakError);
  });

  it("decode fails on wrong schema id", () => {
    expect(() =>
      decodeSarahNostrJourneyReceipt(
        {
          schema: "openagents.sarah.nostr_journey_receipt.v0",
          packet: "SARAH-NR-09",
        },
        { onExcessProperty: "error" },
      ),
    ).toThrow();
  });

  it("live mode is refused in the automated harness", async () => {
    await expect(runSarahNostrJourney({ mode: "live" })).rejects.toThrow(
      /live mode requires a signed Omega install/,
    );
  });

  it("canonical step ids are stable and unique", () => {
    const ids = SARAH_NOSTR_JOURNEY_STEPS.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids[0]).toBe("J01_install_clean_profile");
    expect(ids[ids.length - 1]).toBe("J23_remove_omega_no_side_effect");
  });
});
