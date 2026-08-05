import { describe, expect, test } from "vite-plus/test";

import { ladderRungs, ladderView, type SubmarineLadder } from "./ladder.js";

const SUBMARINE: SubmarineLadder = {
  kind: "submarine",
  hFund: 100,
  hClaim: 130,
  hRefund: 160,
  invoiceExpirationTime: 1_785_900_000,
};

describe("timeout-ladder rungs", () => {
  test("every rung states the user's exit within and after its boundary", () => {
    for (const ladder of [
      SUBMARINE,
      { kind: "reverse", hLockLast: 10, hUserClaim: 20, hProviderRefund: 30, hHoldExpiry: 40 },
      { kind: "chain", hDestinationRefund: 50, hSourceRefund: 90 },
    ] as const) {
      for (const rung of ladderRungs(ladder)) {
        expect(rung.exitWithin).toBeDefined();
        expect(rung.exitAfter).toBeDefined();
        expect(rung.labelKey.startsWith("swap.status.ladder.")).toBe(true);
      }
    }
  });

  test("heights are the authority; times are flagged estimates", () => {
    const view = ladderView("submarine", SUBMARINE, 90, (height) => height * 600);
    for (const rung of view.rungs) {
      expect(rung.timeIsEstimate).toBe(true);
      expect(rung.estimatedTime).toBe(rung.height * 600);
    }
  });

  test("without a height observation no boundary is treated as crossed", () => {
    const view = ladderView("submarine", SUBMARINE, null);
    expect(view.stopTrustingCounterpartyClaims).toBe(false);
    expect(view.rungs.every((rung) => rung.status !== "passed")).toBe(true);
  });

  test("crossing the refund boundary switches the exit to refund and stops trusting claims", () => {
    const before = ladderView("submarine", SUBMARINE, 159);
    expect(before.rungs.find((rung) => rung.id === "refund_valid")!.exitNow).toBe(
      "keep_watching",
    );
    const after = ladderView("submarine", SUBMARINE, 160);
    const refundRung = after.rungs.find((rung) => rung.id === "refund_valid")!;
    expect(refundRung.status).toBe("passed");
    expect(refundRung.exitNow).toBe("refund");
    expect(after.stopTrustingCounterpartyClaims).toBe(true);
  });

  test("a mismatched flow/ladder pair is refused", () => {
    expect(() => ladderView("reverse", SUBMARINE, 100)).toThrow();
  });
});
