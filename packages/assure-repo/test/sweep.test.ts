import { execFileSync } from "node:child_process";

import { describe, expect, test } from "vite-plus/test";

import {
  decodeSweepReceipt,
  diffSweeps,
  repositoryRoot,
  runSweep,
  serializeSweepReceipt,
  type SweepReceipt,
} from "../src/index.ts";

const root = repositoryRoot();
const at = new Date(1_700_000_000_000).toISOString();

describe("runSweep against the real repository", () => {
  test("produces a decodable receipt with the ASSURE-REPO oracles and honest evidence class", () => {
    const receipt = runSweep(root, at);
    // Round-trips through the schema.
    const decoded = decodeSweepReceipt(JSON.parse(serializeSweepReceipt(receipt)));
    expect(decoded.oracleOutcomes.map((o) => o.oracle)).toEqual([
      "assure-repo.inventory_fresh",
      "assure-repo.audit_fresh",
      "assure-repo.doc_drift",
    ]);
    // Runs degraded and labels it honestly before IDE-10 host-observed tests.
    expect(receipt.evidenceClass).toBe("degraded_terminal_observed");
    expect(["green", "red", "unknown"]).toContain(receipt.overall);
  });

  test("is read-only: running it mutates no tracked source (cannot alter guardrails)", () => {
    const before = execFileSync("git", ["-C", root, "status", "--porcelain"], { encoding: "utf8" });
    runSweep(root, at);
    const after = execFileSync("git", ["-C", root, "status", "--porcelain"], { encoding: "utf8" });
    expect(after).toBe(before);
  });

  test("overall is green only when every oracle passed", () => {
    const receipt = runSweep(root, at);
    const allPass = receipt.oracleOutcomes.every((o) => o.outcome === "pass");
    expect(receipt.overall === "green").toBe(allPass);
  });
});

describe("diffSweeps surfaces regressions", () => {
  const base = (): SweepReceipt => ({
    schemaVersion: "1",
    repository: "OpenAgentsInc/openagents",
    commit: "a",
    generatedAt: at,
    evidenceClass: "degraded_terminal_observed",
    inventorySourceDigest: "sha256:x",
    oracleOutcomes: [{ oracle: "assure-repo.inventory_fresh", outcome: "pass", detail: "" }],
    obligationSummary: { designed: 10 },
    driftSummary: { broken: 0, open: 0, unverifiable: 0 },
    overall: "green",
  });

  test("flags an oracle that regressed from pass", () => {
    const prev = base();
    const curr = {
      ...base(),
      oracleOutcomes: [
        { oracle: "assure-repo.inventory_fresh", outcome: "fail" as const, detail: "stale" },
      ],
      overall: "red" as const,
    };
    const findings = diffSweeps(prev, curr);
    expect(findings.some((f) => f.kind === "oracle_regressed")).toBe(true);
    expect(findings.some((f) => f.kind === "overall_regressed")).toBe(true);
  });

  test("flags a drop in designed surfaces", () => {
    const prev = base();
    const curr = { ...base(), obligationSummary: { designed: 8 } };
    expect(diffSweeps(prev, curr).some((f) => f.kind === "obligation_regressed")).toBe(true);
  });

  test("no findings when nothing regressed", () => {
    expect(diffSweeps(base(), base())).toEqual([]);
  });
});
