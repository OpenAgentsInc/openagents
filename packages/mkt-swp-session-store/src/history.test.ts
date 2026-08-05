/**
 * History view-model: actionability-first ordering computed from chain
 * facts and tip height rather than the stored status alone, per-row action
 * and outcome/rung labels, and typed swap-i18n keys that render.
 */
import { makeCatalog, type MessageKey } from "@openagentsinc/swap-i18n";
import { describe, expect, test } from "vite-plus/test";

import { HISTORY_EMPTY_KEY, RELOAD_GUARD_KEY, historyRowOf, historyRows, type SessionChainFacts } from "./history.js";
import { sampleSession } from "./testkit.js";

const catalog = makeCatalog();

const NO_FACTS = () => null;

const facts = (overrides: Partial<SessionChainFacts>): SessionChainFacts => ({
  tipHeight: 850_000,
  destinationClaimable: false,
  refundUnlocked: false,
  ...overrides,
});

describe("actionability-first ordering", () => {
  test("claim/refund/exit rows float above newer non-actionable rows", () => {
    const oldClaimable = sampleSession("old-claimable", { createdAt: 100 });
    const oldRefundable = sampleSession("old-refundable", { createdAt: 200 });
    const newQuiet = sampleSession("new-quiet", {
      createdAt: 900,
      projection: {
        state: "completed",
        terminal: true,
        outcome: "completed",
        rung: "settled",
        unclaimedFunds: false,
      },
    });
    const rows = historyRows([newQuiet, oldClaimable, oldRefundable], (sessionId) => {
      if (sessionId === "old-claimable") return facts({ destinationClaimable: true });
      if (sessionId === "old-refundable") return facts({ refundUnlocked: true });
      return facts({});
    });
    // Actionable first (chronology within the band), quiet terminal row last.
    expect(rows.map((row) => row.sessionId)).toEqual([
      "old-refundable",
      "old-claimable",
      "new-quiet",
    ]);
    expect(rows[0]?.action).toBe("refund");
    expect(rows[1]?.action).toBe("claim");
    expect(rows[2]?.action).toBeNull();
  });

  test("actionability comes from chain facts, not the stored status", () => {
    // The stored projection says settled-and-done; the chain says an output
    // is still claimable. The chain wins.
    const staleStatus = sampleSession("stale", {
      projection: {
        state: "completed",
        terminal: true,
        outcome: "completed",
        rung: "settled",
        unclaimedFunds: false,
      },
    });
    const row = historyRowOf(staleStatus, facts({ destinationClaimable: true }));
    expect(row.action).toBe("claim");
  });

  test("a pending external effect surfaces the exit action", () => {
    const crashWindow = sampleSession("crash-window", {
      effectLedger: [
        {
          effectId: "fund-1",
          requestDigestHex: "ab".repeat(32),
          request: { operation: "funding_broadcast" },
          result: null,
        },
      ],
    });
    expect(historyRowOf(crashWindow, null).action).toBe("exit");
  });

  test("null chain facts degrade honestly to resume/chronology", () => {
    const inFlight = sampleSession("in-flight");
    expect(historyRowOf(inFlight, null).action).toBe("resume");
    const settled = sampleSession("settled", {
      projection: {
        state: "completed",
        terminal: true,
        outcome: "completed",
        rung: "settled",
        unclaimedFunds: false,
      },
    });
    expect(historyRowOf(settled, null).action).toBeNull();
  });
});

describe("labels", () => {
  test("terminal outcomes carry their rung label; a claim without evidence says so", () => {
    const claimedOnly = sampleSession("claimed-only", {
      projection: {
        state: "completed",
        terminal: true,
        outcome: "completed",
        rung: null,
        unclaimedFunds: false,
      },
    });
    const row = historyRowOf(claimedOnly, NO_FACTS());
    expect(row.outcomeLabelKey).toBe("swap.history.outcome.completed");
    expect(row.rungLabelKey).toBe("swap.history.rung.claimed_only");

    const settled = sampleSession("settled", {
      projection: {
        state: "completed",
        terminal: true,
        outcome: "completed",
        rung: "settled",
        unclaimedFunds: false,
      },
    });
    expect(historyRowOf(settled, NO_FACTS()).rungLabelKey).toBe("swap.history.rung.settled");
  });

  test("every emitted key renders through the shared catalog", () => {
    const keys: ReadonlyArray<MessageKey> = [
      HISTORY_EMPTY_KEY,
      RELOAD_GUARD_KEY,
      "swap.history.action.claim",
      "swap.history.action.refund",
      "swap.history.action.exit",
      "swap.history.action.resume",
      "swap.history.delete_confirm",
      "swap.history.outcome.completed",
      "swap.history.outcome.unresolved",
      "swap.history.rung.claimed_only",
      "swap.history.rung.settled",
      "swap.history.export.sensitivity",
      "swap.history.import.refused.digest_mismatch",
    ];
    for (const key of keys) {
      // All emitted history keys are static messages.
      const message: string | ((params: never) => string) = catalog[key];
      expect(typeof message).toBe("string");
      expect((message as string).length).toBeGreaterThan(0);
    }
  });
});
