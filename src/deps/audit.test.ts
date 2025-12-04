import { describe, expect, test } from "bun:test";
import { summarizeNcu, summarizeNpmAudit } from "./audit.js";

const sampleAudit = JSON.stringify({
  metadata: {
    vulnerabilities: { info: 0, low: 1, moderate: 2, high: 0, critical: 0 },
  },
});

const sampleNcu = JSON.stringify({
  effect: { current: "3.19.8", latest: "3.20.0" },
  bun: { current: "1.3.0", latest: "1.3.0" },
});

describe("summaries", () => {
  test("summarizeNpmAudit parses severities", () => {
    const summary = summarizeNpmAudit(sampleAudit);
    expect(summary.vulnerabilities.low).toBe(1);
    expect(summary.vulnerabilities.moderate).toBe(2);
    expect(summary.status).toBe("failed");
  });

  test("summarizeNcu counts packages", () => {
    const summary = summarizeNcu(sampleNcu);
    expect(summary.count).toBe(2);
    expect(summary.packages[0].name).toBe("effect");
  });

  test("handles missing audit output", () => {
    const summary = summarizeNpmAudit(undefined);
    expect(summary.status).toBe("failed");
  });
});
