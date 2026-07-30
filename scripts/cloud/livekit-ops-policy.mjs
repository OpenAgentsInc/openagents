import { createHash } from "node:crypto";

export const LIVEKIT_OPS = Object.freeze({
  project: "openagentsgemini",
  region: "us-central1",
  zones: Object.freeze(["us-central1-a", "us-central1-b", "us-central1-c"]),
  namespace: "livekit-system",
  release: "livekit-server",
  chartVersion: "1.11.0",
  chartSourceCommit: "8f0ad0809c2be8cbed375a6f8bef10625e5e8a2b",
  chartArchiveDigest: "sha256:e63ac0d2605b78e849ebbe850f28adafbf9aa36973d2d77a20743d47ee559272",
  sourceBaseRevision: "c10a0ba1c13edb2e599140ea29b054b893ffbccf",
  serverImage:
    "docker.io/livekit/livekit-server:v1.13.4@sha256:189f7c81b704a36642bc5c7e2d3e1ae83744627c11978a23a251bf19fbec64e0",
  serverImageDigest: "sha256:189f7c81b704a36642bc5c7e2d3e1ae83744627c11978a23a251bf19fbec64e0",
  serverImageSourceCommit: "0b3fd288e3ef3263ec475ba0d78cf3ad77459981",
  certManagerVersion: "v1.21.1",
  certManagerChartDigest: "sha256:c27101f3f3e2349fb4a9e704316105bf7b52ad73b8c8257d3498ef7f2f6a4adc",
  externalSecretsVersion: "2.8.0",
  externalSecretsChartDigest:
    "sha256:251e4615013c6d2f9ade5cedf1cd8615613f286bfc381e44fb005f197e611ecd",
  alphaConcurrency: 20,
  ownerPrivateConcurrency: 1,
  communityConcurrency: 2,
  idleTimeoutSeconds: 120,
  roomLifetimeSeconds: 1_800,
  canaryMaximumLifetimeSeconds: 21_600,
});

const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const COMMIT = /^[0-9a-f]{40}$/u;
const REF = /^[a-z][a-z0-9_.-]*-ref:\/\/[a-zA-Z0-9._~:/?#@!$&'()*+,;=%-]+$/u;
const SECRETISH_KEY =
  /(authorization|cookie|credential|private.?key|secret.?value|token|transcript|audio|prompt|email|ip.?address|endpoint|url)$/iu;
const SECRETISH_VALUE =
  /(-----BEGIN [A-Z ]*PRIVATE KEY-----|(?:sk|sess|pat|ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_-]{12,}|bearer\s+[A-Za-z0-9._~+/=-]{12,}|wss?:\/\/|https?:\/\/|\b(?:\d{1,3}\.){3}\d{1,3}\b)/iu;

const isRecord = (value) => typeof value === "object" && value !== null && !Array.isArray(value);

const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const assertExactKeys = (value, required, optional, label) => {
  assert(isRecord(value), `${label} must be an object`);
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) {
    assert(allowed.has(key), `${label} has unsupported field ${key}`);
  }
  for (const key of required) {
    assert(Object.hasOwn(value, key), `${label} is missing ${key}`);
  }
};

const assertString = (value, label) => {
  assert(typeof value === "string" && value.length > 0, `${label} must be a non-empty string`);
  return value;
};

const assertInteger = (value, minimum, maximum, label) => {
  assert(
    Number.isSafeInteger(value) && value >= minimum && value <= maximum,
    `${label} must be an integer from ${minimum} through ${maximum}`,
  );
  return value;
};

const assertFinite = (value, minimum, maximum, label) => {
  assert(
    typeof value === "number" && Number.isFinite(value) && value >= minimum && value <= maximum,
    `${label} must be a finite number from ${minimum} through ${maximum}`,
  );
  return value;
};

const assertTimestamp = (value, label) => {
  const timestamp = assertString(value, label);
  assert(Number.isFinite(Date.parse(timestamp)), `${label} must be an ISO timestamp`);
  return timestamp;
};

const assertDigest = (value, label) => {
  const digest = assertString(value, label);
  assert(DIGEST.test(digest), `${label} must be a sha256 digest`);
  return digest;
};

const assertDigestHex = (value, label) => {
  const digest = assertString(value, label);
  assert(/^[0-9a-f]{64}$/u.test(digest), `${label} must be a 64-character sha256 hex digest`);
  return digest;
};

const assertCommit = (value, label) => {
  const commit = assertString(value, label);
  assert(COMMIT.test(commit), `${label} must be a 40-character Git commit`);
  return commit;
};

const assertRef = (value, label) => {
  const ref = assertString(value, label);
  assert(REF.test(ref), `${label} must be an opaque ref`);
  assert(!SECRETISH_VALUE.test(ref), `${label} contains private topology`);
  return ref;
};

const assertBoolean = (value, label) => {
  assert(typeof value === "boolean", `${label} must be a boolean`);
  return value;
};

export const sha256 = (value) => `sha256:${createHash("sha256").update(value).digest("hex")}`;

export function assertPublicSafe(value, path = "value") {
  if (Array.isArray(value)) {
    value.forEach((item, index) => assertPublicSafe(item, `${path}[${index}]`));
    return;
  }
  if (isRecord(value)) {
    for (const [key, item] of Object.entries(value)) {
      assert(!SECRETISH_KEY.test(key), `${path}.${key} is a forbidden public receipt field`);
      assertPublicSafe(item, `${path}.${key}`);
    }
    return;
  }
  if (typeof value === "string") {
    assert(!SECRETISH_VALUE.test(value), `${path} contains secret or private topology material`);
  }
}

export function validateDeploymentBundle(value) {
  assertExactKeys(
    value,
    [
      "schemaVersion",
      "project",
      "region",
      "zones",
      "release",
      "namespace",
      "sourceBaseRevision",
      "chart",
      "serverImage",
      "configurationDigest",
      "renderedManifestDigest",
      "resources",
      "secrets",
      "googleSecretContainers",
      "manifests",
      "limits",
      "sourceState",
      "pendingDependencies",
    ],
    [],
    "bundle",
  );
  assert(
    value.schemaVersion === "openagents.livekit_deployment_bundle.v1",
    "bundle schemaVersion is unsupported",
  );
  assert(value.project === LIVEKIT_OPS.project, `bundle project must be ${LIVEKIT_OPS.project}`);
  assert(value.region === LIVEKIT_OPS.region, `bundle region must be ${LIVEKIT_OPS.region}`);
  assert(
    Array.isArray(value.zones) &&
      value.zones.length === LIVEKIT_OPS.zones.length &&
      LIVEKIT_OPS.zones.every((zone, index) => value.zones[index] === zone),
    `bundle zones must be ${LIVEKIT_OPS.zones.join(",")}`,
  );
  assert(value.release === LIVEKIT_OPS.release, `bundle release must be ${LIVEKIT_OPS.release}`);
  assert(
    value.namespace === LIVEKIT_OPS.namespace,
    `bundle namespace must be ${LIVEKIT_OPS.namespace}`,
  );
  assertCommit(value.sourceBaseRevision, "bundle.sourceBaseRevision");
  assert(
    value.sourceBaseRevision === LIVEKIT_OPS.sourceBaseRevision,
    "bundle source base revision is not admitted",
  );
  assertDigest(value.configurationDigest, "bundle.configurationDigest");
  assertDigest(value.renderedManifestDigest, "bundle.renderedManifestDigest");

  assertExactKeys(value.chart, ["version", "sourceCommit", "archiveSha256"], [], "bundle.chart");
  assert(value.chart.version === LIVEKIT_OPS.chartVersion, "bundle chart version is not pinned");
  assert(
    value.chart.sourceCommit === LIVEKIT_OPS.chartSourceCommit,
    "bundle chart source commit is not pinned",
  );
  assertDigestHex(value.chart.archiveSha256, "bundle.chart.archiveSha256");
  assert(
    `sha256:${value.chart.archiveSha256}` === LIVEKIT_OPS.chartArchiveDigest,
    "bundle chart archive digest is not pinned",
  );

  assertExactKeys(
    value.serverImage,
    ["reference", "digest", "sourceCommit"],
    [],
    "bundle.serverImage",
  );
  const imageReference = assertString(value.serverImage.reference, "bundle.serverImage.reference");
  assert(imageReference === LIVEKIT_OPS.serverImage, "bundle server image reference is not pinned");
  assertDigest(value.serverImage.digest, "bundle.serverImage.digest");
  assert(
    value.serverImage.digest === LIVEKIT_OPS.serverImageDigest,
    "bundle server image digest is not pinned",
  );
  assert(
    value.serverImage.sourceCommit === LIVEKIT_OPS.serverImageSourceCommit,
    "bundle server image source commit is not pinned",
  );

  const expectedResources = {
    cluster: "oa-livekit-prod",
    sfuNodePool: "oa-livekit-prod-sfu",
    agentNodePool: "oa-livekit-prod-app",
    redis: "oa-livekit-redis",
    signalingAddress: "oa-livekit-prod-signal",
    turnAddress: "oa-livekit-prod-turn",
    signalingService: "livekit-server",
    turnService: "livekit-server-turn",
    serverKsa: "livekit-server",
    serverGsa: `oa-livekit-server@${LIVEKIT_OPS.project}.iam.gserviceaccount.com`,
  };
  const resourceKeys = Object.keys(expectedResources);
  assertExactKeys(value.resources, resourceKeys, [], "bundle.resources");
  for (const key of resourceKeys) {
    const resource = assertString(value.resources[key], `bundle.resources.${key}`);
    assert(
      resource === expectedResources[key],
      `bundle.resources.${key} must be ${expectedResources[key]}`,
    );
  }

  assert(
    Array.isArray(value.secrets) &&
      value.secrets.length === 4 &&
      [
        "livekit-server-keys",
        "livekit-redis-auth",
        "livekit-turn-tls",
        "cloudflare-dns-token",
      ].every((name, index) => value.secrets[index] === name),
    "bundle secrets must contain the four exact ordered Kubernetes secret references",
  );
  value.secrets.forEach((name, index) => {
    assertString(name, `bundle.secrets[${index}]`);
    assert(
      [
        "livekit-server-keys",
        "livekit-redis-auth",
        "livekit-turn-tls",
        "cloudflare-dns-token",
      ].includes(name),
      `bundle.secrets[${index}] is outside the exact secret-ref set`,
    );
  });
  const expectedGoogleSecrets = [
    "oa-livekit-prod-server-keys",
    "oa-livekit-prod-redis-auth",
    "oa-livekit-prod-cloudflare-dns",
    "oa-livekit-prod-openai-api-key",
  ];
  assert(
    Array.isArray(value.googleSecretContainers) &&
      value.googleSecretContainers.length === expectedGoogleSecrets.length &&
      expectedGoogleSecrets.every((name, index) => value.googleSecretContainers[index] === name),
    "bundle googleSecretContainers must contain the four exact ordered metadata-only refs",
  );

  assert(
    Array.isArray(value.manifests) && value.manifests.length > 0,
    "bundle manifests are required",
  );
  for (const [index, manifest] of value.manifests.entries()) {
    assertExactKeys(manifest, ["path", "sha256"], [], `bundle.manifests[${index}]`);
    const manifestPath = assertString(manifest.path, `bundle.manifests[${index}].path`);
    assert(
      manifestPath.startsWith("infra/livekit/") &&
        !manifestPath.includes("..") &&
        !manifestPath.startsWith("/"),
      `bundle.manifests[${index}].path must stay under infra/livekit`,
    );
    assertDigest(manifest.sha256, `bundle.manifests[${index}].sha256`);
  }

  assertExactKeys(
    value.limits,
    [
      "maxConcurrentSarahRooms",
      "maxOwnerPrivateRooms",
      "maxCommunityRoomsPerCommunity",
      "idleTimeoutSeconds",
      "maxRoomLifetimeSeconds",
    ],
    [],
    "bundle.limits",
  );
  const expectedLimits = {
    maxConcurrentSarahRooms: LIVEKIT_OPS.alphaConcurrency,
    maxOwnerPrivateRooms: LIVEKIT_OPS.ownerPrivateConcurrency,
    maxCommunityRoomsPerCommunity: LIVEKIT_OPS.communityConcurrency,
    idleTimeoutSeconds: LIVEKIT_OPS.idleTimeoutSeconds,
    maxRoomLifetimeSeconds: LIVEKIT_OPS.roomLifetimeSeconds,
  };
  for (const [key, expected] of Object.entries(expectedLimits)) {
    assert(value.limits[key] === expected, `bundle.limits.${key} must be ${expected}`);
  }
  assert(value.sourceState === "source_only", "deployment bundle is source evidence only");
  assert(
    Array.isArray(value.pendingDependencies) &&
      value.pendingDependencies.length === 2 &&
      value.pendingDependencies[0] === "packaged_omega_acceptance" &&
      value.pendingDependencies[1] === "sarah_worker_acceptance",
    "source bundle must keep packaged Omega and Sarah worker acceptance pending",
  );
  return value;
}

export function validateAddonLock(value) {
  assertExactKeys(
    value,
    ["schemaVersion", "certManager", "externalSecrets", "managedPrometheus"],
    [],
    "addon lock",
  );
  assert(
    value.schemaVersion === "openagents.livekit_addon_pins.v1",
    "addon lock schemaVersion is unsupported",
  );
  assertExactKeys(
    value.certManager,
    ["repository", "chart", "version", "chartSha256", "resourceApiVersion"],
    [],
    "addon lock certManager",
  );
  assert(
    value.certManager.repository === "https://charts.jetstack.io" &&
      value.certManager.chart === "cert-manager" &&
      value.certManager.version === LIVEKIT_OPS.certManagerVersion &&
      value.certManager.resourceApiVersion === "cert-manager.io/v1",
    "cert-manager addon coordinates are not admitted",
  );
  assertDigestHex(value.certManager.chartSha256, "addon lock certManager.chartSha256");
  assert(
    `sha256:${value.certManager.chartSha256}` === LIVEKIT_OPS.certManagerChartDigest,
    "cert-manager addon digest is not admitted",
  );

  assertExactKeys(
    value.externalSecrets,
    ["repository", "chart", "version", "chartSha256", "resourceApiVersion"],
    [],
    "addon lock externalSecrets",
  );
  assert(
    value.externalSecrets.repository === "https://charts.external-secrets.io" &&
      value.externalSecrets.chart === "external-secrets" &&
      value.externalSecrets.version === LIVEKIT_OPS.externalSecretsVersion &&
      value.externalSecrets.resourceApiVersion === "external-secrets.io/v1",
    "External Secrets addon coordinates are not admitted",
  );
  assertDigestHex(value.externalSecrets.chartSha256, "addon lock externalSecrets.chartSha256");
  assert(
    `sha256:${value.externalSecrets.chartSha256}` === LIVEKIT_OPS.externalSecretsChartDigest,
    "External Secrets addon digest is not admitted",
  );

  assertExactKeys(
    value.managedPrometheus,
    ["delivery", "resourceApiVersion", "binaryVersionAuthority"],
    [],
    "addon lock managedPrometheus",
  );
  assert(
    value.managedPrometheus.delivery === "gke-managed-collection" &&
      value.managedPrometheus.resourceApiVersion === "monitoring.googleapis.com/v1" &&
      value.managedPrometheus.binaryVersionAuthority === "pinned-gke-cluster-version",
    "managed Prometheus addon authority is not admitted",
  );
  return value;
}

const validateObservationEnvelope = (value, phase) => {
  assertExactKeys(
    value,
    [
      "schemaVersion",
      "phase",
      "stage",
      "sourceBaseRevision",
      "deployedRevision",
      "startedAt",
      "settledAt",
      "resourceRefs",
      "results",
    ],
    ["notes"],
    "observation",
  );
  assert(
    value.schemaVersion === "openagents.livekit_acceptance_observation.v1",
    "observation schemaVersion is unsupported",
  );
  assert(value.phase === phase, `observation phase must be ${phase}`);
  assert(["canary", "production"].includes(value.stage), "observation stage is unsupported");
  assertCommit(value.sourceBaseRevision, "observation.sourceBaseRevision");
  assertCommit(value.deployedRevision, "observation.deployedRevision");
  const startedAt = assertTimestamp(value.startedAt, "observation.startedAt");
  const settledAt = assertTimestamp(value.settledAt, "observation.settledAt");
  assert(Date.parse(settledAt) >= Date.parse(startedAt), "observation timestamps regress");
  assert(
    Array.isArray(value.resourceRefs) &&
      value.resourceRefs.length > 0 &&
      value.resourceRefs.length <= 32,
    "observation resourceRefs must contain 1 through 32 refs",
  );
  value.resourceRefs.forEach((ref, index) => assertRef(ref, `observation.resourceRefs[${index}]`));
  if (value.notes !== undefined) {
    assert(
      Array.isArray(value.notes) && value.notes.length <= 16,
      "observation notes must be a bounded array",
    );
    value.notes.forEach((note, index) => {
      assertString(note, `observation.notes[${index}]`);
      assert(note.length <= 240, `observation.notes[${index}] is too long`);
    });
  }
  assertPublicSafe(value);
  return value;
};

export function validateConnectivityObservation(value) {
  validateObservationEnvelope(value, "connectivity");
  assertExactKeys(
    value.results,
    ["packagedOmega", "signaling", "certificate", "publicIpAdvertisement", "modes"],
    [],
    "observation.results",
  );
  for (const key of ["packagedOmega", "signaling", "certificate", "publicIpAdvertisement"]) {
    assertBoolean(value.results[key], `observation.results.${key}`);
    assert(value.results[key], `connectivity ${key} did not pass`);
  }
  assert(Array.isArray(value.results.modes), "connectivity modes must be an array");
  const expectedModes = ["direct_udp", "tcp_fallback", "turn_tls"];
  assert(
    value.results.modes.length === expectedModes.length,
    "connectivity requires exactly three modes",
  );
  for (const expected of expectedModes) {
    const mode = value.results.modes.find((candidate) => candidate.mode === expected);
    assert(mode !== undefined, `connectivity is missing ${expected}`);
    assertExactKeys(
      mode,
      [
        "mode",
        "roomJoined",
        "microphonePublished",
        "sarahAudioSubscribed",
        "selectedPathObserved",
        "sessionSettled",
        "p95JoinMs",
        "p95FirstAudioMs",
      ],
      [],
      `connectivity.${expected}`,
    );
    for (const key of [
      "roomJoined",
      "microphonePublished",
      "sarahAudioSubscribed",
      "selectedPathObserved",
      "sessionSettled",
    ]) {
      assert(mode[key] === true, `connectivity ${expected}.${key} did not pass`);
    }
    assertFinite(mode.p95JoinMs, 0, 120_000, `connectivity ${expected}.p95JoinMs`);
    assertFinite(mode.p95FirstAudioMs, 0, 120_000, `connectivity ${expected}.p95FirstAudioMs`);
  }
  return value;
}

export function validateLoadObservation(value, bundle) {
  validateObservationEnvelope(value, "load");
  assertExactKeys(
    value.results,
    [
      "concurrentRooms",
      "spareCapacityPercent",
      "capEnforced",
      "settledSessions",
      "unsettledSessions",
      "maximumSfuCpuPercent",
      "maximumWorkerCpuPercent",
      "maximumPacketLossPercent",
      "p95FirstAudioMs",
    ],
    [],
    "observation.results",
  );
  assertInteger(
    value.results.concurrentRooms,
    bundle.limits.maxConcurrentSarahRooms,
    10_000,
    "load concurrentRooms",
  );
  assertFinite(value.results.spareCapacityPercent, 20, 100, "load spareCapacityPercent");
  assert(value.results.capEnforced === true, "load did not prove the hard concurrency cap");
  assertInteger(
    value.results.settledSessions,
    value.results.concurrentRooms,
    100_000,
    "load settledSessions",
  );
  assert(value.results.unsettledSessions === 0, "load left unsettled sessions");
  assertFinite(value.results.maximumSfuCpuPercent, 0, 80, "load maximumSfuCpuPercent");
  assertFinite(value.results.maximumWorkerCpuPercent, 0, 80, "load maximumWorkerCpuPercent");
  assertFinite(value.results.maximumPacketLossPercent, 0, 5, "load maximumPacketLossPercent");
  assertFinite(value.results.p95FirstAudioMs, 0, 5_000, "load p95FirstAudioMs");
  return value;
}

export const REQUIRED_DRILLS = Object.freeze([
  "sfu_pod_drain",
  "sfu_node_loss",
  "zone_loss",
  "redis_failover",
  "signaling_backend_removal",
  "certificate_renewal",
  "turn_backend_loss",
  "worker_crash",
  "provider_disconnect",
  "quota_exhaustion",
  "server_rollback",
]);

export function validateDrillObservation(value) {
  validateObservationEnvelope(value, "drills");
  assertExactKeys(value.results, ["drills"], [], "observation.results");
  assert(Array.isArray(value.results.drills), "drills must be an array");
  assert(
    value.results.drills.length === REQUIRED_DRILLS.length,
    "every required failure drill must appear once",
  );
  for (const scenario of REQUIRED_DRILLS) {
    const drill = value.results.drills.find((candidate) => candidate.scenario === scenario);
    assert(drill !== undefined, `failure drill is missing ${scenario}`);
    assertExactKeys(
      drill,
      [
        "scenario",
        "outcome",
        "visibleFailure",
        "noProviderOverlap",
        "accountingTerminal",
        "freshAdmissionRequired",
        "uninterruptedSpeechClaimed",
      ],
      [],
      `drills.${scenario}`,
    );
    assert(
      ["continued", "bounded_failure"].includes(drill.outcome),
      `drills.${scenario}.outcome is unsupported`,
    );
    assert(
      drill.visibleFailure === true || drill.outcome === "continued",
      `drills.${scenario} hid failure`,
    );
    assert(drill.noProviderOverlap === true, `drills.${scenario} overlapped providers`);
    assert(drill.accountingTerminal === true, `drills.${scenario} did not settle accounting`);
    assert(
      drill.freshAdmissionRequired === true,
      `drills.${scenario} did not require fresh admission`,
    );
    assert(
      drill.uninterruptedSpeechClaimed === false,
      `drills.${scenario} made an unsupported uninterrupted-speech claim`,
    );
  }
  return value;
}

export function validateSecretScanObservation(value) {
  validateObservationEnvelope(value, "secret_scan");
  assertExactKeys(
    value.results,
    ["scopes", "forbiddenPatternCount", "findings", "rawMediaObjects", "transcriptObjects"],
    [],
    "observation.results",
  );
  const expectedScopes = ["pods", "logs", "redis", "object_storage", "traces", "crash_artifacts"];
  assert(
    Array.isArray(value.results.scopes) &&
      value.results.scopes.length === expectedScopes.length &&
      expectedScopes.every((scope) => value.results.scopes.includes(scope)),
    "secret scan does not cover every required scope",
  );
  assertInteger(
    value.results.forbiddenPatternCount,
    1,
    100_000,
    "secret scan forbiddenPatternCount",
  );
  assert(value.results.findings === 0, "secret scan found forbidden material");
  assert(value.results.rawMediaObjects === 0, "secret scan found raw media");
  assert(value.results.transcriptObjects === 0, "secret scan found transcripts");
  return value;
}

export function validateRollbackObservation(value) {
  validateObservationEnvelope(value, "rollback");
  assertExactKeys(
    value.results,
    [
      "newAdmissionDisabled",
      "existingRoomsTerminal",
      "noSilentTransportSwitch",
      "settlementComplete",
      "previousRevisionRestored",
      "unrelatedServicesUnchanged",
    ],
    [],
    "observation.results",
  );
  for (const [key, passed] of Object.entries(value.results)) {
    assert(passed === true, `rollback ${key} did not pass`);
  }
  return value;
}

export function validateCostObservation(value, bundle) {
  validateObservationEnvelope(value, "cost");
  assertExactKeys(
    value.results,
    [
      "fixedFloorMonthlyUsd",
      "forecastMonthlyUsd",
      "observedDailyUsd",
      "budgetAlertsActive",
      "roomCap",
      "googleCreditsModeledAsZeroCost",
      "openAiIncludedInGoogleCost",
    ],
    [],
    "observation.results",
  );
  assertFinite(value.results.fixedFloorMonthlyUsd, 1_500, 1_000_000, "cost fixedFloorMonthlyUsd");
  assertFinite(
    value.results.forecastMonthlyUsd,
    value.results.fixedFloorMonthlyUsd,
    1_000_000,
    "cost forecastMonthlyUsd",
  );
  assertFinite(value.results.observedDailyUsd, 0, 1_000_000, "cost observedDailyUsd");
  assert(value.results.budgetAlertsActive === true, "cost budget alerts are not active");
  assert(
    value.results.roomCap === bundle.limits.maxConcurrentSarahRooms,
    "cost observation room cap does not match the deployment bundle",
  );
  assert(
    value.results.googleCreditsModeledAsZeroCost === false,
    "cost observation modeled Google credits as zero cost",
  );
  assert(
    value.results.openAiIncludedInGoogleCost === false,
    "cost observation collapsed OpenAI into Google cost",
  );
  return value;
}

export const OBSERVATION_VALIDATORS = Object.freeze({
  connectivity: validateConnectivityObservation,
  load: validateLoadObservation,
  drills: validateDrillObservation,
  secret_scan: validateSecretScanObservation,
  cost: validateCostObservation,
  rollback: validateRollbackObservation,
});

export function buildPublicReceipt(observation, bundle) {
  const phase = observation.phase;
  const validator = OBSERVATION_VALIDATORS[phase];
  assert(validator !== undefined, `unsupported acceptance phase ${phase}`);
  if (phase === "load" || phase === "cost") validator(observation, bundle);
  else validator(observation);
  const resultDigest = sha256(JSON.stringify(observation.results));
  const receipt = {
    schemaVersion: "openagents.livekit_ops_receipt.v1",
    receiptRef: `livekit-ops-receipt-ref://sha256/${resultDigest.slice("sha256:".length)}`,
    issueRef: "github-issue-ref://OpenAgentsInc/openagents/9284",
    stage: observation.stage,
    phase,
    sourceBaseRevision: observation.sourceBaseRevision,
    deployedRevision: observation.deployedRevision,
    bundleDigest: sha256(JSON.stringify(bundle)),
    configurationDigest: bundle.configurationDigest,
    resourceRefs: observation.resourceRefs,
    startedAt: observation.startedAt,
    settledAt: observation.settledAt,
    outcome: "passed",
    evidenceTier: "live_observed",
    liveProof: true,
    resultDigest,
    limitations: observation.notes ?? [],
  };
  assertPublicSafe(receipt);
  return receipt;
}

export function validateSourceOnlyReceipt(value) {
  assertExactKeys(
    value,
    [
      "schemaVersion",
      "receiptRef",
      "issueRef",
      "stage",
      "phase",
      "sourceBaseRevision",
      "deployedRevision",
      "bundleDigest",
      "configurationDigest",
      "resourceRefs",
      "startedAt",
      "settledAt",
      "outcome",
      "evidenceTier",
      "liveProof",
      "resultDigest",
      "limitations",
    ],
    [],
    "receipt",
  );
  assert(
    value.schemaVersion === "openagents.livekit_ops_receipt.v1",
    "receipt schema is unsupported",
  );
  assertRef(value.receiptRef, "receipt.receiptRef");
  assert(
    value.issueRef === "github-issue-ref://OpenAgentsInc/openagents/9284",
    "receipt issueRef is wrong",
  );
  assert(["canary", "production"].includes(value.stage), "receipt stage is unsupported");
  assert(
    [
      "deployment",
      "connectivity",
      "load",
      "drills",
      "secret_scan",
      "cost",
      "rollback",
      "destroy",
    ].includes(value.phase),
    "receipt phase is unsupported",
  );
  assertCommit(value.sourceBaseRevision, "receipt.sourceBaseRevision");
  assertDigest(value.bundleDigest, "receipt.bundleDigest");
  assertDigest(value.configurationDigest, "receipt.configurationDigest");
  assert(
    Array.isArray(value.resourceRefs) && value.resourceRefs.length > 0,
    "receipt resourceRefs are required",
  );
  value.resourceRefs.forEach((ref, index) => assertRef(ref, `receipt.resourceRefs[${index}]`));
  assertTimestamp(value.startedAt, "receipt.startedAt");
  assertTimestamp(value.settledAt, "receipt.settledAt");
  assert(
    ["planned", "passed", "failed", "rolled_back", "destroyed"].includes(value.outcome),
    "receipt outcome is unsupported",
  );
  assert(
    ["source_only", "live_observed"].includes(value.evidenceTier),
    "receipt evidenceTier is unsupported",
  );
  assertBoolean(value.liveProof, "receipt.liveProof");
  if (value.evidenceTier === "source_only") {
    assert(
      value.deployedRevision === "pending_dependency",
      "a source-only receipt cannot name a deployed revision",
    );
    assert(value.liveProof === false, "a source-only receipt cannot claim live proof");
    assert(value.outcome === "planned", "a source-only receipt can only be planned");
  } else {
    assertCommit(value.deployedRevision, "receipt.deployedRevision");
    assert(value.liveProof === true, "live-observed evidence must identify live proof");
    assert(value.outcome !== "planned", "live-observed evidence cannot be planned");
  }
  assertDigest(value.resultDigest, "receipt.resultDigest");
  assert(Array.isArray(value.limitations), "receipt limitations must be an array");
  assertPublicSafe(value);
  return value;
}

export function validatePrerequisiteReceipt(value) {
  assert(isRecord(value), "prerequisite receipt must be an object");
  if (value.kind === "google_api_enablement") {
    assertExactKeys(
      value,
      [
        "schemaVersion",
        "receiptRef",
        "issueRef",
        "kind",
        "resourceRefs",
        "status",
        "observedAt",
        "evidenceTier",
        "resultDigest",
        "limitations",
      ],
      [],
      "prerequisite receipt",
    );
    assert(
      value.schemaVersion === "openagents.livekit_prerequisite_receipt.v1",
      "prerequisite receipt schema is unsupported",
    );
    assertRef(value.receiptRef, "prerequisite receipt.receiptRef");
    assert(
      value.issueRef === "github-issue-ref://OpenAgentsInc/openagents/9284",
      "prerequisite receipt issueRef is wrong",
    );
    const expectedServices = [
      "gcp-service-ref://openagentsgemini/servicenetworking.googleapis.com",
      "gcp-service-ref://openagentsgemini/billingbudgets.googleapis.com",
      "gcp-service-ref://openagentsgemini/networkmanagement.googleapis.com",
      "gcp-service-ref://openagentsgemini/iamcredentials.googleapis.com",
    ];
    assert(
      Array.isArray(value.resourceRefs) &&
        value.resourceRefs.length === expectedServices.length &&
        expectedServices.every((ref, index) => value.resourceRefs[index] === ref),
      "prerequisite service refs are outside the exact LiveKit API set",
    );
    value.resourceRefs.forEach((ref, index) =>
      assertRef(ref, `prerequisite receipt.resourceRefs[${index}]`),
    );
    assert(value.status === "succeeded", "prerequisite receipt status is unsupported");
    assertTimestamp(value.observedAt, "prerequisite receipt.observedAt");
    assert(value.evidenceTier === "live_observed", "prerequisite receipt is not live-observed");
    assertDigest(value.resultDigest, "prerequisite receipt.resultDigest");
    assert(
      Array.isArray(value.limitations) && value.limitations.length > 0,
      "prerequisite receipt limitations are required",
    );
    assertPublicSafe(value);
    return value;
  }
  assertExactKeys(
    value,
    [
      "schemaVersion",
      "receiptRef",
      "issueRef",
      "kind",
      "resourceRef",
      "status",
      "observedAt",
      "evidenceTier",
      "destinationRedacted",
      "resultDigest",
      "limitations",
    ],
    [],
    "prerequisite receipt",
  );
  assert(
    value.schemaVersion === "openagents.livekit_prerequisite_receipt.v1",
    "prerequisite receipt schema is unsupported",
  );
  assertRef(value.receiptRef, "prerequisite receipt.receiptRef");
  assert(
    value.issueRef === "github-issue-ref://OpenAgentsInc/openagents/9284",
    "prerequisite receipt issueRef is wrong",
  );
  assert(
    value.kind === "monitoring_notification_channel",
    "prerequisite receipt kind is unsupported",
  );
  assert(
    value.resourceRef ===
      "gcp-monitoring-channel-ref://projects/openagentsgemini/notificationChannels/1554456325732494481",
    "prerequisite receipt resourceRef is outside the exact LiveKit channel",
  );
  assert(value.status === "present", "prerequisite receipt status is unsupported");
  assertTimestamp(value.observedAt, "prerequisite receipt.observedAt");
  assert(value.evidenceTier === "live_observed", "prerequisite receipt is not live-observed");
  assert(value.destinationRedacted === true, "prerequisite receipt exposed its destination");
  assertDigest(value.resultDigest, "prerequisite receipt.resultDigest");
  assert(
    Array.isArray(value.limitations) && value.limitations.length > 0,
    "prerequisite receipt limitations are required",
  );
  assertPublicSafe(value);
  return value;
}
