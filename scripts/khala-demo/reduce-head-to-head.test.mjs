import { describe, expect, test } from "bun:test";

import { loadManifest, reduceKhalaHeadToHeadManifest } from "./reduce-head-to-head.mjs";

const fixturePath = new URL(
  "../../docs/inference/fixtures/khala-head-to-head-dry-run.v1.json",
  import.meta.url,
);

function cloneFixtureManifest() {
  return structuredClone(loadManifest(fixturePath));
}

function livePromotionManifest() {
  const manifest = cloneFixtureManifest();
  let refIndex = 0;
  const liveRef = () => `github:OpenAgentsInc/openagents#6016-live-ref-${++refIndex}`;
  const rewriteFixtureStrings = (value, key = "") => {
    if (Array.isArray(value)) {
      return value.map((entry) => rewriteFixtureStrings(entry));
    }
    if (value !== null && typeof value === "object") {
      return Object.fromEntries(
        Object.entries(value).map(([entryKey, entryValue]) => [
          entryKey,
          rewriteFixtureStrings(entryValue, entryKey),
        ]),
      );
    }
    if (typeof value !== "string") {
      return value;
    }
    if (key === "evidenceMode") {
      return "live";
    }
    if (/\bfixture(?:$|[.:_-])/i.test(value)) {
      return liveRef();
    }
    return value;
  };

  const liveManifest = rewriteFixtureStrings(manifest);
  liveManifest.evidenceMode = "live";
  liveManifest.generatedAt = "2026-06-22T18:00:00.000Z";
  liveManifest.runs.forEach((run) => {
    run.evidenceMode = "live";
    run.blockerRefs = [];
    run.acceptedOutcome.blockerRefs = [];
    run.artifact.blockerRefs = [];
    run.settlement.blockerRefs = [];
    run.verse.blockerRefs = [];
    run.energy.blockerRefs = [];
    run.artifact.playableInWorldRef = liveRef();
    run.verse.playbackRef = liveRef();
    run.verse.sourceRefs = [run.acceptedOutcome.receiptRef];
    run.energy.kwhMeasured = 0.5;
    run.energy.measurementRef = liveRef();
  });

  const khalaRun = liveManifest.runs.find((run) => run.lane === "khala");
  khalaRun.coordinator.mode = "live_conductor";
  khalaRun.coordinator.promoted = true;
  khalaRun.settlement.settled = true;
  khalaRun.settlement.receiptRefs = [liveRef(), liveRef()];

  liveManifest.publication.status = "published";
  liveManifest.publication.publicationRef = liveRef();
  liveManifest.publication.blockerRefs = [];

  return liveManifest;
}

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
    expect(metrics.livePromotionAudit.status).toBe("blocked");
    expect(metrics.livePromotionAudit.fixtureRefPaths).toEqual([]);
    expect(metrics.livePromotionAudit.checks.map((check) => check.id)).toContain(
      "no_fixture_refs_in_live_manifest",
    );
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

  test("blocks live promotion when fixture refs remain in a live manifest", () => {
    const manifest = cloneFixtureManifest();
    manifest.evidenceMode = "live";
    manifest.runs.forEach((run) => {
      run.evidenceMode = "live";
    });

    const metrics = reduceKhalaHeadToHeadManifest(manifest);

    expect(metrics.livePromotionAudit.fixtureRefPaths.length).toBeGreaterThan(0);
    expect(metrics.livePromotionAudit.blockerRefs).toContain(
      "blocker.khala_demo.live_manifest_contains_fixture_refs",
    );
    expect(metrics.closureAudit.canClose).toBe(false);
  });

  test("allows closure only after every live promotion gate passes", () => {
    const metrics = reduceKhalaHeadToHeadManifest(livePromotionManifest());

    expect(metrics.livePromotionAudit.status).toBe("promotable");
    expect(metrics.livePromotionAudit.blockerRefs).toEqual([]);
    expect(metrics.livePromotionAudit.fixtureRefPaths).toEqual([]);
    expect(metrics.closureAudit.canClose).toBe(true);
  });
});
