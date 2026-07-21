import { describe, expect, test } from "vite-plus/test";

import {
  aggregateRound,
  roundIsClean,
  type LensOutcome,
  type LensReport,
  type ReviewFinding,
} from "./index";

const finding = (over: Partial<ReviewFinding> = {}): ReviewFinding => ({
  lens: "correctness",
  title: "off-by-one in slice bound",
  severity: "high",
  claim: "slice reads one past the end",
  probe: "called f([1,2,3], 3)",
  observed: "threw RangeError at index 3",
  ...over,
});

const report = (over: Partial<LensReport> = {}): LensReport => ({
  lens: "correctness",
  probesRun: 4,
  findings: [],
  ...over,
});

const outcome = (lens: string, report: LensReport | null): LensOutcome => ({ lens, report });

describe("aggregateRound anti-laundering rules", () => {
  test("rule 2: a died lens surfaces as AGENT-DIED, never a clean pass", () => {
    const result = aggregateRound([
      outcome("correctness", report({ lens: "correctness" })),
      outcome("security", null),
    ]);

    expect(result.status).toBe("failed");
    expect(roundIsClean(result)).toBe(false);
    const died = result.failures.find((f) => f.kind === "agent-died");
    expect(died?.lens).toBe("security");
    // The proven lens is still counted, but the round cannot be clean while a
    // lens died — the death is not dropped.
    expect(result.lensesSwept).toBe(1);
    expect(result.lensesAttempted).toBe(2);
  });

  test("rule 1: empty findings with zero probes is unproven, not clean", () => {
    const result = aggregateRound([outcome("correctness", report({ probesRun: 0, findings: [] }))]);

    expect(result.status).toBe("failed");
    expect(roundIsClean(result)).toBe(false);
    expect(result.failures[0]?.kind).toBe("lens-unproven");
    expect(result.lensesSwept).toBe(0);
  });

  test("rule 1 positive control: empty findings WITH probes is clean", () => {
    const result = aggregateRound([
      outcome("correctness", report({ probesRun: 3, findings: [] })),
      outcome("security", report({ lens: "security", probesRun: 2, findings: [] })),
    ]);

    expect(result.status).toBe("clean");
    expect(roundIsClean(result)).toBe(true);
    expect(result.failures).toHaveLength(0);
    expect(result.lensesSwept).toBe(2);
    expect(result.probesRun).toBe(5);
  });

  test("rule 3: a finding without a reproduced observation does not count and blocks clean", () => {
    const result = aggregateRound([
      outcome("correctness", report({ probesRun: 5, findings: [finding({ observed: "   " })] })),
    ]);

    expect(result.status).toBe("failed");
    expect(result.confirmedFindings).toHaveLength(0);
    expect(result.failures[0]?.kind).toBe("unsubstantiated-finding");
  });

  test("rule 3: a finding with a reproduced observation is confirmed", () => {
    const result = aggregateRound([
      outcome("correctness", report({ probesRun: 5, findings: [finding()] })),
    ]);

    expect(result.status).toBe("findings");
    expect(result.confirmedFindings).toHaveLength(1);
    expect(result.failures).toHaveLength(0);
    // A lens that produced a real finding demonstrably swept.
    expect(result.lensesSwept).toBe(1);
  });

  test("rule 4: a malformed report is a failure, not an optimistic pass", () => {
    // probesRun as a string does not decode against the contract.
    const malformed = { lens: "perf", probesRun: "lots", findings: [] };
    const result = aggregateRound([outcome("perf", malformed as unknown as LensReport)]);

    expect(result.status).toBe("failed");
    expect(result.failures[0]?.kind).toBe("malformed-report");
    expect(result.lensesReported).toBe(0);
  });

  test("a mixed round reports findings and failures together without laundering", () => {
    const result = aggregateRound([
      outcome("correctness", report({ probesRun: 6, findings: [finding()] })),
      outcome("security", null),
      outcome("perf", report({ lens: "perf", probesRun: 0, findings: [] })),
    ]);

    // Any failure row forces `failed`, even though a real finding also exists —
    // the round is inconclusive until the died and unproven lenses rerun.
    expect(result.status).toBe("failed");
    expect(result.confirmedFindings).toHaveLength(1);
    expect(result.failures.map((f) => f.kind).sort()).toEqual(["agent-died", "lens-unproven"]);
  });

  test("an empty round is failed, not vacuously clean", () => {
    const result = aggregateRound([]);
    // Zero lenses proved zero sweeps. There is nothing to call clean.
    expect(result.status).toBe("failed");
    expect(roundIsClean(result)).toBe(false);
    expect(result.failures[0]?.kind).toBe("no-sweep");
    expect(result.lensesSwept).toBe(0);
    expect(result.lensesAttempted).toBe(0);
  });
});
