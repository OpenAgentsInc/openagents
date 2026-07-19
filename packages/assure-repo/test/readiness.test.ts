import { describe, expect, test } from "vite-plus/test";

import { renderReadiness, type SweepReceipt } from "../src/index.ts";

const receipt = (overrides: Partial<SweepReceipt>): SweepReceipt => ({
  schemaVersion: "1",
  repository: "OpenAgentsInc/openagents",
  commit: "abc123",
  generatedAt: new Date(1_000_000_000_000).toISOString(),
  evidenceClass: "degraded_terminal_observed",
  inventorySourceDigest: "sha256:x",
  oracleOutcomes: [],
  obligationSummary: {},
  driftSummary: { broken: 0, open: 0, unverifiable: 0 },
  overall: "green",
  ...overrides,
});

const now = 1_000_000_000_000;

describe("renderReadiness — no receipt means no light", () => {
  test("no receipt renders unknown, never green", () => {
    const r = renderReadiness(undefined, now);
    expect(r.state).toBe("unknown");
    expect(r.reason).toContain("no receipt means no light");
  });

  test("a stale receipt renders unknown, never green", () => {
    const old = receipt({
      generatedAt: new Date(now - 48 * 3_600_000).toISOString(),
      overall: "green",
    });
    const r = renderReadiness(old, now, 24 * 3_600_000);
    expect(r.state).toBe("unknown");
    expect(r.reason).toContain("stale");
  });

  test("a fresh green receipt renders green", () => {
    const r = renderReadiness(
      receipt({ generatedAt: new Date(now).toISOString(), overall: "green" }),
      now,
    );
    expect(r.state).toBe("green");
  });

  test("a fresh red receipt renders red", () => {
    const r = renderReadiness(
      receipt({ generatedAt: new Date(now).toISOString(), overall: "red" }),
      now,
    );
    expect(r.state).toBe("red");
  });

  test("an unparseable timestamp renders unknown", () => {
    const r = renderReadiness(receipt({ generatedAt: "not-a-date" }), now);
    expect(r.state).toBe("unknown");
  });

  test("an inconclusive sweep renders unknown, never green", () => {
    const r = renderReadiness(
      receipt({ generatedAt: new Date(now).toISOString(), overall: "unknown" }),
      now,
    );
    expect(r.state).toBe("unknown");
  });
});
