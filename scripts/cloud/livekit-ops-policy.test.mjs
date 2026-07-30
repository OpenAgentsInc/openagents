import assert from "node:assert/strict";
import test from "node:test";
import {
  LIVEKIT_OPS,
  REQUIRED_DRILLS,
  assertPublicSafe,
  buildPublicReceipt,
  sha256,
  validateCostObservation,
  validateAddonLock,
  validateConnectivityObservation,
  validateDeploymentBundle,
  validateDrillObservation,
  validateLoadObservation,
  validatePrerequisiteReceipt,
  validateRollbackObservation,
  validateSecretScanObservation,
  validateSourceOnlyReceipt,
} from "./livekit-ops-policy.mjs";

const digest = (character) => `sha256:${character.repeat(64)}`;
const deployedRevision = "1".repeat(40);

const bundle = {
  schemaVersion: "openagents.livekit_deployment_bundle.v1",
  project: LIVEKIT_OPS.project,
  region: LIVEKIT_OPS.region,
  zones: [...LIVEKIT_OPS.zones],
  release: LIVEKIT_OPS.release,
  namespace: LIVEKIT_OPS.namespace,
  sourceBaseRevision: LIVEKIT_OPS.sourceBaseRevision,
  chart: {
    version: LIVEKIT_OPS.chartVersion,
    sourceCommit: LIVEKIT_OPS.chartSourceCommit,
    archiveSha256: LIVEKIT_OPS.chartArchiveDigest.slice("sha256:".length),
  },
  serverImage: {
    reference: LIVEKIT_OPS.serverImage,
    digest: LIVEKIT_OPS.serverImageDigest,
    sourceCommit: LIVEKIT_OPS.serverImageSourceCommit,
  },
  configurationDigest: digest("4"),
  renderedManifestDigest: digest("6"),
  resources: {
    cluster: "oa-livekit-prod",
    sfuNodePool: "oa-livekit-prod-sfu",
    agentNodePool: "oa-livekit-prod-app",
    redis: "oa-livekit-redis",
    signalingAddress: "oa-livekit-prod-signal",
    turnAddress: "oa-livekit-prod-turn",
    signalingService: "livekit-server",
    turnService: "livekit-server-turn",
    serverKsa: "livekit-server",
    serverGsa: "oa-livekit-server@openagentsgemini.iam.gserviceaccount.com",
  },
  secrets: [
    "livekit-server-keys",
    "livekit-redis-auth",
    "livekit-turn-tls",
    "cloudflare-dns-token",
  ],
  googleSecretContainers: [
    "oa-livekit-prod-server-keys",
    "oa-livekit-prod-redis-auth",
    "oa-livekit-prod-cloudflare-dns",
    "oa-livekit-prod-openai-api-key",
  ],
  manifests: [{ path: "infra/livekit/production/resources/server.yaml", sha256: digest("5") }],
  limits: {
    maxConcurrentSarahRooms: 20,
    maxOwnerPrivateRooms: 1,
    maxCommunityRoomsPerCommunity: 2,
    idleTimeoutSeconds: 120,
    maxRoomLifetimeSeconds: 1_800,
  },
  sourceState: "source_only",
  pendingDependencies: ["packaged_omega_acceptance", "sarah_worker_acceptance"],
};

const addonLock = {
  schemaVersion: "openagents.livekit_addon_pins.v1",
  certManager: {
    repository: "https://charts.jetstack.io",
    chart: "cert-manager",
    version: LIVEKIT_OPS.certManagerVersion,
    chartSha256: LIVEKIT_OPS.certManagerChartDigest.slice("sha256:".length),
    resourceApiVersion: "cert-manager.io/v1",
  },
  externalSecrets: {
    repository: "https://charts.external-secrets.io",
    chart: "external-secrets",
    version: LIVEKIT_OPS.externalSecretsVersion,
    chartSha256: LIVEKIT_OPS.externalSecretsChartDigest.slice("sha256:".length),
    resourceApiVersion: "external-secrets.io/v1",
  },
  managedPrometheus: {
    delivery: "gke-managed-collection",
    resourceApiVersion: "monitoring.googleapis.com/v1",
    binaryVersionAuthority: "pinned-gke-cluster-version",
  },
};

const envelope = (phase, results) => ({
  schemaVersion: "openagents.livekit_acceptance_observation.v1",
  phase,
  stage: "production",
  sourceBaseRevision: LIVEKIT_OPS.sourceBaseRevision,
  deployedRevision,
  startedAt: "2026-07-30T21:00:00.000Z",
  settledAt: "2026-07-30T21:05:00.000Z",
  resourceRefs: ["gcp-resource-ref://livekit/production"],
  results,
  notes: ["Live proof must remain linked to the private operator evidence by digest."],
});

test("deployment bundle pins the exact project, region, resources, and caps", () => {
  assert.equal(validateDeploymentBundle(bundle), bundle);
  assert.throws(
    () => validateDeploymentBundle({ ...bundle, project: "another-project" }),
    /project must be openagentsgemini/u,
  );
  assert.throws(
    () =>
      validateDeploymentBundle({
        ...bundle,
        limits: { ...bundle.limits, maxConcurrentSarahRooms: 200 },
      }),
    /maxConcurrentSarahRooms must be 20/u,
  );
  assert.throws(
    () =>
      validateDeploymentBundle({
        ...bundle,
        resources: { ...bundle.resources, cluster: "unrelated-cluster" },
      }),
    /cluster must be oa-livekit-prod/u,
  );
});

test("addon lock pins controllers, API versions, and chart archive digests", () => {
  assert.equal(validateAddonLock(addonLock), addonLock);
  assert.throws(
    () =>
      validateAddonLock({
        ...addonLock,
        certManager: { ...addonLock.certManager, chartSha256: "0".repeat(64) },
      }),
    /cert-manager addon digest is not admitted/u,
  );
  assert.throws(
    () =>
      validateAddonLock({
        ...addonLock,
        externalSecrets: { ...addonLock.externalSecrets, version: "latest" },
      }),
    /External Secrets addon coordinates are not admitted/u,
  );
});

test("connectivity requires packaged Omega and all three observed ICE paths", () => {
  const observation = envelope("connectivity", {
    packagedOmega: true,
    signaling: true,
    certificate: true,
    publicIpAdvertisement: true,
    modes: ["direct_udp", "tcp_fallback", "turn_tls"].map((mode) => ({
      mode,
      roomJoined: true,
      microphonePublished: true,
      sarahAudioSubscribed: true,
      selectedPathObserved: true,
      sessionSettled: true,
      p95JoinMs: 500,
      p95FirstAudioMs: 900,
    })),
  });
  assert.equal(validateConnectivityObservation(observation), observation);
  const missingTurn = structuredClone(observation);
  missingTurn.results.modes.pop();
  assert.throws(() => validateConnectivityObservation(missingTurn), /exactly three modes/u);
  const unobservedPath = structuredClone(observation);
  unobservedPath.results.modes[0].selectedPathObserved = false;
  assert.throws(() => validateConnectivityObservation(unobservedPath), /selectedPathObserved/u);
});

test("load acceptance enforces admitted concurrency, spare capacity, and settlement", () => {
  const observation = envelope("load", {
    concurrentRooms: 20,
    spareCapacityPercent: 25,
    capEnforced: true,
    settledSessions: 20,
    unsettledSessions: 0,
    maximumSfuCpuPercent: 65,
    maximumWorkerCpuPercent: 70,
    maximumPacketLossPercent: 1.2,
    p95FirstAudioMs: 1_500,
  });
  assert.equal(validateLoadObservation(observation, bundle), observation);
  assert.throws(
    () =>
      validateLoadObservation(
        envelope("load", { ...observation.results, concurrentRooms: 19 }),
        bundle,
      ),
    /concurrentRooms/u,
  );
  assert.throws(
    () =>
      validateLoadObservation(
        envelope("load", { ...observation.results, unsettledSessions: 1 }),
        bundle,
      ),
    /unsettled/u,
  );
});

test("failure drills require bounded visible outcomes without speech continuity claims", () => {
  const observation = envelope("drills", {
    drills: REQUIRED_DRILLS.map((scenario) => ({
      scenario,
      outcome: "bounded_failure",
      visibleFailure: true,
      noProviderOverlap: true,
      accountingTerminal: true,
      freshAdmissionRequired: true,
      uninterruptedSpeechClaimed: false,
    })),
  });
  assert.equal(validateDrillObservation(observation), observation);
  const unsupportedClaim = structuredClone(observation);
  unsupportedClaim.results.drills[0].uninterruptedSpeechClaimed = true;
  assert.throws(
    () => validateDrillObservation(unsupportedClaim),
    /unsupported uninterrupted-speech/u,
  );
});

test("secret scan covers every persistence surface and rejects findings", () => {
  const observation = envelope("secret_scan", {
    scopes: ["pods", "logs", "redis", "object_storage", "traces", "crash_artifacts"],
    forbiddenPatternCount: 24,
    findings: 0,
    rawMediaObjects: 0,
    transcriptObjects: 0,
  });
  assert.equal(validateSecretScanObservation(observation), observation);
  assert.throws(
    () =>
      validateSecretScanObservation(
        envelope("secret_scan", { ...observation.results, findings: 1 }),
      ),
    /found forbidden material/u,
  );
});

test("rollback refuses silent transport switching or unsettled accounting", () => {
  const observation = envelope("rollback", {
    newAdmissionDisabled: true,
    existingRoomsTerminal: true,
    noSilentTransportSwitch: true,
    settlementComplete: true,
    previousRevisionRestored: true,
    unrelatedServicesUnchanged: true,
  });
  assert.equal(validateRollbackObservation(observation), observation);
  assert.throws(
    () =>
      validateRollbackObservation(
        envelope("rollback", { ...observation.results, noSilentTransportSwitch: false }),
      ),
    /noSilentTransportSwitch/u,
  );
});

test("cost acceptance preserves the fixed floor, room cap, alerts, and separate providers", () => {
  const observation = envelope("cost", {
    fixedFloorMonthlyUsd: 1_500,
    forecastMonthlyUsd: 1_825,
    observedDailyUsd: 51.25,
    budgetAlertsActive: true,
    roomCap: 20,
    googleCreditsModeledAsZeroCost: false,
    openAiIncludedInGoogleCost: false,
  });
  assert.equal(validateCostObservation(observation, bundle), observation);
  assert.throws(
    () =>
      validateCostObservation(
        envelope("cost", { ...observation.results, googleCreditsModeledAsZeroCost: true }),
        bundle,
      ),
    /modeled Google credits as zero cost/u,
  );
});

test("public receipt projection is digest-only and source receipts cannot claim live proof", () => {
  const observation = envelope("connectivity", {
    packagedOmega: true,
    signaling: true,
    certificate: true,
    publicIpAdvertisement: true,
    modes: ["direct_udp", "tcp_fallback", "turn_tls"].map((mode) => ({
      mode,
      roomJoined: true,
      microphonePublished: true,
      sarahAudioSubscribed: true,
      selectedPathObserved: true,
      sessionSettled: true,
      p95JoinMs: 500,
      p95FirstAudioMs: 900,
    })),
  });
  const receipt = buildPublicReceipt(observation, bundle);
  assert.equal(receipt.evidenceTier, "live_observed");
  assert.equal(receipt.liveProof, true);
  assert.doesNotMatch(JSON.stringify(receipt), /openagentsgemini|https?:|wss?:/u);

  const sourceReceipt = {
    ...receipt,
    receiptRef: "livekit-ops-receipt-ref://sha256/source-only-fixture",
    phase: "deployment",
    deployedRevision: "pending_dependency",
    outcome: "planned",
    evidenceTier: "source_only",
    liveProof: false,
    resultDigest: sha256("source-only"),
  };
  assert.equal(validateSourceOnlyReceipt(sourceReceipt), sourceReceipt);
  assert.throws(
    () => validateSourceOnlyReceipt({ ...sourceReceipt, liveProof: true }),
    /cannot claim live proof/u,
  );
});

test("public-safe validation rejects endpoints, IPs, secrets, transcripts, and audio fields", () => {
  assert.throws(
    () => assertPublicSafe({ endpoint: "redacted" }),
    /forbidden public receipt field/u,
  );
  assert.throws(() => assertPublicSafe({ value: "wss://livekit.example" }), /private topology/u);
  assert.throws(() => assertPublicSafe({ value: "10.1.2.3" }), /private topology/u);
  assert.throws(() => assertPublicSafe({ transcript: "hello" }), /forbidden public receipt field/u);
  assert.throws(() => assertPublicSafe({ audio: "bytes" }), /forbidden public receipt field/u);
});

test("monitoring prerequisite preserves only the channel ref and redacted status", () => {
  const receipt = {
    schemaVersion: "openagents.livekit_prerequisite_receipt.v1",
    receiptRef: "livekit-prerequisite-receipt-ref://sha256/monitoring-channel",
    issueRef: "github-issue-ref://OpenAgentsInc/openagents/9284",
    kind: "monitoring_notification_channel",
    resourceRef:
      "gcp-monitoring-channel-ref://projects/openagentsgemini/notificationChannels/1554456325732494481",
    status: "present",
    observedAt: "2026-07-30T21:45:15.000Z",
    evidenceTier: "live_observed",
    destinationRedacted: true,
    resultDigest: digest("9"),
    limitations: ["This proves channel presence only."],
  };
  assert.equal(validatePrerequisiteReceipt(receipt), receipt);
  assert.throws(
    () => validatePrerequisiteReceipt({ ...receipt, destinationRedacted: false }),
    /exposed its destination/u,
  );
});

test("API prerequisite preserves the exact four service refs and success only", () => {
  const receipt = {
    schemaVersion: "openagents.livekit_prerequisite_receipt.v1",
    receiptRef: "livekit-prerequisite-receipt-ref://sha256/api-enablement",
    issueRef: "github-issue-ref://OpenAgentsInc/openagents/9284",
    kind: "google_api_enablement",
    resourceRefs: [
      "gcp-service-ref://openagentsgemini/servicenetworking.googleapis.com",
      "gcp-service-ref://openagentsgemini/billingbudgets.googleapis.com",
      "gcp-service-ref://openagentsgemini/networkmanagement.googleapis.com",
      "gcp-service-ref://openagentsgemini/iamcredentials.googleapis.com",
    ],
    status: "succeeded",
    observedAt: "2026-07-30T21:46:34.000Z",
    evidenceTier: "live_observed",
    resultDigest: digest("8"),
    limitations: ["This proves only API enablement."],
  };
  assert.equal(validatePrerequisiteReceipt(receipt), receipt);
  assert.throws(
    () =>
      validatePrerequisiteReceipt({
        ...receipt,
        resourceRefs: receipt.resourceRefs.slice(1),
      }),
    /exact LiveKit API set/u,
  );
});
