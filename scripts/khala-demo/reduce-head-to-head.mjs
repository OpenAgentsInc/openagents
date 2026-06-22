#!/usr/bin/env bun

import { readFileSync } from "node:fs";

const MANIFEST_SCHEMA = "openagents.khala_head_to_head_evidence.v1";
const METRICS_SCHEMA = "openagents.khala_head_to_head_metrics.v1";
const VALID_TOP_LEVEL_MODES = new Set(["fixture_scaffold", "live"]);
const VALID_RUN_MODES = new Set(["fixture_scaffold", "live", "reported_external"]);
const VALID_LANES = new Set(["khala", "frontier_baseline", "reported_external"]);

function fail(message) {
  throw new Error(message);
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function requireRecord(value, path) {
  if (!isRecord(value)) {
    fail(`${path} must be an object`);
  }
  return value;
}

function requireArray(value, path) {
  if (!Array.isArray(value)) {
    fail(`${path} must be an array`);
  }
  return value;
}

function requireString(value, path) {
  if (typeof value !== "string" || value.trim().length === 0) {
    fail(`${path} must be a non-empty string`);
  }
  return value;
}

function requireFiniteNumber(value, path) {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    fail(`${path} must be a finite number`);
  }
  return value;
}

function optionalFiniteNumber(value, path) {
  if (value === null || value === undefined) {
    return null;
  }
  return requireFiniteNumber(value, path);
}

function requireBoolean(value, path) {
  if (typeof value !== "boolean") {
    fail(`${path} must be a boolean`);
  }
  return value;
}

function requireStringArray(value, path) {
  return requireArray(value, path).map((entry, index) =>
    requireString(entry, `${path}[${index}]`),
  );
}

function roundNumber(value, digits = 4) {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return null;
  }
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

function totalTokensFor(usage, path) {
  const totalTokens = optionalFiniteNumber(usage.totalTokens, `${path}.totalTokens`);
  if (totalTokens !== null) {
    return totalTokens;
  }
  const promptTokens = requireFiniteNumber(usage.promptTokens, `${path}.promptTokens`);
  const completionTokens = requireFiniteNumber(usage.completionTokens, `${path}.completionTokens`);
  return promptTokens + completionTokens;
}

function validatePublicSafeStrings(value, path = "$") {
  if (typeof value === "string") {
    const checks = [
      [/\/Users\//, "local absolute paths are not public-safe"],
      [/\.secrets\//, "secret-file refs are not public-safe"],
      [/-----BEGIN [A-Z ]*PRIVATE KEY-----/, "private key material is not public-safe"],
      [/\bsk-[A-Za-z0-9_-]{16,}\b/, "OpenAI-style API keys are not public-safe"],
      [/\bgh[pousr]_[A-Za-z0-9_]{20,}\b/, "GitHub token strings are not public-safe"],
      [/\b[A-Za-z0-9+/]{32,}={0,2}\s+mnemonic\b/i, "wallet material is not public-safe"],
    ];
    for (const [pattern, reason] of checks) {
      if (pattern.test(value)) {
        fail(`${path} is unsafe: ${reason}`);
      }
    }
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((entry, index) => validatePublicSafeStrings(entry, `${path}[${index}]`));
    return;
  }
  if (isRecord(value)) {
    for (const [key, entry] of Object.entries(value)) {
      validatePublicSafeStrings(entry, `${path}.${key}`);
    }
  }
}

function validateRun(rawRun, index) {
  const path = `runs[${index}]`;
  const run = requireRecord(rawRun, path);
  const runId = requireString(run.runId, `${path}.runId`);
  const lane = requireString(run.lane, `${path}.lane`);
  if (!VALID_LANES.has(lane)) {
    fail(`${path}.lane must be one of ${[...VALID_LANES].join(", ")}`);
  }

  const evidenceMode = requireString(run.evidenceMode, `${path}.evidenceMode`);
  if (!VALID_RUN_MODES.has(evidenceMode)) {
    fail(`${path}.evidenceMode must be one of ${[...VALID_RUN_MODES].join(", ")}`);
  }

  const usage = requireRecord(run.usage, `${path}.usage`);
  const acceptedOutcome = requireRecord(run.acceptedOutcome, `${path}.acceptedOutcome`);
  const artifact = requireRecord(run.artifact, `${path}.artifact`);
  const settlement = requireRecord(run.settlement, `${path}.settlement`);
  const verse = requireRecord(run.verse, `${path}.verse`);
  const energy = requireRecord(run.energy, `${path}.energy`);
  const coordinator = requireRecord(run.coordinator, `${path}.coordinator`);

  requireString(run.label, `${path}.label`);
  requireString(run.model, `${path}.model`);
  requireString(run.provider, `${path}.provider`);
  requireString(coordinator.mode, `${path}.coordinator.mode`);
  requireString(coordinator.policyRef, `${path}.coordinator.policyRef`);
  requireBoolean(coordinator.promoted, `${path}.coordinator.promoted`);
  requireString(run.startedAt, `${path}.startedAt`);
  requireString(run.completedAt, `${path}.completedAt`);
  requireFiniteNumber(run.wallClockMs, `${path}.wallClockMs`);
  requireFiniteNumber(run.costUsd, `${path}.costUsd`);
  requireFiniteNumber(run.costMsat, `${path}.costMsat`);
  requireFiniteNumber(run.priceMsat, `${path}.priceMsat`);
  requireBoolean(acceptedOutcome.accepted, `${path}.acceptedOutcome.accepted`);
  requireString(acceptedOutcome.verificationClass, `${path}.acceptedOutcome.verificationClass`);
  requireString(acceptedOutcome.verdictRef, `${path}.acceptedOutcome.verdictRef`);
  requireString(acceptedOutcome.verifierRef, `${path}.acceptedOutcome.verifierRef`);
  requireString(acceptedOutcome.receiptRef, `${path}.acceptedOutcome.receiptRef`);
  requireStringArray(acceptedOutcome.evidenceRefs, `${path}.acceptedOutcome.evidenceRefs`);
  requireStringArray(acceptedOutcome.blockerRefs, `${path}.acceptedOutcome.blockerRefs`);
  requireString(artifact.kind, `${path}.artifact.kind`);
  requireString(artifact.artifactRef, `${path}.artifact.artifactRef`);
  if (artifact.playableInWorldRef !== null) {
    requireString(artifact.playableInWorldRef, `${path}.artifact.playableInWorldRef`);
  }
  requireStringArray(artifact.blockerRefs, `${path}.artifact.blockerRefs`);
  requireBoolean(settlement.settled, `${path}.settlement.settled`);
  requireStringArray(settlement.receiptRefs, `${path}.settlement.receiptRefs`);
  requireStringArray(settlement.blockerRefs, `${path}.settlement.blockerRefs`);
  if (verse.playbackRef !== null) {
    requireString(verse.playbackRef, `${path}.verse.playbackRef`);
  }
  requireStringArray(verse.sourceRefs, `${path}.verse.sourceRefs`);
  requireFiniteNumber(verse.inWorldWorkUnits, `${path}.verse.inWorldWorkUnits`);
  requireFiniteNumber(verse.gatewayWorkUnits, `${path}.verse.gatewayWorkUnits`);
  requireStringArray(verse.blockerRefs, `${path}.verse.blockerRefs`);
  optionalFiniteNumber(energy.kwhMeasured, `${path}.energy.kwhMeasured`);
  if (energy.measurementRef !== null) {
    requireString(energy.measurementRef, `${path}.energy.measurementRef`);
  }
  requireStringArray(energy.blockerRefs, `${path}.energy.blockerRefs`);
  requireStringArray(run.sourceRefs, `${path}.sourceRefs`);
  requireStringArray(run.blockerRefs, `${path}.blockerRefs`);

  return run;
}

function validateManifest(manifest) {
  requireRecord(manifest, "$");
  if (manifest.schema !== MANIFEST_SCHEMA) {
    fail(`schema must be ${MANIFEST_SCHEMA}`);
  }
  requireString(manifest.manifestRef, "manifestRef");
  const evidenceMode = requireString(manifest.evidenceMode, "evidenceMode");
  if (!VALID_TOP_LEVEL_MODES.has(evidenceMode)) {
    fail(`evidenceMode must be one of ${[...VALID_TOP_LEVEL_MODES].join(", ")}`);
  }
  requireString(manifest.generatedAt, "generatedAt");
  requireRecord(manifest.scope, "scope");
  requireString(manifest.scope.issueRef, "scope.issueRef");
  requireString(manifest.scope.parentIssueRef, "scope.parentIssueRef");
  requireString(manifest.scope.roadmapRef, "scope.roadmapRef");
  requireString(manifest.scope.runbookRef, "scope.runbookRef");
  requireString(manifest.scope.prompt, "scope.prompt");
  requireString(manifest.scope.benchmarkRef, "scope.benchmarkRef");

  const runs = requireArray(manifest.runs, "runs").map(validateRun);
  if (runs.length === 0) {
    fail("runs must contain at least one run");
  }
  requireArray(manifest.externalReportedClaims, "externalReportedClaims").forEach((claim, index) => {
    const path = `externalReportedClaims[${index}]`;
    const record = requireRecord(claim, path);
    requireString(record.claimRef, `${path}.claimRef`);
    requireString(record.label, `${path}.label`);
    requireString(record.citationStatus, `${path}.citationStatus`);
    optionalFiniteNumber(record.tokens, `${path}.tokens`);
    optionalFiniteNumber(record.costUsd, `${path}.costUsd`);
    optionalFiniteNumber(record.wallClockMs, `${path}.wallClockMs`);
    requireString(record.verdictSummary, `${path}.verdictSummary`);
    requireStringArray(record.blockerRefs, `${path}.blockerRefs`);
  });

  const publication = requireRecord(manifest.publication, "publication");
  requireString(publication.status, "publication.status");
  if (publication.publicationRef !== null) {
    requireString(publication.publicationRef, "publication.publicationRef");
  }
  requireStringArray(publication.claimUpgradeRefs, "publication.claimUpgradeRefs");
  requireStringArray(publication.blockerRefs, "publication.blockerRefs");
  validatePublicSafeStrings(manifest);
  return manifest;
}

function metricForRun(run) {
  const tokens = totalTokensFor(run.usage, `run(${run.runId}).usage`);
  const accepted = run.acceptedOutcome.accepted;
  const inWorldUnits = run.verse.inWorldWorkUnits;
  const gatewayUnits = run.verse.gatewayWorkUnits;
  const totalWorkUnits = inWorldUnits + gatewayUnits;
  const kwhMeasured = run.energy.kwhMeasured;
  const acceptedOutcomes = accepted ? 1 : 0;

  return {
    runId: run.runId,
    lane: run.lane,
    label: run.label,
    model: run.model,
    provider: run.provider,
    evidenceMode: run.evidenceMode,
    coordinatorMode: run.coordinator.mode,
    tokens,
    dollars: roundNumber(run.costUsd, 6),
    costMsat: run.costMsat,
    priceMsat: run.priceMsat,
    wallClockMs: run.wallClockMs,
    verificationClass: run.acceptedOutcome.verificationClass,
    accepted,
    costPerAcceptedOutcomeUsd: accepted ? roundNumber(run.costUsd, 6) : "not_applicable",
    acceptedOutcomesPerKwh:
      kwhMeasured !== null && kwhMeasured > 0
        ? roundNumber(acceptedOutcomes / kwhMeasured, 6)
        : "not_measured",
    inWorldVsGatewaySplit:
      totalWorkUnits > 0
        ? {
            status: "measured_from_manifest_units",
            inWorldWorkUnits: inWorldUnits,
            gatewayWorkUnits: gatewayUnits,
            inWorldShare: roundNumber(inWorldUnits / totalWorkUnits, 6),
            gatewayShare: roundNumber(gatewayUnits / totalWorkUnits, 6),
          }
        : {
            status: "not_measured",
            inWorldWorkUnits: inWorldUnits,
            gatewayWorkUnits: gatewayUnits,
            inWorldShare: null,
            gatewayShare: null,
          },
    settlement: {
      settled: run.settlement.settled,
      receiptRefs: run.settlement.receiptRefs,
      blockerRefs: run.settlement.blockerRefs,
    },
    refs: {
      verdictRef: run.acceptedOutcome.verdictRef,
      verifierRef: run.acceptedOutcome.verifierRef,
      acceptedOutcomeReceiptRef: run.acceptedOutcome.receiptRef,
      artifactRef: run.artifact.artifactRef,
      playableInWorldRef: run.artifact.playableInWorldRef,
      versePlaybackRef: run.verse.playbackRef,
      energyMeasurementRef: run.energy.measurementRef,
      sourceRefs: run.sourceRefs,
    },
    blockerRefs: [
      ...run.blockerRefs,
      ...run.acceptedOutcome.blockerRefs,
      ...run.artifact.blockerRefs,
      ...run.settlement.blockerRefs,
      ...run.verse.blockerRefs,
      ...run.energy.blockerRefs,
    ],
  };
}

function unique(values) {
  return [...new Set(values.filter((value) => typeof value === "string" && value.length > 0))];
}

function collectFixtureRefPaths(value, path = "$", out = []) {
  if (typeof value === "string") {
    if (/\bfixture(?:$|[.:_-])/i.test(value)) {
      out.push(path);
    }
    return out;
  }
  if (Array.isArray(value)) {
    value.forEach((entry, index) => collectFixtureRefPaths(entry, `${path}[${index}]`, out));
    return out;
  }
  if (isRecord(value)) {
    for (const [key, entry] of Object.entries(value)) {
      collectFixtureRefPaths(entry, `${path}.${key}`, out);
    }
  }
  return out;
}

function promotionCheck(checks, id, passed, blockerRef, detail) {
  checks.push({
    id,
    passed,
    blockerRef: passed ? null : blockerRef,
    detail,
  });
}

function deriveLivePromotionAudit(manifest, metrics) {
  const khalaRun = metrics.scoreboard.find((run) => run.lane === "khala");
  const frontierRun = metrics.scoreboard.find((run) => run.lane === "frontier_baseline");
  const fixtureRefPaths =
    manifest.evidenceMode === "live" ? collectFixtureRefPaths(manifest) : [];
  const checks = [];

  promotionCheck(
    checks,
    "live_manifest",
    manifest.evidenceMode === "live",
    "blocker.khala_demo.fixture_scaffold_not_live",
    `manifest evidenceMode is ${manifest.evidenceMode}`,
  );
  promotionCheck(
    checks,
    "no_fixture_refs_in_live_manifest",
    manifest.evidenceMode !== "live" || fixtureRefPaths.length === 0,
    "blocker.khala_demo.live_manifest_contains_fixture_refs",
    fixtureRefPaths.length === 0
      ? "no fixture refs detected for live mode"
      : `${fixtureRefPaths.length} fixture ref path(s) remain`,
  );
  promotionCheck(
    checks,
    "khala_live_run",
    khalaRun !== undefined && khalaRun.evidenceMode === "live",
    "blocker.khala_demo.live_khala_run_missing",
    khalaRun === undefined ? "missing khala lane" : `khala evidenceMode is ${khalaRun.evidenceMode}`,
  );
  promotionCheck(
    checks,
    "openagents_khala_model",
    khalaRun !== undefined && khalaRun.model === "openagents/khala",
    "blocker.khala_demo.openagents_khala_model_missing",
    khalaRun === undefined ? "missing khala lane" : `khala model is ${khalaRun.model}`,
  );
  promotionCheck(
    checks,
    "khala_accepted_outcome",
    khalaRun !== undefined && khalaRun.accepted,
    "blocker.khala_demo.khala_accepted_outcome_missing",
    khalaRun === undefined ? "missing khala lane" : `khala accepted is ${String(khalaRun.accepted)}`,
  );
  promotionCheck(
    checks,
    "m7_live_conductor",
    khalaRun !== undefined && khalaRun.coordinatorMode === "live_conductor",
    "blocker.khala_demo.m7_live_conductor_missing",
    khalaRun === undefined ? "missing khala lane" : `coordinator mode is ${khalaRun.coordinatorMode}`,
  );
  promotionCheck(
    checks,
    "settlement_receipts",
    khalaRun !== undefined && khalaRun.settlement.settled && khalaRun.settlement.receiptRefs.length > 0,
    "blocker.khala_demo.settlement_receipts_missing",
    khalaRun === undefined
      ? "missing khala lane"
      : `${khalaRun.settlement.receiptRefs.length} settlement receipt ref(s)`,
  );
  promotionCheck(
    checks,
    "verse_playback",
    khalaRun !== undefined && khalaRun.refs.versePlaybackRef !== null,
    "blocker.khala_demo.verse_playback_missing",
    khalaRun === undefined ? "missing khala lane" : String(khalaRun.refs.versePlaybackRef),
  );
  promotionCheck(
    checks,
    "artifact_playable_in_world",
    khalaRun !== undefined && khalaRun.refs.playableInWorldRef !== null,
    "blocker.khala_demo.artifact_not_playable_in_world",
    khalaRun === undefined ? "missing khala lane" : String(khalaRun.refs.playableInWorldRef),
  );
  promotionCheck(
    checks,
    "energy_telemetry",
    khalaRun !== undefined && khalaRun.acceptedOutcomesPerKwh !== "not_measured",
    "blocker.khala_demo.energy_telemetry_missing",
    khalaRun === undefined ? "missing khala lane" : `AO/kWh is ${khalaRun.acceptedOutcomesPerKwh}`,
  );
  promotionCheck(
    checks,
    "frontier_live_run",
    frontierRun !== undefined && frontierRun.evidenceMode === "live",
    frontierRun === undefined
      ? "blocker.khala_demo.frontier_baseline_missing"
      : "blocker.khala_demo.frontier_baseline_not_live",
    frontierRun === undefined
      ? "missing frontier baseline lane"
      : `frontier evidenceMode is ${frontierRun.evidenceMode}`,
  );
  promotionCheck(
    checks,
    "publication_published",
    manifest.publication.status === "published" && manifest.publication.publicationRef !== null,
    "blocker.khala_demo.publication_missing",
    `publication status is ${manifest.publication.status}`,
  );
  promotionCheck(
    checks,
    "public_safety",
    metrics.publicSafety.blockerRefs.length === 0,
    "blocker.khala_demo.public_safety_failed",
    `${metrics.publicSafety.blockerRefs.length} public-safety blocker(s)`,
  );

  const blockerRefs = unique(checks.map((check) => check.blockerRef));

  return {
    status: blockerRefs.length === 0 ? "promotable" : "blocked",
    fixtureRefPaths,
    checks,
    blockerRefs,
  };
}

function deriveClosureAudit(manifest, livePromotionAudit) {
  return {
    issueRef: manifest.scope.issueRef,
    canClose: livePromotionAudit.blockerRefs.length === 0,
    requiredEvidence: [
      "live openagents/khala run",
      "live frontier baseline run",
      "accepted-outcome verifier receipt",
      "worker and validator settlement receipts",
      "Verse playback ref",
      "artifact playable in the three-effect world",
      "measured AO/kWh telemetry",
      "published comparison ref",
      "public-safe manifest",
    ],
    blockerRefs: livePromotionAudit.blockerRefs,
  };
}

export function reduceKhalaHeadToHeadManifest(rawManifest) {
  const manifest = validateManifest(rawManifest);
  const scoreboard = manifest.runs.map(metricForRun);
  const verifiedRuns = scoreboard.filter((run) => run.verificationClass !== "none");
  const verifiedRunCount = verifiedRuns.length;
  const verifiedAcceptedRunCount = verifiedRuns.filter((run) => run.accepted).length;
  const acceptedRunCount = scoreboard.filter((run) => run.accepted).length;
  const totalDollars = scoreboard.reduce((sum, run) => sum + run.dollars, 0);
  const totalTokens = scoreboard.reduce((sum, run) => sum + run.tokens, 0);
  const measuredEnergyRuns = manifest.runs.filter(
    (run) =>
      typeof run.energy.kwhMeasured === "number" &&
      Number.isFinite(run.energy.kwhMeasured) &&
      run.energy.kwhMeasured > 0,
  );
  const measuredKwh = measuredEnergyRuns.reduce((sum, run) => sum + run.energy.kwhMeasured, 0);
  const measuredEnergyAcceptedRunCount = measuredEnergyRuns.filter(
    (run) => run.acceptedOutcome.accepted,
  ).length;

  const metrics = {
    schema: METRICS_SCHEMA,
    manifestRef: manifest.manifestRef,
    evidenceMode: manifest.evidenceMode,
    generatedAt: new Date().toISOString(),
    scope: manifest.scope,
    summary: {
      runCount: scoreboard.length,
      acceptedRunCount,
      verifiedRate:
        verifiedRunCount > 0 ? roundNumber(verifiedAcceptedRunCount / verifiedRunCount, 6) : "not_measured",
      totalTokens,
      totalDollars: roundNumber(totalDollars, 6),
      acceptedOutcomesPerKwh:
        measuredKwh > 0 ? roundNumber(measuredEnergyAcceptedRunCount / measuredKwh, 6) : "not_measured",
    },
    scoreboard,
    externalReportedClaims: manifest.externalReportedClaims,
    publication: manifest.publication,
    publicSafety: {
      status: "passed",
      blockerRefs: [],
    },
  };

  const livePromotionAudit = deriveLivePromotionAudit(manifest, metrics);

  return {
    ...metrics,
    livePromotionAudit,
    closureAudit: deriveClosureAudit(manifest, livePromotionAudit),
  };
}

export function loadManifest(path) {
  const text = readFileSync(path, "utf8");
  return JSON.parse(text);
}

if (import.meta.main) {
  const manifestPath = process.argv[2];
  if (manifestPath === undefined) {
    console.error("usage: bun scripts/khala-demo/reduce-head-to-head.mjs <manifest.json>");
    process.exit(2);
  }
  const manifest = loadManifest(manifestPath);
  const metrics = reduceKhalaHeadToHeadManifest(manifest);
  console.log(JSON.stringify(metrics, null, 2));
}
