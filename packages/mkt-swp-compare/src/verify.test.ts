/**
 * Behaviour-contract oracle for the funding gate (issue #9318 §6).
 *
 * Enforced contract (registry: `@openagentsinc/behavior-contracts`,
 * market-swap-compare):
 * - openagents_web.swap_compare.funding_disabled_until_checks_pass.v1
 *   The fund action cannot be enabled while any verify-before-fund check is
 *   unresolved or failed (`swp_funding_not_authorized`); verification truth
 *   arrives only from the engine port.
 */
import { describe, expect, test } from "vite-plus/test";

import { allPassRows, testVerifyReport } from "./testkit.js";
import { fundingGate, VERIFY_CHECK_IDS } from "./verify.js";
import { verifyChecklistView } from "./view.js";

describe("fundingGate", () => {
  test("no report: disabled with swp_funding_not_authorized", () => {
    const gate = fundingGate(null, 1);
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) {
      expect(gate.error).toBe("swp_funding_not_authorized");
      expect(gate.reason).toBe("no_report");
    }
  });

  test("stale epoch: a superseded report never enables funding", () => {
    const gate = fundingGate(testVerifyReport({ epoch: 1 }), 2);
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) expect(gate.reason).toBe("stale_report");
  });

  test("any unresolved row keeps funding disabled and names the row", () => {
    const rows = allPassRows().map(row =>
      row.id === "timeout_ladder"
        ? ({ id: row.id, status: "unresolved" } as const)
        : row,
    );
    const gate = fundingGate(testVerifyReport({ rows }), 1);
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) {
      expect(gate.reason).toBe("rows_incomplete");
      expect(gate.unresolved).toEqual(["timeout_ladder"]);
    }
  });

  test("a missing row is unresolved, never implicitly passed", () => {
    const rows = allPassRows().filter(row => row.id !== "exit_package");
    const gate = fundingGate(testVerifyReport({ rows }), 1);
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) expect(gate.unresolved).toEqual(["exit_package"]);
  });

  test("any failed row keeps funding disabled with its typed identifier", () => {
    const rows = allPassRows().map(row =>
      row.id === "output_key_rederived"
        ? ({
            id: row.id,
            status: "fail",
            error: "swp_script_commitment_mismatch",
          } as const)
        : row,
    );
    const gate = fundingGate(testVerifyReport({ rows }), 1);
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) {
      expect(gate.failed).toEqual([
        {
          id: "output_key_rederived",
          status: "fail",
          error: "swp_script_commitment_mismatch",
        },
      ]);
    }
  });

  test("all rows pass but the engine verdict is blocked: still disabled — the UI never overrides the engine", () => {
    const gate = fundingGate(
      testVerifyReport({ verdict: "verification_blocked" }),
      1,
    );
    expect(gate.enabled).toBe(false);
    if (!gate.enabled) expect(gate.reason).toBe("engine_verdict_blocked");
  });

  test("enabled only on a complete engine pass for the current epoch", () => {
    const gate = fundingGate(testVerifyReport(), 1);
    expect(gate).toEqual({ enabled: true, reportEpoch: 1 });
  });
});

describe("verifyChecklistView", () => {
  test("a null report renders every row unresolved with the gate disabled", () => {
    const view = verifyChecklistView(null, 1);
    expect(view.rows).toHaveLength(VERIFY_CHECK_IDS.length);
    for (const row of view.rows) expect(row.status).toBe("unresolved");
    expect(view.gate.enabled).toBe(false);
  });

  test("every failing row is individually identifiable: id, label, and §17 identifier", () => {
    const rows = allPassRows().map(row =>
      row.id === "lightning_invoice"
        ? ({ id: row.id, status: "fail", error: "swp_invoice_invalid" } as const)
        : row,
    );
    const view = verifyChecklistView(testVerifyReport({ rows }), 1);
    const failing = view.rows.filter(row => row.status === "fail");
    expect(failing).toHaveLength(1);
    expect(failing[0]?.id).toBe("lightning_invoice");
    expect(failing[0]?.error).toBe("swp_invoice_invalid");
    expect(failing[0]?.label.key).toBe("swap.compare.verify.lightning_invoice");
    // Funding is unreachable while the row fails.
    expect(view.gate.enabled).toBe(false);
  });

  test("no two checklist rows share a label — a failure names its row, never a generic page", () => {
    const view = verifyChecklistView(null, 1);
    const keys = view.rows.map(row => row.label.key);
    const messages = view.rows.map(row => row.label.message);
    expect(new Set(keys).size).toBe(view.rows.length);
    expect(new Set(messages).size).toBe(view.rows.length);
  });
});
