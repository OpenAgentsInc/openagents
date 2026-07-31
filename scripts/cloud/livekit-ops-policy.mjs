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

export const LIVEKIT_PREREQUISITE_SERVICE_REFS = Object.freeze([
  "gcp-service-ref://openagentsgemini/servicenetworking.googleapis.com",
  "gcp-service-ref://openagentsgemini/billingbudgets.googleapis.com",
  "gcp-service-ref://openagentsgemini/networkmanagement.googleapis.com",
  "gcp-service-ref://openagentsgemini/iamcredentials.googleapis.com",
  "gcp-service-ref://openagentsgemini/sts.googleapis.com",
]);

const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const COMMIT = /^[0-9a-f]{40}$/u;
const REF = /^[a-z][a-z0-9_.-]*-ref:\/\/[a-zA-Z0-9._~:/?#@!$&'()*+,;=%-]+$/u;
const SECRETISH_KEY =
  /(authorization|cookie|credential|private.?key|secret.?value|token|transcript|audio|prompt|email|ip.?address|endpoint|url)$/iu;
const SECRETISH_VALUE =
  /(-----BEGIN [A-Z ]*PRIVATE KEY-----|(?:sk|sess|pat|ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_-]{12,}|bearer\s+[A-Za-z0-9._~+/=-]{12,}|wss?:\/\/|https?:\/\/|\b(?:\d{1,3}\.){3}\d{1,3}\b)/iu;
const COMMAND_OUTPUT_SECRETISH_LINE =
  /(address|authorization|cookie|credential|email|endpoint|password|private.?key|secret|token|transcript|audio|prompt)/iu;
const COMMAND_OUTPUT_NETWORK_VALUE =
  /(?:https?|wss?):\/\/|\b(?:\d{1,3}\.){3}\d{1,3}\b/iu;
const CERT_MANAGER_IMAGES = Object.freeze({
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
});
const EXTERNAL_SECRETS_IMAGE =
  "ghcr.io/external-secrets/external-secrets:v2.8.0@sha256:24c0dd3699e0988520afd2218612758cd97d1f702757b5b4fcf89adaa33ef679";
const RUNTIME_RESOURCE_NAMESPACES = new Map([
  ["apps/v1/Deployment", Object.freeze(["livekit-system"])],
  ["autoscaling/v2/HorizontalPodAutoscaler", Object.freeze(["livekit-system"])],
  ["cert-manager.io/v1/Certificate", Object.freeze(["livekit-system"])],
  ["cert-manager.io/v1/ClusterIssuer", null],
  ["cloud.google.com/v1/BackendConfig", Object.freeze(["livekit-system"])],
  ["external-secrets.io/v1/ExternalSecret", Object.freeze(["cert-manager", "livekit-system"])],
  ["external-secrets.io/v1/SecretStore", Object.freeze(["cert-manager", "livekit-system"])],
  ["monitoring.googleapis.com/v1/PodMonitoring", Object.freeze(["cert-manager", "livekit-system"])],
  ["networking.gke.io/v1/ManagedCertificate", Object.freeze(["livekit-system"])],
  ["networking.k8s.io/v1/Ingress", Object.freeze(["livekit-system"])],
  ["policy/v1/PodDisruptionBudget", Object.freeze(["livekit-system"])],
  ["scheduling.k8s.io/v1/PriorityClass", null],
  ["v1/ConfigMap", Object.freeze(["livekit-system"])],
  ["v1/Namespace", null],
  ["v1/Service", Object.freeze(["livekit-system"])],
  ["v1/ServiceAccount", Object.freeze(["cert-manager", "livekit-system"])],
]);

const isRecord = (value) => typeof value === "object" && value !== null && !Array.isArray(value);

export const publicSafeCommandFailure = (stderr, stdout) => {
  const failureOutput =
    typeof stderr === "string" && stderr.trim().length > 0 ? stderr : (stdout ?? "");
  const safeMessage = failureOutput
    .split("\n")
    .filter(
      (line) =>
        !COMMAND_OUTPUT_SECRETISH_LINE.test(line) && !COMMAND_OUTPUT_NETWORK_VALUE.test(line),
    )
    .slice(0, 20)
    .join("\n")
    .trim()
    .slice(0, 2_000);
  if (safeMessage) return safeMessage;
  return failureOutput.trim().length > 0 ? "provider diagnostics were redacted by policy" : "";
};

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

export const runtimeInventoryKey = ({ apiVersion, kind, namespace, name }) =>
  `${apiVersion}/${kind}/${namespace ?? "<cluster>"}/${name}`;

export const validateRuntimeManifestInventory = (value, label) => {
  assert(Array.isArray(value) && value.length > 0, `${label} must contain Kubernetes resources`);
  const keys = new Set();
  for (const [index, resource] of value.entries()) {
    const resourceLabel = `${label}[${index}]`;
    assertExactKeys(resource, ["apiVersion", "kind", "namespace", "name"], [], resourceLabel);
    const apiVersion = assertString(resource.apiVersion, `${resourceLabel}.apiVersion`);
    const kind = assertString(resource.kind, `${resourceLabel}.kind`);
    const name = assertString(resource.name, `${resourceLabel}.name`);
    const resourceType = `${apiVersion}/${kind}`;
    assert(
      RUNTIME_RESOURCE_NAMESPACES.has(resourceType),
      `${resourceLabel} is outside the admitted runtime kind set`,
    );
    const admittedNamespaces = RUNTIME_RESOURCE_NAMESPACES.get(resourceType);
    if (admittedNamespaces === null) {
      assert(
        resource.namespace === null,
        `${resourceLabel} cluster-scoped resource has a namespace`,
      );
      if (resourceType === "v1/Namespace") {
        assert(name === LIVEKIT_OPS.namespace, `${resourceLabel} creates an unsupported namespace`);
      }
    } else {
      assert(
        typeof resource.namespace === "string" && admittedNamespaces.includes(resource.namespace),
        `${resourceLabel} must declare an admitted explicit namespace`,
      );
    }
    const key = runtimeInventoryKey(resource);
    assert(!keys.has(key), `${label} contains duplicate resource ${key}`);
    keys.add(key);
  }
  return [...value].sort((left, right) =>
    runtimeInventoryKey(left).localeCompare(runtimeInventoryKey(right)),
  );
};

export const validateBillingAccountId = (value) => {
  const billingAccountId = assertString(value, "billing account variable");
  assert(
    /^[0-9A-Z]{6}(?:-[0-9A-Z]{6}){2}$/u.test(billingAccountId),
    "billing account variable has an invalid identifier shape",
  );
  return billingAccountId;
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

export function validateServerKeyProjection(value) {
  assertExactKeys(value, ["api_key", "api_secret", "keys_yaml"], [], "server key projection");
  const apiKey = assertString(value.api_key, "server key projection.api_key");
  const apiSecret = assertString(value.api_secret, "server key projection.api_secret");
  const keysYaml = assertString(value.keys_yaml, "server key projection.keys_yaml");
  assert(
    /^API[23456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz]{12}$/u.test(apiKey),
    "server key projection api_key has an unsupported shape",
  );
  assert(
    /^[A-Za-z0-9]{43,52}$/u.test(apiSecret),
    "server key projection api_secret has an unsupported shape",
  );
  const exactMapping = `${apiKey}: ${apiSecret}`;
  assert(
    keysYaml === exactMapping || keysYaml === `${exactMapping}\n`,
    "server key projection keys_yaml is not the exact admitted mapping",
  );
  return value;
}

const productionRedisResourceName = () =>
  `projects/${LIVEKIT_OPS.project}/locations/${LIVEKIT_OPS.region}/instances/oa-livekit-redis`;

const terraformResourceBody = (source, type, name) => {
  assertString(source, "Terraform source");
  const declaration = new RegExp(`\\bresource\\s+"${type}"\\s+"${name}"\\s*\\{`, "gu");
  const matches = [...source.matchAll(declaration)];
  assert(matches.length === 1, `Terraform source must contain one exact ${type}.${name} resource`);

  const bodyStart = matches[0].index + matches[0][0].length;
  let depth = 1;
  let quoted = false;
  let escaped = false;
  let lineComment = false;
  let blockComment = false;
  let body = "";
  for (let index = bodyStart; index < source.length; index += 1) {
    const character = source[index];
    const next = source[index + 1];
    if (lineComment) {
      if (character === "\n") {
        lineComment = false;
        body += character;
      }
      continue;
    }
    if (blockComment) {
      if (character === "*" && next === "/") {
        blockComment = false;
        index += 1;
      } else if (character === "\n") {
        body += character;
      }
      continue;
    }
    if (quoted) {
      body += character;
      if (escaped) {
        escaped = false;
      } else if (character === "\\") {
        escaped = true;
      } else if (character === '"') {
        quoted = false;
      }
      continue;
    }
    if (character === "#") {
      lineComment = true;
      continue;
    }
    if (character === "/" && next === "/") {
      lineComment = true;
      index += 1;
      continue;
    }
    if (character === "/" && next === "*") {
      blockComment = true;
      index += 1;
      continue;
    }
    if (character === '"') {
      quoted = true;
      body += character;
      continue;
    }
    if (character === "{") {
      depth += 1;
      body += character;
    } else if (character === "}") {
      depth -= 1;
      if (depth === 0) return body;
      body += character;
    } else {
      body += character;
    }
  }
  throw new Error(`Terraform source has an unterminated ${type}.${name} resource`);
};

const requireTerraformAssignment = (body, name, expected) => {
  const assignments = [
    ...body.matchAll(new RegExp(`^\\s*${name}\\s*=\\s*([^\\s#]+)\\s*$`, "gmu")),
  ].map((match) => match[1]);
  assert(
    assignments.length === 1 && assignments[0] === expected,
    `Terraform Redis source must set ${name} to ${expected}`,
  );
};

export function validateProductionRedisProjection(redis, redisSecret, terraformSource) {
  assertExactKeys(
    redis,
    ["name", "tier", "state", "transitEncryptionMode", "host", "serverCaCerts"],
    ["region", "authEnabled"],
    "production Redis metadata",
  );
  assertExactKeys(redisSecret, ["host", "ca_cert"], [], "production Redis secret projection");

  const redisResource = terraformResourceBody(terraformSource, "google_redis_instance", "livekit");
  requireTerraformAssignment(redisResource, "tier", '"STANDARD_HA"');
  requireTerraformAssignment(redisResource, "transit_encryption_mode", '"SERVER_AUTHENTICATION"');
  requireTerraformAssignment(redisResource, "auth_enabled", "false");

  assert(
    redis.name === productionRedisResourceName(),
    "production Redis metadata is not the exact project/location/instance resource",
  );
  if (Object.hasOwn(redis, "region")) {
    assert(
      redis.region === LIVEKIT_OPS.region,
      "production Redis metadata region disagrees with its resource name",
    );
  }
  if (Object.hasOwn(redis, "authEnabled")) {
    assert(
      redis.authEnabled === false,
      "production Redis metadata does not explicitly disable AUTH",
    );
  }
  assert(redis.tier === "STANDARD_HA", "production Redis metadata is not STANDARD_HA");
  assert(redis.state === "READY", "production Redis metadata is not READY");
  assert(
    redis.transitEncryptionMode === "SERVER_AUTHENTICATION",
    "production Redis metadata does not require server-authenticated TLS",
  );
  assertString(redis.host, "production Redis metadata.host");
  const redisCertificate = redis.serverCaCerts?.[0]?.cert;
  assertString(redisCertificate, "production Redis metadata.serverCaCerts[0].cert");
  assert(
    redisSecret.host === redis.host && redisSecret.ca_cert.trim() === redisCertificate.trim(),
    "production Redis host/CA secret projection does not match live metadata",
  );
  return redis;
}

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
  return validateDeploymentBundleInternal(value, false);
}

export function validateHistoricalDeploymentBundle(value) {
  return validateDeploymentBundleInternal(value, true);
}

function validateDeploymentBundleInternal(value, historical) {
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
  if (!historical) {
    assert(
      value.sourceBaseRevision === LIVEKIT_OPS.sourceBaseRevision,
      "bundle source base revision is not admitted",
    );
  }
  assertDigest(value.configurationDigest, "bundle.configurationDigest");
  assertDigest(value.renderedManifestDigest, "bundle.renderedManifestDigest");

  assertExactKeys(value.chart, ["version", "sourceCommit", "archiveSha256"], [], "bundle.chart");
  assertString(value.chart.version, "bundle.chart.version");
  assertCommit(value.chart.sourceCommit, "bundle.chart.sourceCommit");
  if (!historical) {
    assert(value.chart.version === LIVEKIT_OPS.chartVersion, "bundle chart version is not pinned");
    assert(
      value.chart.sourceCommit === LIVEKIT_OPS.chartSourceCommit,
      "bundle chart source commit is not pinned",
    );
  }
  assertDigestHex(value.chart.archiveSha256, "bundle.chart.archiveSha256");
  if (!historical) {
    assert(
      `sha256:${value.chart.archiveSha256}` === LIVEKIT_OPS.chartArchiveDigest,
      "bundle chart archive digest is not pinned",
    );
  }

  assertExactKeys(
    value.serverImage,
    ["reference", "digest", "sourceCommit"],
    [],
    "bundle.serverImage",
  );
  const imageReference = assertString(value.serverImage.reference, "bundle.serverImage.reference");
  assert(
    /^docker\.io\/livekit\/livekit-server:[A-Za-z0-9._-]+@sha256:[0-9a-f]{64}$/u.test(
      imageReference,
    ),
    "bundle server image reference is not an immutable LiveKit server image",
  );
  if (!historical) {
    assert(imageReference === LIVEKIT_OPS.serverImage, "bundle server image reference is not pinned");
  }
  assertDigest(value.serverImage.digest, "bundle.serverImage.digest");
  assert(
    imageReference.endsWith(`@${value.serverImage.digest}`),
    "bundle server image reference and digest do not agree",
  );
  assertCommit(value.serverImage.sourceCommit, "bundle.serverImage.sourceCommit");
  if (!historical) {
    assert(
      value.serverImage.digest === LIVEKIT_OPS.serverImageDigest,
      "bundle server image digest is not pinned",
    );
    assert(
      value.serverImage.sourceCommit === LIVEKIT_OPS.serverImageSourceCommit,
      "bundle server image source commit is not pinned",
    );
  }

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
    ["repository", "chart", "version", "chartSha256", "resourceApiVersion", "images"],
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
    value.certManager.images,
    Object.keys(CERT_MANAGER_IMAGES),
    [],
    "addon lock certManager.images",
  );
  for (const [name, image] of Object.entries(CERT_MANAGER_IMAGES)) {
    assert(
      value.certManager.images[name] === image,
      `cert-manager ${name} image is not digest-pinned`,
    );
  }

  assertExactKeys(
    value.externalSecrets,
    ["repository", "chart", "version", "chartSha256", "resourceApiVersion", "image"],
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
  assert(
    value.externalSecrets.image === EXTERNAL_SECRETS_IMAGE,
    "External Secrets image is not digest-pinned",
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
      value.managedPrometheus.binaryVersionAuthority === "gke-stable-release-channel",
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
    assert(
      Array.isArray(value.resourceRefs) &&
        value.resourceRefs.length === LIVEKIT_PREREQUISITE_SERVICE_REFS.length &&
        LIVEKIT_PREREQUISITE_SERVICE_REFS.every(
          (ref, index) => value.resourceRefs[index] === ref,
        ),
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
