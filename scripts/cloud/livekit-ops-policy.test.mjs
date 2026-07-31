import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import {
  LIVEKIT_OPS,
  LIVEKIT_PREREQUISITE_SERVICE_REFS,
  REQUIRED_DRILLS,
  assertPublicSafe,
  buildPublicReceipt,
  publicSafeCommandFailure,
  sha256,
  validateCostObservation,
  validateAddonLock,
  validateBillingAccountId,
  validateConnectivityObservation,
  validateDeploymentBundle,
  validateDrillObservation,
  validateHistoricalDeploymentBundle,
  validateLoadObservation,
  validatePrerequisiteReceipt,
  validateProductionRedisProjection,
  validateRollbackObservation,
  validateRuntimeManifestInventory,
  validateSecretScanObservation,
  validateServerKeyProjection,
  validateSourceOnlyReceipt,
} from "./livekit-ops-policy.mjs";

const digest = (character) => `sha256:${character.repeat(64)}`;
const deployedRevision = "1".repeat(40);

test("billing account IDs admit Google's uppercase alphanumeric shape", () => {
  const linkedBillingAccountId = "01D15C-64524A-1062EA";
  assert.equal(validateBillingAccountId(linkedBillingAccountId), linkedBillingAccountId);

  for (const invalidBillingAccountId of [
    "",
    " ",
    "01d15c-64524A-1062EA",
    "01D15_-64524A-1062EA",
    "01D15-64524A-1062EA",
    "01D15C-64524A-1062E",
  ]) {
    assert.throws(
      () => validateBillingAccountId(invalidBillingAccountId),
      /billing account variable/u,
    );
  }
});

test("command failure projection never leaves a provider failure blank after redaction", () => {
  const providerFailure =
    "Error creating Address: Invalid resource.purpose SHARED_LOADBALANCER_VIP for external address";
  assert.equal(
    publicSafeCommandFailure(providerFailure, ""),
    "provider diagnostics were redacted by policy",
  );
  assert.equal(publicSafeCommandFailure("Error: quota exhausted", ""), "Error: quota exhausted");
  assert.equal(publicSafeCommandFailure("", ""), "");
});

test("canary boot invokes Caddy, opens exact host ports, and grants only log-write IAM", () => {
  const startup = readFileSync(
    resolve(import.meta.dirname, "../../infra/modules/livekit-gce-canary/startup.sh.tftpl"),
    "utf8",
  );
  const module = readFileSync(
    resolve(import.meta.dirname, "../../infra/modules/livekit-gce-canary/main.tf"),
    "utf8",
  );

  assert.ok(
    startup.includes(
      '"${reverse_proxy_image}" \\\n  caddy run --config /run/livekit/Caddyfile --adapter caddyfile',
    ),
  );
  assert.ok(
    !startup.includes('"${reverse_proxy_image}" \\\n  run --config /run/livekit/Caddyfile'),
  );

  for (const projection of [
    "tcp_fallback_port         = var.tcp_fallback_port",
    "turn_tls_port             = var.turn_tls_port",
    "media_udp_port_start      = var.media_udp_port_range.start",
    "media_udp_port_end        = var.media_udp_port_range.end",
    "enable_turn_udp           = var.enable_turn_udp",
    "turn_udp_port             = var.turn_udp_port",
  ]) {
    assert.ok(module.includes(projection));
  }
  assert.match(startup, /iptables --wait --check INPUT/u);
  assert.match(startup, /iptables --wait --insert INPUT 1/u);
  const hostFirewallRules = [
    ...startup.matchAll(/^\s*allow_host_port (tcp|udp) "([^"]+)"$/gmu),
  ].map((match) => `${match[1]}:${match[2]}`);
  assert.deepEqual(hostFirewallRules, [
    "tcp:443",
    "tcp:${tcp_fallback_port}",
    "tcp:${turn_tls_port}",
    "udp:${media_udp_port_start}:${media_udp_port_end}",
    "udp:${turn_udp_port}",
  ]);
  assert.ok(startup.includes("if ${enable_turn_udp}; then"));
  assert.ok(!startup.includes('allow_host_port tcp "7880"'));

  const bindingStart = module.indexOf('resource "google_project_iam_member" "canary_log_writer"');
  assert.notEqual(bindingStart, -1);
  const bindingEnd = module.indexOf("\n}\n", bindingStart);
  assert.notEqual(bindingEnd, -1);
  const binding = module.slice(bindingStart, bindingEnd);
  assert.match(binding, /project = var\.project_id/u);
  assert.match(binding, /role    = "roles\/logging\.logWriter"/u);
  assert.match(binding, /member  = "serviceAccount:\$\{google_service_account\.canary\.email\}"/u);
  assert.doesNotMatch(binding, /roles\/(?:editor|logging\.admin|owner)/u);
  assert.match(module, /google_project_iam_member\.canary_log_writer,/u);
});

test("production TURN reserves a premium external address without an internal-only purpose", () => {
  const module = readFileSync(
    resolve(import.meta.dirname, "../../infra/modules/livekit-gke/main.tf"),
    "utf8",
  );
  const resourceStart = module.indexOf('resource "google_compute_address" "turn"');
  const resourceEnd = module.indexOf('resource "google_redis_instance" "livekit"', resourceStart);
  assert.notEqual(resourceStart, -1);
  assert.notEqual(resourceEnd, -1);
  const turnAddress = module.slice(resourceStart, resourceEnd);

  assert.match(turnAddress, /^\s*address_type\s*=\s*"EXTERNAL"$/mu);
  assert.match(turnAddress, /^\s*network_tier\s*=\s*"PREMIUM"$/mu);
  assert.doesNotMatch(turnAddress, /^\s*purpose\s*=/mu);
});

test("server key projection binds token minting and runtime authentication", () => {
  const apiKey = "API23456789ABCD";
  const apiSecret = "B".repeat(43);
  const projection = {
    api_key: apiKey,
    api_secret: apiSecret,
    keys_yaml: `${apiKey}: ${apiSecret}\n`,
  };
  assert.equal(validateServerKeyProjection(projection), projection);
  assert.throws(
    () => validateServerKeyProjection({ ...projection, keys_yaml: `${apiKey}: ${"C".repeat(43)}` }),
    /exact admitted mapping/u,
  );
  assert.throws(
    () => validateServerKeyProjection({ ...projection, api_key: "A".repeat(24) }),
    /api_key/u,
  );
  assert.throws(
    () => validateServerKeyProjection({ ...projection, api_secret: "B".repeat(42) }),
    /api_secret/u,
  );
  assert.throws(
    () =>
      validateServerKeyProjection({
        ...projection,
        keys_yaml: `${apiKey}: ${apiSecret}\nextra: key\n`,
      }),
    /exact admitted mapping/u,
  );
  assert.throws(
    () => validateServerKeyProjection({ ...projection, extra: "not-admitted" }),
    /unsupported field/u,
  );
});

const redisTerraformSource = `
resource "google_redis_instance" "livekit" {
  tier                    = "STANDARD_HA"
  transit_encryption_mode = "SERVER_AUTHENTICATION"
  auth_enabled            = false
}
`;
const redisMetadata = {
  name: "projects/openagentsgemini/locations/us-central1/instances/oa-livekit-redis",
  tier: "STANDARD_HA",
  state: "READY",
  transitEncryptionMode: "SERVER_AUTHENTICATION",
  host: "private-redis-host",
  port: 6378,
  serverCaCerts: [{ cert: "redis-ca" }],
};
const redisSecret = {
  host: "private-redis-host:6378",
  ca_cert: "redis-ca\n",
};

test("production Redis projection derives omitted defaults from exact live and source evidence", () => {
  assert.equal(
    validateProductionRedisProjection(redisMetadata, redisSecret, redisTerraformSource),
    redisMetadata,
  );
  assert.doesNotThrow(() =>
    validateProductionRedisProjection(
      { ...redisMetadata, region: "us-central1", authEnabled: false },
      redisSecret,
      redisTerraformSource,
    ),
  );
});

test("production Redis projection rejects ambiguous identity and AUTH evidence", () => {
  for (const invalidMetadata of [
    { ...redisMetadata, name: "oa-livekit-redis" },
    {
      ...redisMetadata,
      name: "projects/openagentsgemini/locations/us-east1/instances/oa-livekit-redis",
    },
    { ...redisMetadata, region: "us-east1" },
    { ...redisMetadata, authEnabled: true },
  ]) {
    assert.throws(
      () => validateProductionRedisProjection(invalidMetadata, redisSecret, redisTerraformSource),
      /production Redis/u,
    );
  }

  for (const invalidSource of [
    redisTerraformSource.replace(
      "auth_enabled            = false",
      "auth_enabled            = true",
    ),
    redisTerraformSource.replace(
      "auth_enabled            = false",
      "# auth_enabled            = false",
    ),
    redisTerraformSource.replace(
      "auth_enabled            = false",
      "/* auth_enabled            = false */",
    ),
    `${redisTerraformSource}\nresource "google_redis_instance" "livekit" {\n  auth_enabled = false\n}\n`,
  ]) {
    assert.throws(
      () => validateProductionRedisProjection(redisMetadata, redisSecret, invalidSource),
      /Terraform/u,
    );
  }
});

test("production Redis projection rejects live host and CA drift", () => {
  assert.throws(
    () =>
      validateProductionRedisProjection(
        redisMetadata,
        { ...redisSecret, host: "different-private-host:6378" },
        redisTerraformSource,
      ),
    /host\/CA/u,
  );
  assert.throws(
    () =>
      validateProductionRedisProjection(
        { ...redisMetadata, port: 0 },
        redisSecret,
        redisTerraformSource,
      ),
    /port/u,
  );
  assert.throws(
    () =>
      validateProductionRedisProjection(
        redisMetadata,
        { ...redisSecret, ca_cert: "different-ca" },
        redisTerraformSource,
      ),
    /host\/CA/u,
  );
});

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
  workerImage: {
    reference: `us-central1-docker.pkg.dev/openagentsgemini/livekit/sarah-livekit-agent@sha256:${"0".repeat(64)}`,
    digest: `sha256:${"0".repeat(64)}`,
    pinState: "build_required",
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
    agentKsa: "sarah-agent",
    agentGsa: "oa-livekit-agent@openagentsgemini.iam.gserviceaccount.com",
    sarahSecretReaderKsa: "oa-livekit-sarah-secret-reader",
    sarahSecretReaderGsa: "oa-livekit-sarah-secret-reader@openagentsgemini.iam.gserviceaccount.com",
  },
  secrets: [
    "livekit-server-keys",
    "livekit-redis-auth",
    "livekit-turn-tls",
    "cloudflare-dns-token",
    "sarah-livekit-agent",
  ],
  googleSecretContainers: [
    "oa-livekit-prod-server-keys",
    "oa-livekit-prod-redis-auth",
    "oa-livekit-prod-cloudflare-dns",
    "oa-livekit-prod-openai-api-key",
    "oa-livekit-prod-sarah-control-root",
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
    images: {
      controller:
        "quay.io/jetstack/cert-manager-controller:v1.21.1@sha256:416a2d76870d996460e62bd7f521bf14fa017be9e3e904aab92163a331fcb61a",
      cainjector:
        "quay.io/jetstack/cert-manager-cainjector:v1.21.1@sha256:ccf6b919ec0500745a47a910118f834f9636d0aac1ff221245cd2557ed8c7c98",
      webhook:
        "quay.io/jetstack/cert-manager-webhook:v1.21.1@sha256:d8b3961b51c8c7320633f8208dc46bf88aa13804d0f7cbe48a096b2c523cee42",
      acmeSolver:
        "quay.io/jetstack/cert-manager-acmesolver:v1.21.1@sha256:dbc7cc1354f603918e7c5af7f55a0a620537394452c93a565bde75c6f48e8837",
      startupApiCheck:
        "quay.io/jetstack/cert-manager-startupapicheck:v1.21.1@sha256:d8ab6416e6e7303a86fa0a8daa82c94a8001f21c9d78eb2e7db20534e5d07ae8",
    },
  },
  externalSecrets: {
    repository: "https://charts.external-secrets.io",
    chart: "external-secrets",
    version: LIVEKIT_OPS.externalSecretsVersion,
    chartSha256: LIVEKIT_OPS.externalSecretsChartDigest.slice("sha256:".length),
    resourceApiVersion: "external-secrets.io/v1",
    image:
      "ghcr.io/external-secrets/external-secrets:v2.8.0@sha256:24c0dd3699e0988520afd2218612758cd97d1f702757b5b4fcf89adaa33ef679",
  },
  managedPrometheus: {
    delivery: "gke-managed-collection",
    resourceApiVersion: "monitoring.googleapis.com/v1",
    binaryVersionAuthority: "gke-stable-release-channel",
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
  assert.throws(
    () =>
      validateDeploymentBundle({
        ...bundle,
        workerImage: {
          ...bundle.workerImage,
          reference:
            "docker.io/openagents/sarah-livekit-agent@sha256:0000000000000000000000000000000000000000000000000000000000000000",
        },
      }),
    /Artifact Registry/u,
  );
  assert.throws(
    () =>
      validateDeploymentBundle({
        ...bundle,
        workerImage: {
          ...bundle.workerImage,
          pinState: "pinned",
        },
      }),
    /placeholder digest/u,
  );
});

test("historical bundle validation preserves topology while permitting older immutable pins", () => {
  const historicalDigest = digest("a");
  const historical = {
    ...bundle,
    sourceBaseRevision: "b".repeat(40),
    chart: {
      version: "1.10.0",
      sourceCommit: "c".repeat(40),
      archiveSha256: "d".repeat(64),
    },
    serverImage: {
      reference: `docker.io/livekit/livekit-server:v1.12.0@${historicalDigest}`,
      digest: historicalDigest,
      sourceCommit: "e".repeat(40),
    },
  };
  assert.equal(validateHistoricalDeploymentBundle(historical), historical);
  assert.throws(() => validateDeploymentBundle(historical), /source base revision/u);
  assert.throws(
    () =>
      validateHistoricalDeploymentBundle({
        ...historical,
        resources: { ...historical.resources, cluster: "another-cluster" },
      }),
    /bundle.resources.cluster/u,
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
        certManager: {
          ...addonLock.certManager,
          images: { ...addonLock.certManager.images, controller: "tag-only:latest" },
        },
      }),
    /controller image is not digest-pinned/u,
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

test("runtime inventory requires exact scopes and explicit admitted namespaces", () => {
  const inventory = [
    {
      apiVersion: "v1",
      kind: "ServiceAccount",
      namespace: "cert-manager",
      name: "cloudflare-dns01-solver",
    },
    {
      apiVersion: "cert-manager.io/v1",
      kind: "ClusterIssuer",
      namespace: null,
      name: "letsencrypt-production",
    },
    {
      apiVersion: "v1",
      kind: "Namespace",
      namespace: null,
      name: "livekit-system",
    },
    {
      apiVersion: "apps/v1",
      kind: "Deployment",
      namespace: "livekit-system",
      name: "livekit-server",
    },
    {
      apiVersion: "networking.k8s.io/v1",
      kind: "NetworkPolicy",
      namespace: "livekit-system",
      name: "sarah-livekit-agent-deny-ingress",
    },
  ];
  assert.deepEqual(validateRuntimeManifestInventory(inventory, "runtime manifest"), [
    inventory[3],
    inventory[1],
    inventory[4],
    inventory[2],
    inventory[0],
  ]);

  for (const invalidResource of [
    { ...inventory[3], namespace: null },
    { ...inventory[3], namespace: "default" },
    { ...inventory[1], namespace: "livekit-system" },
    { ...inventory[2], name: "unexpected-system" },
    { ...inventory[3], apiVersion: "batch/v1", kind: "Deployment" },
    { ...inventory[3], apiVersion: "apps/v1", kind: "Namespace", namespace: null },
  ]) {
    assert.throws(
      () => validateRuntimeManifestInventory([invalidResource], "runtime manifest"),
      /admitted explicit namespace|cluster-scoped resource|unsupported namespace|runtime kind/u,
    );
  }
  assert.throws(
    () => validateRuntimeManifestInventory([inventory[3], inventory[3]], "runtime manifest"),
    /duplicate resource/u,
  );
});

test("production runtime apply relies only on explicit manifest namespaces", () => {
  const result = spawnSync(
    process.execPath,
    [
      resolve(import.meta.dirname, "livekit-gcp-ops.mjs"),
      "--operation",
      "production-runtime-apply",
      "--bundle",
      "infra/livekit/bundle.json",
    ],
    {
      cwd: resolve(import.meta.dirname, "../.."),
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  assert.equal(result.status, 0, result.stderr);
  const plan = JSON.parse(result.stdout);
  const applyCommands = plan.commands.filter(
    (command) => command.bin === "kubectl" && command.args[0] === "apply",
  );
  assert.equal(applyCommands.length, 1);
  assert.deepEqual(applyCommands[0].args.slice(0, 4), [
    "apply",
    "--server-side",
    "--field-manager=openagents-livekit-ops",
    "-f",
  ]);
  assert.ok(!applyCommands[0].args.includes("--namespace"));
  const rolloutTargets = plan.commands
    .filter((command) => command.bin === "kubectl" && command.args.includes("rollout"))
    .map((command) => command.args.find((argument) => argument.startsWith("deployment/")));
  assert.ok(rolloutTargets.includes("deployment/livekit-server"));
  assert.ok(rolloutTargets.includes("deployment/sarah-livekit-agent"));
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

test("API prerequisite preserves the exact production service refs and success only", () => {
  assert.deepEqual(LIVEKIT_PREREQUISITE_SERVICE_REFS, [
    "gcp-service-ref://openagentsgemini/servicenetworking.googleapis.com",
    "gcp-service-ref://openagentsgemini/billingbudgets.googleapis.com",
    "gcp-service-ref://openagentsgemini/networkmanagement.googleapis.com",
    "gcp-service-ref://openagentsgemini/iamcredentials.googleapis.com",
    "gcp-service-ref://openagentsgemini/sts.googleapis.com",
  ]);
  const receipt = {
    schemaVersion: "openagents.livekit_prerequisite_receipt.v1",
    receiptRef: "livekit-prerequisite-receipt-ref://sha256/api-enablement",
    issueRef: "github-issue-ref://OpenAgentsInc/openagents/9284",
    kind: "google_api_enablement",
    resourceRefs: [...LIVEKIT_PREREQUISITE_SERVICE_REFS],
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
