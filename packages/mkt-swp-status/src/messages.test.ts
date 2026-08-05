import { describe, expect, test } from "vite-plus/test";

import { swapErrorMessages } from "@openagentsinc/swap-i18n";
import { STATUS_MESSAGES, statusErrorKey } from "./messages.js";
import { CLOSE_OUTCOMES } from "./terminal.js";

describe("status message table", () => {
  test("every message is a distinct swap.status.* entry", () => {
    const values = Object.values(STATUS_MESSAGES);
    expect(new Set(values).size).toBe(values.length);
    for (const key of Object.keys(STATUS_MESSAGES)) {
      expect(key.startsWith("swap.status.")).toBe(true);
    }
  });

  test("the gap message explains unknown; it does not narrate progress", () => {
    expect(STATUS_MESSAGES["swap.status.gap_unknown"]).toContain("unknown");
    expect(STATUS_MESSAGES["swap.status.gap_not_closed_by_later"]).toContain(
      "Only the exact missing record",
    );
  });

  test("the unresolved copy states not-failed and not-complete", () => {
    expect(STATUS_MESSAGES["swap.status.terminal.unresolved"]).toContain("not failed");
    expect(STATUS_MESSAGES["swap.status.terminal.unresolved"]).toContain("not complete");
    expect(STATUS_MESSAGES["swap.status.unresolved_explainer"]).toContain("unknown");
  });

  test("every terminal outcome has copy and every exit kind has copy", () => {
    for (const outcome of CLOSE_OUTCOMES) {
      expect(STATUS_MESSAGES[`swap.status.terminal.${outcome}`]).toBeDefined();
    }
    for (const exit of [
      "none_needed",
      "claim",
      "refund",
      "rescue",
      "dispute",
      "keep_watching",
    ] as const) {
      expect(STATUS_MESSAGES[`swap.status.exit.${exit}`]).toBeDefined();
    }
  });

  test("§17 identifiers render through the shared swap-i18n table, never local prose", () => {
    for (const identifier of [
      "swp_status_gap",
      "swp_status_fork",
      "swp_status_signer_invalid",
      "swp_status_transition_invalid",
      "swp_settlement_overclaim",
      "swp_unresolved_loss",
    ] as const) {
      const key = statusErrorKey(identifier);
      expect(key).toBe(`swap.error.${identifier}`);
      expect(swapErrorMessages[key]).toBeDefined();
    }
  });
});
