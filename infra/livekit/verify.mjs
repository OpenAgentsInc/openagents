import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const bundlePath = resolve(process.argv[2] ?? "");
const renderedPath = resolve(process.argv[3] ?? "");
const directory = dirname(fileURLToPath(import.meta.url));
const repositoryRoot = resolve(directory, "../..");

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const readJson = async (path) => JSON.parse(await readFile(path, "utf8"));
const requireEqual = (actual, expected, label) => {
  if (actual !== expected) {
    throw new Error(`${label}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`);
  }
};
const requireIncludes = (source, expected, label) => {
  if (!source.includes(expected)) {
    throw new Error(`${label}: missing ${JSON.stringify(expected)}`);
  }
};
const requireExcludes = (source, rejected, label) => {
  if (source.includes(rejected)) {
    throw new Error(`${label}: rejected ${JSON.stringify(rejected)}`);
  }
};
const requireKnownKeys = (value, keys, label) => {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  requireEqual(JSON.stringify(actual), JSON.stringify(expected), `${label} keys`);
};

const bundle = await readJson(bundlePath);
const pins = await readJson(resolve(directory, "pins.lock.json"));
const addons = await readJson(resolve(directory, "addons.lock.json"));
const rendered = await readFile(renderedPath, "utf8");

requireKnownKeys(
  bundle,
  [
    "schemaVersion",
    "sourceState",
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
    "pendingDependencies",
    "limits",
    "manifests",
  ],
  "bundle",
);
requireEqual(bundle.schemaVersion, "openagents.livekit_deployment_bundle.v1", "bundle schema");
requireEqual(bundle.sourceState, "source_only", "bundle source state");
requireEqual(bundle.project, "openagentsgemini", "project");
requireEqual(bundle.region, "us-central1", "region");
requireEqual(JSON.stringify(bundle.zones), JSON.stringify(["us-central1-a", "us-central1-b", "us-central1-c"]), "zones");
requireEqual(bundle.release, "livekit-server", "release");
requireEqual(bundle.namespace, "livekit-system", "namespace");
if (!/^[0-9a-f]{40}$/.test(bundle.sourceBaseRevision)) {
  throw new Error("sourceBaseRevision must be the 40-hex review base");
}

requireKnownKeys(bundle.chart, ["version", "sourceCommit", "archiveSha256"], "chart");
requireEqual(bundle.chart.version, pins.helm.chartVersion, "chart version");
requireEqual(bundle.chart.sourceCommit, pins.helm.sourceCommit, "chart source commit");
requireEqual(bundle.chart.archiveSha256, pins.helm.archiveSha256, "chart archive digest");

requireKnownKeys(bundle.serverImage, ["reference", "digest", "sourceCommit"], "serverImage");
requireEqual(bundle.serverImage.reference, pins.serverImage.reference, "server image reference");
requireEqual(bundle.serverImage.digest, pins.serverImage.ociIndexDigest, "server image digest");
requireEqual(bundle.serverImage.sourceCommit, pins.serverImage.upstreamSourceCommit, "server image source commit");
requireEqual(
  pins.serverImage.auditedAnalysisSourceCommit,
  "ced94b8645829263a1a9ef6c8101936897252d6b",
  "later audited analysis source",
);

requireKnownKeys(
  bundle.resources,
  [
    "cluster",
    "sfuNodePool",
    "agentNodePool",
    "redis",
    "signalingAddress",
    "turnAddress",
    "signalingService",
    "turnService",
    "serverKsa",
    "serverGsa",
  ],
  "resources",
);
requireEqual(bundle.resources.cluster, "oa-livekit-prod", "cluster");
requireEqual(bundle.resources.sfuNodePool, "oa-livekit-prod-sfu", "SFU node pool");
requireEqual(bundle.resources.agentNodePool, "oa-livekit-prod-app", "agent node pool");
requireEqual(bundle.resources.redis, "oa-livekit-redis", "Redis");
requireEqual(bundle.resources.signalingAddress, "oa-livekit-prod-signal", "signaling address");
requireEqual(bundle.resources.turnAddress, "oa-livekit-prod-turn", "TURN address");
requireEqual(bundle.resources.signalingService, "livekit-server", "signaling service");
requireEqual(bundle.resources.turnService, "livekit-server-turn", "TURN service");
requireEqual(bundle.resources.serverKsa, "livekit-server", "server KSA");
requireEqual(
  bundle.resources.serverGsa,
  "oa-livekit-server@openagentsgemini.iam.gserviceaccount.com",
  "server GSA",
);

requireKnownKeys(
  bundle.limits,
  [
    "maxConcurrentSarahRooms",
    "maxOwnerPrivateRooms",
    "maxCommunityRoomsPerCommunity",
    "idleTimeoutSeconds",
    "maxRoomLifetimeSeconds",
  ],
  "limits",
);
requireEqual(bundle.limits.maxConcurrentSarahRooms, 20, "concurrent room target");
requireEqual(bundle.limits.maxOwnerPrivateRooms, 1, "owner room limit");
requireEqual(bundle.limits.maxCommunityRoomsPerCommunity, 2, "community room limit");
requireEqual(bundle.limits.idleTimeoutSeconds, 120, "idle timeout");
requireEqual(bundle.limits.maxRoomLifetimeSeconds, 1800, "room lifetime");

const expectedSecrets = [
  "livekit-server-keys",
  "livekit-redis-auth",
  "livekit-turn-tls",
  "cloudflare-dns-token",
];
requireEqual(JSON.stringify(bundle.secrets), JSON.stringify(expectedSecrets), "Kubernetes secret refs");
requireEqual(
  JSON.stringify(bundle.googleSecretContainers),
  JSON.stringify([
    "oa-livekit-prod-server-keys",
    "oa-livekit-prod-redis-auth",
    "oa-livekit-prod-cloudflare-dns",
    "oa-livekit-prod-openai-api-key",
  ]),
  "Google Secret Manager containers",
);
requireEqual(
  JSON.stringify(bundle.pendingDependencies),
  JSON.stringify(["packaged_omega_acceptance", "sarah_worker_acceptance"]),
  "pending source-only acceptance dependencies",
);

const configurationBytes = await readFile(resolve(directory, "production/livekit.yaml"));
const canaryConfiguration = await readFile(resolve(directory, "canary/livekit.yaml"), "utf8");
requireEqual(bundle.configurationDigest, `sha256:${sha256(configurationBytes)}`, "configuration digest");
requireEqual(bundle.renderedManifestDigest, `sha256:${sha256(rendered)}`, "rendered manifest digest");
requireIncludes(
  canaryConfiguration,
  "domain: turn-livekit-staging.openagents.com",
  "canary TURN certificate domain",
);
requireExcludes(canaryConfiguration, "udp_port:", "canary TURN/UDP");

for (const entry of bundle.manifests) {
  requireKnownKeys(entry, ["path", "sha256"], `manifest ${entry.path ?? "<missing>"}`);
  if (!entry.path.startsWith("infra/livekit/")) throw new Error(`manifest outside bundle: ${entry.path}`);
  const bytes = await readFile(resolve(repositoryRoot, entry.path));
  requireEqual(entry.sha256, `sha256:${sha256(bytes)}`, `manifest digest ${entry.path}`);
}

requireIncludes(rendered, "hostNetwork: true", "host networking");
requireIncludes(rendered, "cloud.google.com/gke-nodepool: oa-livekit-prod-sfu", "node-pool placement");
requireIncludes(rendered, "openagents.com/livekit-workload: sfu", "semantic node label");
requireIncludes(rendered, "topologyKey: kubernetes.io/hostname", "one-pod-per-node anti-affinity");
requireIncludes(rendered, "topologyKey: topology.kubernetes.io/zone", "zone spread");
requireIncludes(rendered, "minAvailable: 2", "disruption budget");
requireIncludes(rendered, "maxUnavailable: 0", "rolling update availability");
requireIncludes(rendered, "minReplicas: 3", "minimum replica count");
requireIncludes(rendered, "maxReplicas: 6", "maximum replica count");
requireIncludes(rendered, pins.serverImage.tagAndDigest, "pinned server image");
requireIncludes(rendered, "auto_create: false", "explicit room creation");
requireIncludes(rendered, "mime: audio/opus", "Opus-only room codec");
requireIncludes(rendered, "ca_cert_file: /etc/livekit/redis/ca.crt", "Redis CA reference");
requireIncludes(rendered, "insecure: false", "Redis TLS verification");
requireIncludes(rendered, "name: REDIS_HOST", "private Redis host reference");
requireIncludes(rendered, "name: livekit-server-keys", "LiveKit key-file secret reference");
requireIncludes(rendered, "key_file: /etc/livekit/keys.yaml", "absolute LiveKit key-file path");
requireIncludes(rendered, "mountPath: /etc/livekit/keys.yaml", "absolute key-file mount");
requireIncludes(rendered, "kind: ClusterIssuer", "TURN certificate issuer");
requireIncludes(rendered, "kind: Certificate", "TURN certificate");
requireIncludes(rendered, "kind: PodMonitoring", "managed Prometheus scrape");
requireIncludes(rendered, "kind: Rules", "managed Prometheus alerts");
requireIncludes(rendered, "kind: Ingress", "signaling ingress");
requireIncludes(rendered, "cloud.google.com/neg: '{\"ingress\": true}'", "signaling NEG");
requireIncludes(rendered, "kubernetes.io/ingress.global-static-ip-name: oa-livekit-prod-signal", "signaling address pin");
requireIncludes(rendered, "networking.gke.io/managed-certificates: livekit-signal", "signaling certificate pin");
requireIncludes(rendered, "host: livekit.openagents.com", "signaling hostname");
requireIncludes(rendered, "name: livekit-server-turn", "TURN service");
requireIncludes(rendered, "port: 443", "public TURN/TLS port");
requireIncludes(rendered, "targetPort: 5349", "internal TURN/TLS port");
requireExcludes(rendered, "--disable-strict-config", "strict LiveKit configuration");
requireExcludes(rendered, "REDIS_PASSWORD", "Terraform-state-bearing Redis AUTH");
requireExcludes(rendered, "secretKey: password", "Redis password secret projection");
requireExcludes(rendered, "property: password", "Redis password source property");
requireExcludes(rendered, "mountPath: keys.yaml", "relative key-file mount");
requireExcludes(rendered, "turn-udp", "TURN/UDP");
requireExcludes(rendered, "udp_port:", "TURN/UDP configuration");
requireExcludes(rendered, ":latest", "mutable image tag");
requireExcludes(rendered, "kind: Secret\n", "raw Kubernetes Secret");
requireExcludes(rendered, "sk-", "provider secret marker");

requireEqual(addons.certManager.resourceApiVersion, "cert-manager.io/v1", "cert-manager API");
requireEqual(addons.externalSecrets.resourceApiVersion, "external-secrets.io/v1", "External Secrets API");
requireEqual(addons.managedPrometheus.resourceApiVersion, "monitoring.googleapis.com/v1", "managed Prometheus API");

process.stdout.write(`verified ${renderedPath}\n`);
