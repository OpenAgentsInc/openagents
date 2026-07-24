import { describe, expect, it } from "vite-plus/test";

import {
  SarahNostrSecretLeakError,
  assertSarahNostrPublicSafe,
} from "../nostr-identity/redaction.ts";
import {
  SARAH_COMMUNITY_JOURNEY_STEPS,
  runSarahCommunityJourney,
  serializeSarahCommunityJourneyReceipt,
  validateSarahCommunityJourneyReceipt,
  SARAH_COMMUNITY_JOURNEY_RECEIPT_SCHEMA,
  decodeSarahCommunityJourneyReceipt,
} from "./index.ts";

describe("SARAH-CW-09 Sarah community journey harness", () => {
  it("runs simulated journey, emits a schema-valid public-safe receipt", async () => {
    const receipt = await runSarahCommunityJourney({
      generatedAt: "2026-07-24T23:00:00.000Z",
    });

    expect(receipt.schema).toBe(SARAH_COMMUNITY_JOURNEY_RECEIPT_SCHEMA);
    expect(receipt.packet).toBe("SARAH-CW-09");
    expect(receipt.issue).toBe("OpenAgentsInc/openagents#9231");
    expect(receipt.mode).toBe("simulated");
    expect(receipt.steps).toHaveLength(SARAH_COMMUNITY_JOURNEY_STEPS.length);
    expect(receipt.summary.automatedFailed).toBe(0);
    expect(receipt.summary.overall).toBe("simulated_green");
    expect(receipt.summary.humanResidual).toBeGreaterThan(0);
    expect(receipt.redaction.ok).toBe(true);
    expect(receipt.independentReviewer.status).toBe("pending");
    expect(receipt.independentReviewer.checklist.length).toBeGreaterThanOrEqual(
      7,
    );

    const automated = receipt.steps.filter((s) => s.class === "automated");
    expect(automated.every((s) => s.status === "passed")).toBe(true);
    const human = receipt.steps.filter((s) => s.class === "human");
    expect(human.every((s) => s.status === "skipped_human")).toBe(true);

    assertSarahNostrPublicSafe(receipt);
    const json = serializeSarahCommunityJourneyReceipt(receipt);
    expect(json.includes('"privateKey"')).toBe(false);
    expect(json.includes("nsec1")).toBe(false);
    expect(json.includes('"mnemonic"')).toBe(false);

    const revalidated = validateSarahCommunityJourneyReceipt(JSON.parse(json));
    expect(revalidated.summary.automatedPassed).toBe(
      receipt.summary.automatedPassed,
    );
  });

  it("rejects receipts that contain secret-shaped fields", async () => {
    const receipt = await runSarahCommunityJourney();
    const poisoned = {
      ...receipt,
      privateKey: "deadbeef",
    };
    expect(() => assertSarahNostrPublicSafe(poisoned)).toThrow(
      SarahNostrSecretLeakError,
    );
    expect(() => validateSarahCommunityJourneyReceipt(poisoned)).toThrow();
  });

  it("rejects nsec-looking string leaves in receipt projections", () => {
    expect(() =>
      assertSarahNostrPublicSafe({
        schema: SARAH_COMMUNITY_JOURNEY_RECEIPT_SCHEMA,
        note: "nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
      }),
    ).toThrow(SarahNostrSecretLeakError);
  });

  it("decode fails on wrong schema id", () => {
    expect(() =>
      decodeSarahCommunityJourneyReceipt(
        {
          schema: "openagents.sarah.community_journey_receipt.v0",
          packet: "SARAH-CW-09",
        },
        { onExcessProperty: "error" },
      ),
    ).toThrow();
  });

  it("live mode is refused in the automated harness", async () => {
    await expect(runSarahCommunityJourney({ mode: "live" })).rejects.toThrow(
      /live mode requires a real outside developer/,
    );
  });

  it("canonical step ids are stable and unique", () => {
    const ids = SARAH_COMMUNITY_JOURNEY_STEPS.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids[0]).toBe("J01_invite_outside_developer");
    expect(ids[ids.length - 1]).toBe("J21_developer_confirms_own_words");
    expect(ids).toContain("J10_no_payment_room_copy");
    expect(ids).toContain("J13_refuse_replay_self_verify_expired");
  });
});
