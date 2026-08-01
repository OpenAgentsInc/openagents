#!/usr/bin/env node

import { lstatSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  validateCostObservation,
  validateDeploymentBundle,
  validateLoadObservation,
  validateSecretScanObservation,
} from "./livekit-ops-policy.mjs";

const PROBE_SCHEMA = "openagents.livekit_probe_result.v1";
const COMMIT = /^[0-9a-f]{40}$/u;
const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const RUNTIME_PRIVACY_SCOPES = Object.freeze([
  "pods",
  "logs",
  "redis",
  "object_storage",
  "traces",
  "crash_artifacts",
]);
const ALL_PRIVACY_SCOPES = Object.freeze([
  "packaged_omega",
  "packaged_clients",
  ...RUNTIME_PRIVACY_SCOPES,
]);
const COST_CATEGORIES = Object.freeze([
  "gke_control_plane",
  "sfu_compute",
  "worker_compute",
  "redis",
  "load_balancing_networking",
  "observability",
]);
const CONNECTIVITY_MODES = new Set(["direct_udp", "tcp_fallback", "turn_tls"]);

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-acceptance-probe.mjs \\
    --step production_preflight|direct_udp|tcp_fallback|turn_tls|alpha_load|runtime_secret_scan|billing_reconciliation \\
    --bundle infra/livekit/bundle.json \\
    --source-base-revision <40-hex> \\
    --deployed-revision <40-hex> \\
    --input <private-capture.json>

Reads a closed-schema private capture and emits one collector-compatible probe
result. It makes no network request and mutates no external state. Captures
must come from the named packaged-client, telemetry, privacy-scan, or billing
export procedures in the LiveKit production runbook.
`);
};

const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const exactKeys = (value, required, optional, label) => {
  assert(value && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) {
    assert(allowed.has(key), `${label} has unsupported field ${key}`);
  }
  for (const key of required) assert(Object.hasOwn(value, key), `${label} is missing ${key}`);
};

const finite = (value, minimum, maximum, label) => {
  assert(
    typeof value === "number" && Number.isFinite(value) && value >= minimum && value <= maximum,
    `${label} is outside ${minimum} through ${maximum}`,
  );
  return value;
};

const integer = (value, minimum, maximum, label) => {
  assert(
    Number.isSafeInteger(value) && value >= minimum && value <= maximum,
    `${label} is outside ${minimum} through ${maximum}`,
  );
  return value;
};

const timestamp = (value, label) => {
  assert(typeof value === "string" && Number.isFinite(Date.parse(value)), `${label} is invalid`);
  return value;
};

const readJson = (path, label) => {
  try {
    return JSON.parse(readFileSync(resolve(path), "utf8"));
  } catch (error) {
    throw new Error(`${label} is not readable JSON`, { cause: error });
  }
};

const assertPrivateInput = (path) => {
  const input = lstatSync(resolve(path));
  assert(
    input.isFile() && !input.isSymbolicLink() && (input.mode & 0o077) === 0,
    "private capture must be a mode-0600-or-stricter regular file",
  );
};

const parseArgs = (args) => {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    if (
      !["--bundle", "--deployed-revision", "--input", "--source-base-revision", "--step"].includes(
        argument,
      )
    ) {
      throw new Error(`unsupported argument ${argument}`);
    }
    const value = args[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    parsed[
      {
        "--bundle": "bundle",
        "--deployed-revision": "deployedRevision",
        "--input": "input",
        "--source-base-revision": "sourceBaseRevision",
        "--step": "step",
      }[argument]
    ] = value;
    index += 1;
  }
  for (const key of ["bundle", "deployedRevision", "input", "sourceBaseRevision", "step"]) {
    if (!parsed[key]) throw new Error(`missing required argument ${key}`);
  }
  if (!COMMIT.test(parsed.sourceBaseRevision) || !COMMIT.test(parsed.deployedRevision)) {
    throw new Error("source and deployed revisions must be full Git revisions");
  }
  return parsed;
};

const validateCaptureBinding = (capture, schema, sourceBaseRevision, deployedRevision) => {
  assert(capture.schemaVersion === schema, "capture schema is unsupported");
  assert(capture.sourceBaseRevision === sourceBaseRevision, "capture source revision drifted");
  assert(capture.deployedRevision === deployedRevision, "capture deployed revision drifted");
  timestamp(capture.observedAt, "capture observedAt");
};

const percentile95 = (values) => {
  const sorted = values.toSorted((left, right) => left - right);
  return sorted[Math.max(0, Math.ceil(sorted.length * 0.95) - 1)];
};

const observationEnvelope = (phase, sourceBaseRevision, deployedRevision, observedAt, results) => ({
  schemaVersion: "openagents.livekit_acceptance_observation.v1",
  phase,
  stage: "production",
  sourceBaseRevision,
  deployedRevision,
  resourceRefs: ["livekit-probe-ref://production/read-only"],
  startedAt: observedAt,
  settledAt: observedAt,
  results,
});

const projectPreflight = (capture, binding) => {
  exactKeys(
    capture,
    [
      "schemaVersion",
      "sourceBaseRevision",
      "deployedRevision",
      "observedAt",
      "packagedOmega",
      "runtime",
      "network",
    ],
    [],
    "preflight capture",
  );
  validateCaptureBinding(
    capture,
    "openagents.livekit_connectivity_inventory.v1",
    binding.sourceBaseRevision,
    binding.deployedRevision,
  );
  exactKeys(
    capture.packagedOmega,
    ["releaseSigned", "launchSucceeded", "artifactDigest"],
    [],
    "packagedOmega",
  );
  assert(DIGEST.test(capture.packagedOmega.artifactDigest), "packaged Omega digest is invalid");
  exactKeys(
    capture.runtime,
    [
      "serverDesiredReplicas",
      "serverReadyReplicas",
      "workerDesiredReplicas",
      "workerReadyReplicas",
      "digestPinnedImages",
      "hostNetwork",
      "externalIpDiscoveryEnabled",
      "sfuNodeExternalAddressCount",
    ],
    [],
    "runtime",
  );
  exactKeys(
    capture.network,
    [
      "signalingAddressProvisioned",
      "turnAddressProvisioned",
      "signalingDnsMatchesAddress",
      "turnDnsMatchesAddress",
      "signalingTlsAuthorized",
      "turnTlsAuthorized",
      "signalingCertificateUnexpired",
      "turnCertificateUnexpired",
    ],
    [],
    "network",
  );
  const runtimeReady =
    integer(capture.runtime.serverDesiredReplicas, 3, 100, "server desired replicas") ===
      integer(capture.runtime.serverReadyReplicas, 0, 100, "server ready replicas") &&
    integer(capture.runtime.workerDesiredReplicas, 3, 100, "worker desired replicas") ===
      integer(capture.runtime.workerReadyReplicas, 0, 100, "worker ready replicas") &&
    capture.runtime.digestPinnedImages === true;
  const signaling =
    runtimeReady &&
    capture.network.signalingAddressProvisioned === true &&
    capture.network.signalingDnsMatchesAddress === true;
  const certificate =
    capture.network.signalingTlsAuthorized === true &&
    capture.network.turnTlsAuthorized === true &&
    capture.network.signalingCertificateUnexpired === true &&
    capture.network.turnCertificateUnexpired === true;
  const publicIpAdvertisement =
    capture.runtime.hostNetwork === true &&
    capture.runtime.externalIpDiscoveryEnabled === true &&
    integer(capture.runtime.sfuNodeExternalAddressCount, 3, 100, "SFU external address count") >=
      capture.runtime.serverReadyReplicas &&
    capture.network.turnAddressProvisioned === true &&
    capture.network.turnDnsMatchesAddress === true;
  return {
    packagedOmega:
      capture.packagedOmega.releaseSigned === true &&
      capture.packagedOmega.launchSucceeded === true,
    signaling,
    certificate,
    publicIpAdvertisement,
  };
};

const projectConnectivityMode = (capture, step, binding) => {
  exactKeys(
    capture,
    [
      "schemaVersion",
      "sourceBaseRevision",
      "deployedRevision",
      "observedAt",
      "mode",
      "networkControl",
      "samples",
    ],
    [],
    "connectivity capture",
  );
  validateCaptureBinding(
    capture,
    "openagents.livekit_connectivity_capture.v1",
    binding.sourceBaseRevision,
    binding.deployedRevision,
  );
  assert(CONNECTIVITY_MODES.has(step) && capture.mode === step, "connectivity mode drifted");
  exactKeys(capture.networkControl, ["udpBlocked", "nonTlsTcpBlocked"], [], "networkControl");
  const expectedControl = {
    direct_udp: { udpBlocked: false, nonTlsTcpBlocked: false },
    tcp_fallback: { udpBlocked: true, nonTlsTcpBlocked: false },
    turn_tls: { udpBlocked: true, nonTlsTcpBlocked: true },
  }[step];
  assert(
    capture.networkControl.udpBlocked === expectedControl.udpBlocked &&
      capture.networkControl.nonTlsTcpBlocked === expectedControl.nonTlsTcpBlocked,
    `${step} network control is not the required forced path`,
  );
  assert(
    Array.isArray(capture.samples) && capture.samples.length >= 3 && capture.samples.length <= 100,
    "connectivity capture requires 3 through 100 samples",
  );
  for (const [index, sample] of capture.samples.entries()) {
    exactKeys(
      sample,
      [
        "selectedPath",
        "roomJoined",
        "microphonePublished",
        "sarahAudioSubscribed",
        "sessionSettled",
        "joinMs",
        "firstAudioMs",
      ],
      [],
      `samples[${index}]`,
    );
    assert(sample.selectedPath === step, `samples[${index}] selected the wrong path`);
    for (const key of [
      "roomJoined",
      "microphonePublished",
      "sarahAudioSubscribed",
      "sessionSettled",
    ]) {
      assert(sample[key] === true, `samples[${index}].${key} did not pass`);
    }
    finite(sample.joinMs, 0, 120_000, `samples[${index}].joinMs`);
    finite(sample.firstAudioMs, 0, 120_000, `samples[${index}].firstAudioMs`);
  }
  return {
    roomJoined: true,
    microphonePublished: true,
    sarahAudioSubscribed: true,
    selectedPathObserved: true,
    sessionSettled: true,
    p95JoinMs: percentile95(capture.samples.map((sample) => sample.joinMs)),
    p95FirstAudioMs: percentile95(capture.samples.map((sample) => sample.firstAudioMs)),
  };
};

const maximumOverlap = (sessions) => {
  const events = sessions.flatMap((session) => [
    { at: Date.parse(session.startedAt), delta: 1 },
    { at: Date.parse(session.settledAt), delta: -1 },
  ]);
  events.sort((left, right) => left.at - right.at || right.delta - left.delta);
  let active = 0;
  let maximum = 0;
  for (const event of events) {
    active += event.delta;
    maximum = Math.max(maximum, active);
  }
  return maximum;
};

const projectLoad = (capture, bundle, binding) => {
  exactKeys(
    capture,
    [
      "schemaVersion",
      "sourceBaseRevision",
      "deployedRevision",
      "observedAt",
      "sessions",
      "capAttempt",
      "telemetrySamples",
    ],
    [],
    "load capture",
  );
  validateCaptureBinding(
    capture,
    "openagents.livekit_load_capture.v1",
    binding.sourceBaseRevision,
    binding.deployedRevision,
  );
  assert(
    Array.isArray(capture.sessions) &&
      capture.sessions.length >= bundle.limits.maxConcurrentSarahRooms &&
      capture.sessions.length <= 10_000,
    "load capture does not contain the admitted room cohort",
  );
  const sessionRefs = new Set();
  for (const [index, session] of capture.sessions.entries()) {
    exactKeys(
      session,
      ["sessionRef", "startedAt", "settledAt", "terminal", "firstAudioMs"],
      [],
      `sessions[${index}]`,
    );
    assert(
      typeof session.sessionRef === "string" &&
        session.sessionRef.startsWith("livekit-load-session-ref://") &&
        !sessionRefs.has(session.sessionRef),
      `sessions[${index}] has an invalid or duplicate opaque ref`,
    );
    sessionRefs.add(session.sessionRef);
    const startedAt = Date.parse(timestamp(session.startedAt, `sessions[${index}].startedAt`));
    const settledAt = Date.parse(timestamp(session.settledAt, `sessions[${index}].settledAt`));
    assert(settledAt >= startedAt, `sessions[${index}] timestamps regress`);
    finite(session.firstAudioMs, 0, 120_000, `sessions[${index}].firstAudioMs`);
    assert(typeof session.terminal === "boolean", `sessions[${index}].terminal is invalid`);
  }
  const earliestSessionStart = Math.min(
    ...capture.sessions.map((session) => Date.parse(session.startedAt)),
  );
  const latestSessionSettlement = Math.max(
    ...capture.sessions.map((session) => Date.parse(session.settledAt)),
  );
  const captureObservedAt = Date.parse(capture.observedAt);
  assert(
    captureObservedAt >= latestSessionSettlement &&
      captureObservedAt - latestSessionSettlement <= 5 * 60_000,
    "load capture was not sealed immediately after the measured sessions",
  );
  const concurrentRooms = maximumOverlap(capture.sessions);
  assert(
    concurrentRooms >= bundle.limits.maxConcurrentSarahRooms,
    "load sessions never overlapped at the admitted concurrency",
  );
  exactKeys(
    capture.capAttempt,
    ["observedAt", "requestedRooms", "responseStatus", "refusalCode"],
    [],
    "capAttempt",
  );
  const capAttemptObservedAt = Date.parse(
    timestamp(capture.capAttempt.observedAt, "capAttempt.observedAt"),
  );
  assert(
    capture.capAttempt.requestedRooms === bundle.limits.maxConcurrentSarahRooms + 1 &&
      capture.capAttempt.responseStatus === 409 &&
      capture.capAttempt.refusalCode === "sarah_voice_livekit_capacity_limit" &&
      capAttemptObservedAt >= earliestSessionStart &&
      capAttemptObservedAt <= latestSessionSettlement,
    "load cap attempt did not prove the typed refusal immediately above the admitted cap",
  );
  assert(
    Array.isArray(capture.telemetrySamples) &&
      capture.telemetrySamples.length >= 3 &&
      capture.telemetrySamples.length <= 10_000,
    "load capture requires 3 through 10000 telemetry samples",
  );
  for (const [index, sample] of capture.telemetrySamples.entries()) {
    exactKeys(
      sample,
      [
        "observedAt",
        "activeRooms",
        "capacityRooms",
        "sfuCpuPercent",
        "workerCpuPercent",
        "packetLossPercent",
      ],
      [],
      `telemetrySamples[${index}]`,
    );
    const sampleObservedAt = Date.parse(
      timestamp(sample.observedAt, `telemetrySamples[${index}].observedAt`),
    );
    assert(
      sampleObservedAt >= earliestSessionStart && sampleObservedAt <= latestSessionSettlement,
      `telemetrySamples[${index}] is outside the load window`,
    );
    integer(sample.activeRooms, 0, 10_000, `telemetrySamples[${index}].activeRooms`);
    integer(sample.capacityRooms, 1, 10_000, `telemetrySamples[${index}].capacityRooms`);
    assert(
      sample.capacityRooms >= sample.activeRooms,
      `telemetrySamples[${index}] capacity is below active rooms`,
    );
    finite(sample.sfuCpuPercent, 0, 100, `telemetrySamples[${index}].sfuCpuPercent`);
    finite(sample.workerCpuPercent, 0, 100, `telemetrySamples[${index}].workerCpuPercent`);
    finite(sample.packetLossPercent, 0, 100, `telemetrySamples[${index}].packetLossPercent`);
  }
  assert(
    capture.telemetrySamples.some(
      (sample) => sample.activeRooms >= bundle.limits.maxConcurrentSarahRooms,
    ),
    "load telemetry never observed the admitted concurrency",
  );
  const results = {
    concurrentRooms,
    spareCapacityPercent: Math.min(
      ...capture.telemetrySamples.map(
        (sample) => ((sample.capacityRooms - sample.activeRooms) / sample.capacityRooms) * 100,
      ),
    ),
    capEnforced: true,
    settledSessions: capture.sessions.filter((session) => session.terminal).length,
    unsettledSessions: capture.sessions.filter((session) => !session.terminal).length,
    maximumSfuCpuPercent: Math.max(
      ...capture.telemetrySamples.map((sample) => sample.sfuCpuPercent),
    ),
    maximumWorkerCpuPercent: Math.max(
      ...capture.telemetrySamples.map((sample) => sample.workerCpuPercent),
    ),
    maximumPacketLossPercent: Math.max(
      ...capture.telemetrySamples.map((sample) => sample.packetLossPercent),
    ),
    p95FirstAudioMs: percentile95(capture.sessions.map((session) => session.firstAudioMs)),
  };
  validateLoadObservation(
    observationEnvelope(
      "load",
      binding.sourceBaseRevision,
      binding.deployedRevision,
      capture.observedAt,
      results,
    ),
    bundle,
  );
  return results;
};

const projectSecretScan = (capture, binding) => {
  exactKeys(
    capture,
    ["schemaVersion", "sourceBaseRevision", "observedAt", "outcome", "results"],
    [],
    "privacy scan",
  );
  assert(
    capture.schemaVersion === "openagents.sarah.livekit_privacy_scan.v1" &&
      capture.sourceBaseRevision === binding.sourceBaseRevision &&
      capture.outcome === "passed",
    "privacy scan is not a passing capture for the deployed source",
  );
  timestamp(capture.observedAt, "privacy scan observedAt");
  assert(
    Array.isArray(capture.results.scopes) &&
      JSON.stringify(capture.results.scopes.toSorted()) ===
        JSON.stringify([...ALL_PRIVACY_SCOPES].toSorted()) &&
      Array.isArray(capture.results.scopeResults) &&
      JSON.stringify(capture.results.scopeResults.map((result) => result.scope).toSorted()) ===
        JSON.stringify([...ALL_PRIVACY_SCOPES].toSorted()),
    "privacy scan does not contain the exact eight-scope evidence set",
  );
  const scopeResults = capture.results.scopeResults.filter((result) =>
    RUNTIME_PRIVACY_SCOPES.includes(result.scope),
  );
  const results = {
    scopes: [...RUNTIME_PRIVACY_SCOPES],
    forbiddenPatternCount: capture.results.forbiddenPatternCount,
    findings: scopeResults.reduce((total, result) => total + result.findings, 0),
    rawMediaObjects: scopeResults.reduce((total, result) => total + result.rawMediaObjects, 0),
    transcriptObjects: scopeResults.reduce((total, result) => total + result.transcriptObjects, 0),
    scopeResults,
  };
  validateSecretScanObservation(
    observationEnvelope(
      "secret_scan",
      binding.sourceBaseRevision,
      binding.deployedRevision,
      capture.observedAt,
      results,
    ),
  );
  return results;
};

const projectCost = (capture, bundle, binding) => {
  exactKeys(
    capture,
    [
      "schemaVersion",
      "sourceBaseRevision",
      "deployedRevision",
      "observedAt",
      "fixedFloorMonthlyUsd",
      "forecastMonthlyGrossUsd",
      "billingRows",
      "budget",
    ],
    [],
    "cost capture",
  );
  validateCaptureBinding(
    capture,
    "openagents.livekit_cost_capture.v1",
    binding.sourceBaseRevision,
    binding.deployedRevision,
  );
  finite(capture.fixedFloorMonthlyUsd, 0, 1_000_000, "fixed floor");
  finite(capture.forecastMonthlyGrossUsd, 0, 1_000_000, "monthly gross forecast");
  assert(
    Array.isArray(capture.billingRows) &&
      capture.billingRows.length > 0 &&
      capture.billingRows.length <= 100_000,
    "cost capture requires billing export rows",
  );
  const categories = new Set();
  const dates = new Set();
  let grossCost = 0;
  for (const [index, row] of capture.billingRows.entries()) {
    exactKeys(
      row,
      ["usageDate", "serviceCategory", "grossCostUsd", "creditUsd"],
      [],
      `rows[${index}]`,
    );
    assert(/^\d{4}-\d{2}-\d{2}$/u.test(row.usageDate), `rows[${index}].usageDate is invalid`);
    assert(
      [...COST_CATEGORIES, "other_livekit"].includes(row.serviceCategory),
      `rows[${index}].serviceCategory is unsupported`,
    );
    finite(row.grossCostUsd, 0, 1_000_000, `rows[${index}].grossCostUsd`);
    finite(row.creditUsd, -1_000_000, 0, `rows[${index}].creditUsd`);
    categories.add(row.serviceCategory);
    dates.add(row.usageDate);
    grossCost += row.grossCostUsd;
  }
  assert(
    COST_CATEGORIES.every((category) => categories.has(category)),
    "billing export is missing a required LiveKit cost category",
  );
  exactKeys(
    capture.budget,
    [
      "active",
      "currency",
      "thresholds",
      "notificationChannelCount",
      "filterIncludesProject",
      "filterIncludesLivekitLabel",
    ],
    [],
    "budget",
  );
  assert(capture.budget.currency === "USD", "budget currency is not USD");
  const expectedThresholds = [
    { percent: 0.5, basis: "CURRENT_SPEND" },
    { percent: 0.8, basis: "CURRENT_SPEND" },
    { percent: 1, basis: "FORECASTED_SPEND" },
  ];
  assert(
    JSON.stringify(capture.budget.thresholds) === JSON.stringify(expectedThresholds),
    "budget thresholds do not match the production policy",
  );
  const results = {
    fixedFloorMonthlyUsd: capture.fixedFloorMonthlyUsd,
    forecastMonthlyUsd: capture.forecastMonthlyGrossUsd,
    observedDailyUsd: grossCost / dates.size,
    budgetAlertsActive:
      capture.budget.active === true &&
      integer(capture.budget.notificationChannelCount, 1, 100, "notification channels") > 0 &&
      capture.budget.filterIncludesProject === true &&
      capture.budget.filterIncludesLivekitLabel === true,
    roomCap: bundle.limits.maxConcurrentSarahRooms,
    googleCreditsModeledAsZeroCost: false,
    openAiIncludedInGoogleCost: false,
  };
  validateCostObservation(
    observationEnvelope(
      "cost",
      binding.sourceBaseRevision,
      binding.deployedRevision,
      capture.observedAt,
      results,
    ),
    bundle,
  );
  return results;
};

export const buildProbeResult = ({
  step,
  capture,
  bundle,
  sourceBaseRevision,
  deployedRevision,
}) => {
  assert(bundle.sourceBaseRevision === sourceBaseRevision, "bundle source revision drifted");
  const binding = { sourceBaseRevision, deployedRevision };
  let phase;
  let result;
  if (step === "production_preflight") {
    phase = "connectivity";
    result = projectPreflight(capture, binding);
  } else if (CONNECTIVITY_MODES.has(step)) {
    phase = "connectivity";
    result = projectConnectivityMode(capture, step, binding);
  } else if (step === "alpha_load") {
    phase = "load";
    result = projectLoad(capture, bundle, binding);
  } else if (step === "runtime_secret_scan") {
    phase = "secret_scan";
    result = projectSecretScan(capture, binding);
  } else if (step === "billing_reconciliation") {
    phase = "cost";
    result = projectCost(capture, bundle, binding);
  } else {
    throw new Error(`unsupported non-destructive probe step ${step}`);
  }
  return {
    schemaVersion: PROBE_SCHEMA,
    phase,
    stepId: step,
    observedAt: capture.observedAt,
    result,
  };
};

const run = () => {
  const args = parseArgs(process.argv.slice(2));
  const bundle = validateDeploymentBundle(readJson(args.bundle, "deployment bundle"));
  assertPrivateInput(args.input);
  const result = buildProbeResult({
    step: args.step,
    capture: readJson(args.input, "private capture"),
    bundle,
    sourceBaseRevision: args.sourceBaseRevision,
    deployedRevision: args.deployedRevision,
  });
  process.stdout.write(`${JSON.stringify(result)}\n`);
};

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) {
  try {
    run();
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    usage();
    process.exitCode = 1;
  }
}
