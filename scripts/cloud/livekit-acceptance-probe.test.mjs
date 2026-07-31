import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { buildProbeResult } from "./livekit-acceptance-probe.mjs";
import { projectConnectivityInventory } from "./livekit-connectivity-inventory.mjs";
import { buildPlan } from "./livekit-production-plan.mjs";
import { validateDeploymentBundle } from "./livekit-ops-policy.mjs";

const bundlePath = new URL("../../infra/livekit/bundle.json", import.meta.url);
const bundle = validateDeploymentBundle(JSON.parse(readFileSync(bundlePath, "utf8")));
const sourceBaseRevision = bundle.sourceBaseRevision;
const deployedRevision = "1".repeat(40);
const observedAt = new Date().toISOString();
const binding = { bundle, sourceBaseRevision, deployedRevision };
const digest = (character) => `sha256:${character.repeat(64)}`;
const loadObservedAt = "2026-07-31T12:10:30.000Z";

const scopeResult = (scope, index) => ({
  scope,
  state: "complete",
  objectCount: 1,
  bytesScanned: 100 + index,
  evidenceDigest: digest(String((index % 9) + 1)),
  findings: 0,
  rawMediaObjects: 0,
  transcriptObjects: 0,
});

const deployment = (name, image, hostNetwork = false) => ({
  metadata: { name },
  spec: {
    replicas: 3,
    template: {
      spec: {
        hostNetwork,
        containers: [{ image }],
      },
    },
  },
  status: { readyReplicas: 3 },
});

test("live connectivity inventory projects only readiness, address equality, and certificate state", () => {
  const inventory = projectConnectivityInventory({
    bundle,
    deployedRevision,
    packagedOmega: {
      schemaVersion: "openagents.omega_packaged_attestation.v1",
      observedAt,
      releaseSigned: true,
      launchSucceeded: true,
      artifactDigest: digest("a"),
    },
    serverDeployment: deployment("livekit-server", bundle.serverImage.reference, true),
    workerDeployment: deployment("sarah-livekit-agent", bundle.workerImage.reference),
    serverConfig: { data: { "config.yaml": "rtc:\n  use_external_ip: true\n" } },
    nodes: {
      items: Array.from({ length: 3 }, (_, index) => ({
        status: { addresses: [{ type: "ExternalIP", address: `address-${index}` }] },
      })),
    },
    signalingIngress: { status: { loadBalancer: { ingress: [{ ip: "signal-address" }] } } },
    turnService: { status: { loadBalancer: { ingress: [{ ip: "turn-address" }] } } },
    signalingCertificate: { status: { certificateStatus: "Active" } },
    turnCertificate: { status: { conditions: [{ type: "Ready", status: "True" }] } },
    signalingDns: new Set(["signal-address"]),
    turnDns: new Set(["turn-address"]),
    signalingTls: { authorized: true, unexpired: true },
    turnTls: { authorized: true, unexpired: true },
    observedAt,
  });
  assert.equal(inventory.runtime.digestPinnedImages, true);
  assert.equal(inventory.runtime.externalIpDiscoveryEnabled, true);
  assert.equal(inventory.network.signalingDnsMatchesAddress, true);
  assert.equal(inventory.network.turnTlsAuthorized, true);
  assert.ok(!JSON.stringify(inventory).includes("livekit.openagents.com"));
});

test("preflight derives infrastructure booleans from packaged and runtime inventory", () => {
  const capture = {
    schemaVersion: "openagents.livekit_connectivity_inventory.v1",
    sourceBaseRevision,
    deployedRevision,
    observedAt,
    packagedOmega: {
      releaseSigned: true,
      launchSucceeded: true,
      artifactDigest: digest("a"),
    },
    runtime: {
      serverDesiredReplicas: 3,
      serverReadyReplicas: 3,
      workerDesiredReplicas: 3,
      workerReadyReplicas: 3,
      digestPinnedImages: true,
      hostNetwork: true,
      externalIpDiscoveryEnabled: true,
      sfuNodeExternalAddressCount: 3,
    },
    network: {
      signalingAddressProvisioned: true,
      turnAddressProvisioned: true,
      signalingDnsMatchesAddress: true,
      turnDnsMatchesAddress: true,
      signalingTlsAuthorized: true,
      turnTlsAuthorized: true,
      signalingCertificateUnexpired: true,
      turnCertificateUnexpired: true,
    },
  };
  const probe = buildProbeResult({ step: "production_preflight", capture, ...binding });
  assert.equal(probe.phase, "connectivity");
  assert.deepEqual(probe.result, {
    packagedOmega: true,
    signaling: true,
    certificate: true,
    publicIpAdvertisement: true,
  });
  assert.equal(
    buildProbeResult({
      step: "production_preflight",
      capture: {
        ...capture,
        runtime: { ...capture.runtime, workerReadyReplicas: 2 },
      },
      ...binding,
    }).result.signaling,
    false,
  );
});

test("connectivity modes require forced path controls and calculate measured p95", () => {
  const capture = {
    schemaVersion: "openagents.livekit_connectivity_capture.v1",
    sourceBaseRevision,
    deployedRevision,
    observedAt,
    mode: "turn_tls",
    networkControl: { udpBlocked: true, nonTlsTcpBlocked: true },
    samples: [400, 900, 600].map((firstAudioMs, index) => ({
      selectedPath: "turn_tls",
      roomJoined: true,
      microphonePublished: true,
      sarahAudioSubscribed: true,
      sessionSettled: true,
      joinMs: 100 + index,
      firstAudioMs,
    })),
  };
  const probe = buildProbeResult({ step: "turn_tls", capture, ...binding });
  assert.equal(probe.result.p95JoinMs, 102);
  assert.equal(probe.result.p95FirstAudioMs, 900);
  assert.throws(
    () =>
      buildProbeResult({
        step: "turn_tls",
        capture: {
          ...capture,
          networkControl: { udpBlocked: true, nonTlsTcpBlocked: false },
        },
        ...binding,
      }),
    /required forced path/u,
  );
});

test("load result is derived from overlapping terminal sessions, cap refusal, and telemetry", () => {
  const sessions = Array.from({ length: 20 }, (_, index) => ({
    sessionRef: `livekit-load-session-ref://opaque-${index}`,
    startedAt: "2026-07-31T12:00:00.000Z",
    settledAt: "2026-07-31T12:10:00.000Z",
    terminal: true,
    firstAudioMs: 800 + index,
  }));
  const capture = {
    schemaVersion: "openagents.livekit_load_capture.v1",
    sourceBaseRevision,
    deployedRevision,
    observedAt: loadObservedAt,
    sessions,
    capAttempt: {
      observedAt: "2026-07-31T12:05:00.000Z",
      requestedRooms: 21,
      refused: true,
    },
    telemetrySamples: [0, 20, 20].map((activeRooms, index) => ({
      observedAt: `2026-07-31T12:0${index}:00.000Z`,
      activeRooms,
      capacityRooms: 25,
      sfuCpuPercent: 50 + index,
      workerCpuPercent: 60 + index,
      packetLossPercent: index / 10,
    })),
  };
  const probe = buildProbeResult({ step: "alpha_load", capture, ...binding });
  assert.equal(probe.result.concurrentRooms, 20);
  assert.equal(probe.result.spareCapacityPercent, 20);
  assert.equal(probe.result.settledSessions, 20);
  assert.equal(probe.result.maximumWorkerCpuPercent, 62);
  const unsettledSessions = structuredClone(capture.sessions);
  unsettledSessions[0].terminal = false;
  assert.throws(
    () =>
      buildProbeResult({
        step: "alpha_load",
        capture: {
          ...capture,
          sessions: unsettledSessions,
        },
        ...binding,
      }),
    /settledSessions|unsettled/u,
  );
});

test("privacy probe projects only the six runtime scopes from a passing eight-scope scan", () => {
  const scopes = [
    "packaged_omega",
    "packaged_clients",
    "pods",
    "logs",
    "redis",
    "object_storage",
    "traces",
    "crash_artifacts",
  ];
  const capture = {
    schemaVersion: "openagents.sarah.livekit_privacy_scan.v1",
    sourceBaseRevision,
    observedAt,
    outcome: "passed",
    results: {
      scopes,
      forbiddenPatternCount: 4,
      findings: 0,
      rawMediaObjects: 0,
      transcriptObjects: 0,
      scopeResults: scopes.map(scopeResult),
    },
  };
  const probe = buildProbeResult({ step: "runtime_secret_scan", capture, ...binding });
  assert.equal(probe.phase, "secret_scan");
  assert.deepEqual(probe.result.scopes, scopes.slice(2));
  assert.equal(probe.result.scopeResults.length, 6);
  assert.throws(
    () =>
      buildProbeResult({
        step: "runtime_secret_scan",
        capture: { ...capture, outcome: "failed" },
        ...binding,
      }),
    /not a passing capture/u,
  );
});

test("cost probe derives gross daily cost and requires all categories and active budget policy", () => {
  const categories = [
    "gke_control_plane",
    "sfu_compute",
    "worker_compute",
    "redis",
    "load_balancing_networking",
    "observability",
  ];
  const capture = {
    schemaVersion: "openagents.livekit_cost_capture.v1",
    sourceBaseRevision,
    deployedRevision,
    observedAt,
    fixedFloorMonthlyUsd: 1_500,
    forecastMonthlyGrossUsd: 1_900,
    billingRows: categories.flatMap((serviceCategory) =>
      ["2026-07-30", "2026-07-31"].map((usageDate) => ({
        usageDate,
        serviceCategory,
        grossCostUsd: 10,
        creditUsd: -2,
      })),
    ),
    budget: {
      active: true,
      currency: "USD",
      thresholds: [
        { percent: 0.5, basis: "CURRENT_SPEND" },
        { percent: 0.8, basis: "CURRENT_SPEND" },
        { percent: 1, basis: "FORECASTED_SPEND" },
      ],
      notificationChannelCount: 1,
      filterIncludesProject: true,
      filterIncludesLivekitLabel: true,
    },
  };
  const probe = buildProbeResult({ step: "billing_reconciliation", capture, ...binding });
  assert.equal(probe.result.observedDailyUsd, 60);
  assert.equal(probe.result.googleCreditsModeledAsZeroCost, false);
  assert.throws(
    () =>
      buildProbeResult({
        step: "billing_reconciliation",
        capture: {
          ...capture,
          billingRows: capture.billingRows.filter((row) => row.serviceCategory !== "observability"),
        },
        ...binding,
      }),
    /missing a required/u,
  );
});

test("plan generator emits exact collector commands for non-destructive phases", () => {
  const plan = buildPlan({
    phase: "connectivity",
    bundlePath: "/private/bundle.json",
    bundle,
    deployedRevision,
    resourceRefs: ["gke-cluster-ref://openagentsgemini/us-central1/oa-livekit-prod"],
    inputs: {
      production_preflight: "/private/preflight.json",
      direct_udp: "/private/direct.json",
      tcp_fallback: "/private/tcp.json",
      turn_tls: "/private/turn.json",
    },
  });
  assert.deepEqual(
    plan.steps.map((step) => step.id),
    ["production_preflight", "direct_udp", "tcp_fallback", "turn_tls"],
  );
  assert.ok(plan.steps.every((step) => step.command.includes(deployedRevision)));
  assert.throws(
    () =>
      buildPlan({
        phase: "drills",
        bundlePath: "/private/bundle.json",
        bundle,
        deployedRevision,
        resourceRefs: ["gke-cluster-ref://openagentsgemini/us-central1/oa-livekit-prod"],
        inputs: {},
      }),
    /only non-destructive/u,
  );
});
