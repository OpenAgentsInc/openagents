import { describe, expect, test } from "bun:test";

import { loadManifest, reduceKhalaHeadToHeadManifest } from "./reduce-head-to-head.mjs";

const fixturePath = new URL(
  "../../docs/inference/fixtures/khala-head-to-head-dry-run.v1.json",
  import.meta.url,
);

describe("Khala head-to-head reducer", () => {
  test("computes the dry-run scoreboard while blocking #6016 closure", () => {
    const manifest = loadManifest(fixturePath);
    const metrics = reduceKhalaHeadToHeadManifest(manifest);

    expect(metrics.schema).toBe("openagents.khala_head_to_head_metrics.v1");
    expect(metrics.summary.runCount).toBe(2);
    expect(metrics.summary.verifiedRate).toBe(1);
    expect(metrics.summary.acceptedOutcomesPerKwh).toBe("not_measured");

    const khala = metrics.scoreboard.find((run) => run.lane === "khala");
    expect(khala).toBeDefined();
    expect(khala.tokens).toBe(89600);
    expect(khala.costPerAcceptedOutcomeUsd).toBe(7.32);
    expect(khala.inWorldVsGatewaySplit.inWorldShare).toBe(0.6);
    expect(khala.acceptedOutcomesPerKwh).toBe("not_measured");

    expect(metrics.closureAudit.canClose).toBe(false);
    expect(metrics.closureAudit.blockerRefs).toContain("blocker.khala_demo.fixture_scaffold_not_live");
    expect(metrics.closureAudit.blockerRefs).toContain("blocker.khala_demo.settlement_receipts_missing");
    expect(metrics.closureAudit.blockerRefs).toContain("blocker.khala_demo.energy_telemetry_missing");
  });

  test("rejects local filesystem paths in public-safe manifest values", () => {
    const manifest = loadManifest(fixturePath);
    manifest.runs[0].sourceRefs.push("/Users/example/.secrets/provider.env");

    expect(() => reduceKhalaHeadToHeadManifest(manifest)).toThrow("unsafe");
  });

  test("computes AO per kWh when measured telemetry is present", () => {
    const manifest = loadManifest(fixturePath);
    manifest.runs[0].energy.kwhMeasured = 0.5;
    manifest.runs[0].energy.measurementRef = "fixture.energy.khala.crossy_road.0_5kwh.v1";
    manifest.runs[0].energy.blockerRefs = [];

    const metrics = reduceKhalaHeadToHeadManifest(manifest);
    const khala = metrics.scoreboard.find((run) => run.lane === "khala");

    expect(khala.acceptedOutcomesPerKwh).toBe(2);
    expect(metrics.summary.acceptedOutcomesPerKwh).toBe(2);
  });
});
