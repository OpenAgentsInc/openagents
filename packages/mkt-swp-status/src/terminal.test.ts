import { describe, expect, test } from "vite-plus/test";

import {
  CLOSE_OUTCOMES,
  closesView,
  isWatchTerminal,
  lossAccountingView,
  terminalDescriptor,
} from "./terminal.js";
import { close, COMPLETE_LOSS_ACCOUNTING } from "./testkit.js";

describe("terminality has one definition", () => {
  test("every terminal outcome states a user exit", () => {
    for (const outcome of CLOSE_OUTCOMES) {
      const descriptor = terminalDescriptor(outcome);
      expect(descriptor.exit).toBeDefined();
      expect(descriptor.displayKey).toBe(`swap.status.terminal.${outcome}`);
    }
  });

  test("unresolved is never watch-terminal and displays as unresolved, not failed or complete", () => {
    const record = close("requester", "unresolved");
    expect(isWatchTerminal(record)).toBe(false);
    const descriptor = terminalDescriptor("unresolved");
    expect(descriptor.displayKey).toBe("swap.status.terminal.unresolved");
    expect(descriptor.exit).toBe("keep_watching");
  });

  test("failed stays watched unless principal is fully accounted (the Boltz divergence, fixed)", () => {
    expect(
      isWatchTerminal(close("requester", "failed", { lossAccounting: COMPLETE_LOSS_ACCOUNTING })),
    ).toBe(true);
    expect(
      isWatchTerminal(
        close("requester", "failed", {
          lossAccounting: { ...COMPLETE_LOSS_ACCOUNTING, principal_unresolved: "12000" },
        }),
      ),
    ).toBe(false);
    expect(
      isWatchTerminal(
        close("requester", "failed", {
          lossAccounting: COMPLETE_LOSS_ACCOUNTING,
          unknownFields: ["miner_fee_paid"],
        }),
      ),
    ).toBe(false);
    expect(isWatchTerminal(close("requester", "failed", { lossAccounting: undefined }))).toBe(
      false,
    );
  });

  test("disputed keeps being watched", () => {
    expect(isWatchTerminal(close("provider", "disputed"))).toBe(false);
  });
});

describe("loss accounting rendering (§15)", () => {
  test("unknown values render as unknown, never as zero", () => {
    const view = lossAccountingView(
      { ...COMPLETE_LOSS_ACCOUNTING, miner_fee_paid: "0" },
      ["miner_fee_paid"],
    );
    const miner = view.fields.find((field) => field.field === "miner_fee_paid")!;
    expect(miner.value).toBe("unknown");
    expect(view.complete).toBe(false);
  });

  test("fees are their own rows, never collapsed into principal", () => {
    const view = lossAccountingView(COMPLETE_LOSS_ACCOUNTING);
    const feeRows = view.fields.filter((field) => field.isFee).map((field) => field.field);
    expect(feeRows).toEqual([
      "provider_fee_paid",
      "miner_fee_paid",
      "lightning_routing_fee_paid",
    ]);
    expect(view.fields.map((field) => field.field)).toContain("input_committed");
  });
});

describe("conflicting Closes", () => {
  test("both parties' Closes remain visible; completed does not force the other's outcome", () => {
    const view = closesView([
      close("provider", "completed"),
      close("requester", "unresolved"),
    ]);
    expect(view.closes).toHaveLength(2);
    expect(view.conflict).toBe(true);
    expect(view.watchTerminal).toBe(false);
    expect(view.closes.map((c) => c.close.outcome).sort()).toEqual([
      "completed",
      "unresolved",
    ]);
  });

  test("agreement on a verified terminal outcome ends the watch", () => {
    const view = closesView([close("provider", "refunded"), close("requester", "refunded")]);
    expect(view.conflict).toBe(false);
    expect(view.watchTerminal).toBe(true);
  });
});
