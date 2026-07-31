import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
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
    throw new Error(
      `${label}: expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`,
    );
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
const inventoryExpression =
  '[. | select(. != null) | {"apiVersion": .apiVersion, "kind": .kind, "namespace": (.metadata.namespace // null), "name": .metadata.name}]';
const inventoryResult = spawnSync(
  "yq",
  ["eval-all", "-o=json", "-I=0", inventoryExpression, renderedPath],
  {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  },
);
if (inventoryResult.status !== 0) {
  throw new Error("rendered runtime inventory could not be read");
}
const observedInventory = JSON.parse(inventoryResult.stdout)
  .map(
    (resource) =>
      `${resource.apiVersion}/${resource.kind}/${resource.namespace ?? "<cluster>"}/${resource.name}`,
  )
  .sort();
const expectedInventory = [
  "apps/v1/Deployment/livekit-system/livekit-server",
  "apps/v1/Deployment/livekit-system/sarah-livekit-agent",
  "autoscaling/v2/HorizontalPodAutoscaler/livekit-system/livekit-server",
  "autoscaling/v2/HorizontalPodAutoscaler/livekit-system/sarah-livekit-agent",
  "cert-manager.io/v1/Certificate/livekit-system/livekit-turn",
  "cert-manager.io/v1/ClusterIssuer/<cluster>/livekit-public-dns01",
  "cloud.google.com/v1/BackendConfig/livekit-system/livekit-server",
  "external-secrets.io/v1/ExternalSecret/cert-manager/cloudflare-dns-token",
  "external-secrets.io/v1/ExternalSecret/livekit-system/livekit-redis-auth",
  "external-secrets.io/v1/ExternalSecret/livekit-system/livekit-server-keys",
  "external-secrets.io/v1/ExternalSecret/livekit-system/sarah-livekit-agent",
  "external-secrets.io/v1/SecretStore/cert-manager/livekit-google-secret-manager",
  "external-secrets.io/v1/SecretStore/livekit-system/google-secret-manager",
  "external-secrets.io/v1/SecretStore/livekit-system/sarah-google-secret-manager",
  "monitoring.googleapis.com/v1/PodMonitoring/cert-manager/livekit-cert-manager",
  "monitoring.googleapis.com/v1/PodMonitoring/livekit-system/livekit-server",
  "networking.gke.io/v1/ManagedCertificate/livekit-system/livekit-signal",
  "networking.k8s.io/v1/Ingress/livekit-system/livekit-server",
  "networking.k8s.io/v1/NetworkPolicy/livekit-system/sarah-livekit-agent-deny-ingress",
  "policy/v1/PodDisruptionBudget/livekit-system/livekit-server",
  "policy/v1/PodDisruptionBudget/livekit-system/sarah-livekit-agent",
  "scheduling.k8s.io/v1/PriorityClass/<cluster>/livekit-media-critical",
  "v1/ConfigMap/livekit-system/livekit-server",
  "v1/Namespace/<cluster>/livekit-system",
  "v1/Service/livekit-system/livekit-server",
  "v1/Service/livekit-system/livekit-server-turn",
  "v1/ServiceAccount/cert-manager/livekit-cert-manager-secret-reader",
  "v1/ServiceAccount/livekit-system/livekit-secret-reader",
  "v1/ServiceAccount/livekit-system/livekit-server",
  "v1/ServiceAccount/livekit-system/oa-livekit-sarah-secret-reader",
  "v1/ServiceAccount/livekit-system/sarah-agent",
];
requireEqual(
  JSON.stringify(observedInventory),
  JSON.stringify(expectedInventory),
  "exact rendered runtime inventory",
);

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
    "workerImage",
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
requireEqual(
  JSON.stringify(bundle.zones),
  JSON.stringify(["us-central1-a", "us-central1-b", "us-central1-c"]),
  "zones",
);
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
requireEqual(
  bundle.serverImage.sourceCommit,
  pins.serverImage.upstreamSourceCommit,
  "server image source commit",
);
requireEqual(
  pins.serverImage.auditedAnalysisSourceCommit,
  "ced94b8645829263a1a9ef6c8101936897252d6b",
  "later audited analysis source",
);

requireKnownKeys(bundle.workerImage, ["reference", "digest", "pinState"], "workerImage");
requireEqual(
  bundle.workerImage.reference,
  `us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/sarah-livekit-agent@${bundle.workerImage.digest}`,
  "worker image reference",
);
if (!/^sha256:[0-9a-f]{64}$/u.test(bundle.workerImage.digest)) {
  throw new Error("worker image digest must be immutable");
}
if (bundle.workerImage.pinState === "build_required") {
  requireEqual(
    bundle.workerImage.digest,
    `sha256:${"0".repeat(64)}`,
    "build-required worker image placeholder",
  );
} else {
  requireEqual(bundle.workerImage.pinState, "pinned", "worker image pin state");
  if (bundle.workerImage.digest === `sha256:${"0".repeat(64)}`) {
    throw new Error("pinned worker image still uses the placeholder digest");
  }
}

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
    "agentKsa",
    "agentGsa",
    "sarahSecretReaderKsa",
    "sarahSecretReaderGsa",
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
requireEqual(bundle.resources.agentKsa, "sarah-agent", "worker KSA");
requireEqual(
  bundle.resources.agentGsa,
  "oa-livekit-agent@openagentsgemini.iam.gserviceaccount.com",
  "worker GSA",
);
requireEqual(
  bundle.resources.sarahSecretReaderKsa,
  "oa-livekit-sarah-secret-reader",
  "worker secret reader KSA",
);
requireEqual(
  bundle.resources.sarahSecretReaderGsa,
  "oa-livekit-sarah-secret-reader@openagentsgemini.iam.gserviceaccount.com",
  "worker secret reader GSA",
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
  "sarah-livekit-agent",
];
requireEqual(
  JSON.stringify(bundle.secrets),
  JSON.stringify(expectedSecrets),
  "Kubernetes secret refs",
);
requireEqual(
  JSON.stringify(bundle.googleSecretContainers),
  JSON.stringify([
    "oa-livekit-prod-server-keys",
    "oa-livekit-prod-redis-auth",
    "oa-livekit-prod-cloudflare-dns",
    "oa-livekit-prod-openai-api-key",
    "oa-livekit-prod-sarah-control-root",
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
const canaryRoot = await readFile(resolve(repositoryRoot, "infra/livekit-staging/main.tf"), "utf8");
const canaryModule = await readFile(
  resolve(repositoryRoot, "infra/modules/livekit-gce-canary/main.tf"),
  "utf8",
);
const canaryStartup = await readFile(
  resolve(repositoryRoot, "infra/modules/livekit-gce-canary/startup.sh.tftpl"),
  "utf8",
);
const gkeInfrastructure = await readFile(
  resolve(repositoryRoot, "infra/modules/livekit-gke/main.tf"),
  "utf8",
);
const gkeVariables = await readFile(
  resolve(repositoryRoot, "infra/modules/livekit-gke/variables.tf"),
  "utf8",
);
const productionInfrastructure = await readFile(
  resolve(repositoryRoot, "infra/livekit-production/main.tf"),
  "utf8",
);
const productionProviders = await readFile(
  resolve(repositoryRoot, "infra/livekit-production/providers.tf"),
  "utf8",
);
const productionSecretIdentity = await readFile(
  resolve(directory, "production/resources/secret-identity.yaml"),
  "utf8",
);
const sarahAgentIdentity = await readFile(
  resolve(directory, "production/resources/sarah-agent-identity.yaml"),
  "utf8",
);
const sarahAgentRuntime = await readFile(
  resolve(directory, "production/resources/sarah-agent-runtime.yaml"),
  "utf8",
);
const observabilityInfrastructure = await readFile(
  resolve(repositoryRoot, "infra/modules/livekit-observability/main.tf"),
  "utf8",
);
const workerDockerIgnore = await readFile(
  resolve(repositoryRoot, "apps/sarah-livekit-agent/Dockerfile.dockerignore"),
  "utf8",
);
const workerDockerfile = await readFile(
  resolve(repositoryRoot, "apps/sarah-livekit-agent/Dockerfile"),
  "utf8",
);
const workerCloudBuild = await readFile(
  resolve(repositoryRoot, "docker/cloud/cloudbuild-sarah-livekit-agent.yaml"),
  "utf8",
);
const workerBuildScript = await readFile(
  resolve(repositoryRoot, "scripts/cloud/build-sarah-livekit-agent.sh"),
  "utf8",
);
requireEqual(
  bundle.configurationDigest,
  `sha256:${sha256(configurationBytes)}`,
  "configuration digest",
);
requireEqual(
  bundle.renderedManifestDigest,
  `sha256:${sha256(rendered)}`,
  "rendered manifest digest",
);
requireIncludes(
  canaryConfiguration,
  "domain: turn-livekit-staging.openagents.com",
  "canary TURN certificate domain",
);
requireIncludes(workerDockerIgnore, "!apps/sarah-livekit-agent/**", "worker Docker source");
requireIncludes(workerDockerIgnore, "!packages/audio-contract/**", "worker contract source");
requireExcludes(workerDockerIgnore, "!docs", "worker Docker documentation context");
const workerNodeImage =
  "node:24.13.1-bookworm-slim@sha256:85a395c77b811fa7f5b5e4aa69cd6eb4c3b80c7f1a8e34704dc0ce061e5b404e";
requireEqual(
  workerDockerfile.split(workerNodeImage).length - 1,
  2,
  "worker build and runtime Node image pins",
);
requireIncludes(
  workerCloudBuild,
  "us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/sarah-livekit-agent:source-only",
  "worker Cloud Build Artifact Registry repository",
);
requireIncludes(workerCloudBuild, "linux/amd64", "worker Cloud Build platform");
requireExcludes(
  workerCloudBuild,
  "openagentsgemini/livekit/sarah-livekit-agent",
  "nonexistent worker Artifact Registry repository",
);
requireIncludes(
  workerBuildScript,
  "status --porcelain --untracked-files=normal",
  "worker clean-source publication gate",
);
requireIncludes(workerBuildScript, "image_summary.digest", "worker immutable image resolution");
for (const identity of [
  "oa-livekit-agent@openagentsgemini.iam.gserviceaccount.com",
  "oa-livekit-sarah-secret-reader@openagentsgemini.iam.gserviceaccount.com",
]) {
  requireIncludes(sarahAgentIdentity, identity, "Sarah workload identity");
}
for (const secretContainer of [
  "oa-livekit-prod-server-keys",
  "oa-livekit-prod-openai-api-key",
  "oa-livekit-prod-sarah-control-root",
]) {
  requireIncludes(sarahAgentIdentity, secretContainer, "Sarah External Secret source");
}
for (const secretKey of [
  "livekit-api-key",
  "livekit-api-secret",
  "openai-api-key",
  "control-root",
]) {
  requireIncludes(sarahAgentIdentity, `secretKey: ${secretKey}`, "Sarah Kubernetes secret key");
}
requireExcludes(sarahAgentIdentity, "sk-", "raw OpenAI secret");
requireExcludes(sarahAgentIdentity, "stringData:", "raw Kubernetes secret data");
requireIncludes(
  gkeInfrastructure,
  'resource "google_secret_manager_secret" "sarah_control_root"',
  "Sarah control-root Secret Manager container",
);
requireIncludes(
  gkeInfrastructure,
  "sarah_control_root = google_secret_manager_secret.sarah_control_root.secret_id",
  "Sarah control-root least-privilege reader",
);
requireExcludes(canaryConfiguration, "udp_port:", "canary TURN/UDP");
requireIncludes(
  canaryConfiguration,
  "cert_file: /run/livekit/tls.crt",
  "canary TURN certificate path",
);
requireIncludes(
  canaryRoot,
  'livekit_config           = file("${path.module}/../livekit/canary/livekit.yaml")',
  "canary root configuration source",
);
requireIncludes(
  canaryModule,
  "livekit_config_base64     = base64encode(var.livekit_config)",
  "canary module configuration projection",
);
for (const projection of [
  "tcp_fallback_port         = var.tcp_fallback_port",
  "turn_tls_port             = var.turn_tls_port",
  "media_udp_port_start      = var.media_udp_port_range.start",
  "media_udp_port_end        = var.media_udp_port_range.end",
  "enable_turn_udp           = var.enable_turn_udp",
  "turn_udp_port             = var.turn_udp_port",
]) {
  requireIncludes(canaryModule, projection, "canary host firewall variable projection");
}
requireIncludes(
  canaryModule,
  'resource "google_project_iam_member" "canary_log_writer"',
  "canary Cloud Logging IAM binding",
);
requireIncludes(
  canaryModule,
  'role    = "roles/logging.logWriter"',
  "canary least-privilege log writer role",
);
requireIncludes(
  canaryModule,
  'member  = "serviceAccount:${google_service_account.canary.email}"',
  "canary log writer identity",
);
requireIncludes(
  canaryModule,
  "google_project_iam_member.canary_log_writer,",
  "canary VM log writer dependency",
);
requireIncludes(
  canaryStartup,
  "printf '%s' '${livekit_config_base64}' | base64 --decode",
  "canary launcher audited configuration use",
);
requireIncludes(canaryStartup, "--key-file /run/livekit/keys.yaml", "canary key-file use");
for (const rule of [
  'allow_host_port tcp "443"',
  'allow_host_port tcp "${tcp_fallback_port}"',
  'allow_host_port tcp "${turn_tls_port}"',
  'allow_host_port udp "${media_udp_port_start}:${media_udp_port_end}"',
  'allow_host_port udp "${turn_udp_port}"',
]) {
  requireIncludes(canaryStartup, rule, "canary host firewall rule");
}
requireIncludes(
  canaryStartup,
  "if ${enable_turn_udp}; then",
  "canary conditional TURN UDP host firewall rule",
);
requireExcludes(
  canaryStartup,
  'allow_host_port tcp "7880"',
  "canary internal signaling host firewall rule",
);
requireIncludes(
  canaryStartup,
  "caddy run --config /run/livekit/Caddyfile --adapter caddyfile",
  "canary Caddy binary invocation",
);
requireExcludes(
  canaryStartup,
  '"${reverse_proxy_image}" \\\n  run --config /run/livekit/Caddyfile',
  "canary Caddy image command override without executable",
);
requireExcludes(
  canaryStartup,
  'cat >"$runtime_dir/livekit.yaml"',
  "synthesized canary configuration",
);
for (const infrastructureSource of [gkeInfrastructure, gkeVariables, productionInfrastructure]) {
  requireExcludes(
    infrastructureSource,
    "master_ipv4_cidr_block",
    "legacy GKE control-plane CIDR incompatible with Private Service Connect",
  );
}
requireIncludes(
  gkeInfrastructure,
  "private_cluster_config {\n    enable_private_endpoint = false\n    enable_private_nodes    = false\n  }",
  "public GKE control-plane endpoint and nodes",
);
requireIncludes(
  gkeInfrastructure,
  "master_authorized_networks_config {",
  "GKE control-plane authorized networks",
);
requireIncludes(
  gkeVariables,
  "length(var.master_authorized_networks) > 0",
  "non-empty GKE control-plane authorized networks",
);
requireIncludes(
  gkeVariables,
  'network.cidr_block != "0.0.0.0/0"',
  "bounded GKE control-plane authorized networks",
);
requireIncludes(
  productionInfrastructure,
  "master_authorized_networks = var.master_authorized_networks",
  "production GKE control-plane authorized networks projection",
);
requireIncludes(
  gkeInfrastructure,
  'resource "google_compute_address" "turn"',
  "regional TURN address",
);
requireIncludes(gkeInfrastructure, 'address_type = "EXTERNAL"', "external TURN address type");
requireIncludes(gkeInfrastructure, 'network_tier = "PREMIUM"', "premium TURN address tier");
requireExcludes(
  gkeInfrastructure,
  "SHARED_LOADBALANCER_VIP",
  "internal-only purpose on external TURN address",
);
requireIncludes(
  gkeInfrastructure,
  'resource "google_redis_instance" "livekit"',
  "production Redis resource",
);
requireIncludes(gkeInfrastructure, 'tier               = "STANDARD_HA"', "production Redis tier");
requireIncludes(
  gkeInfrastructure,
  'transit_encryption_mode = "SERVER_AUTHENTICATION"',
  "production Redis TLS mode",
);
requireIncludes(
  gkeInfrastructure,
  "auth_enabled            = false",
  "production Redis no-AUTH mode",
);
requireIncludes(
  productionInfrastructure,
  'redis_name           = "oa-livekit-redis"',
  "production Redis instance name",
);
requireIncludes(
  productionInfrastructure,
  'secret_reader_service_account_id       = "livekit-secret-reader"',
  "production Secret Manager reader identity",
);
requireIncludes(
  productionSecretIdentity,
  "iam.gke.io/gcp-service-account: livekit-secret-reader@openagentsgemini.iam.gserviceaccount.com",
  "production Secret Manager reader workload identity annotation",
);
requireExcludes(
  productionSecretIdentity,
  "oa-livekit-secret-reader@openagentsgemini.iam.gserviceaccount.com",
  "nonexistent production Secret Manager reader identity",
);
requireIncludes(
  productionProviders,
  "billing_project       = var.project_id",
  "production Google API quota project",
);
requireIncludes(
  productionProviders,
  "user_project_override = true",
  "production Google API quota project override",
);
const workloadIdentityBindings = new Map(
  [
    ...gkeInfrastructure.matchAll(
      /resource "google_service_account_iam_member" "([^"]+)" \{([\s\S]*?)\n\}/g,
    ),
  ].map((match) => [match[1], match[2]]),
);
const expectedWorkloadIdentityBindings = [
  "server_workload_identity",
  "agent_workload_identity",
  "secret_reader_workload_identity",
  "dns_secret_reader_workload_identity",
  "sarah_secret_reader_workload_identity",
];
requireEqual(
  JSON.stringify([...workloadIdentityBindings.keys()]),
  JSON.stringify(expectedWorkloadIdentityBindings),
  "Workload Identity IAM binding inventory",
);
for (const bindingName of expectedWorkloadIdentityBindings) {
  const binding = workloadIdentityBindings.get(bindingName);
  requireEqual(
    JSON.stringify([...binding.matchAll(/^\s*role\s*=\s*"([^"]+)"$/gm)].map((match) => match[1])),
    JSON.stringify(["roles/iam.workloadIdentityUser"]),
    `${bindingName} least-privilege role`,
  );
  requireIncludes(
    binding,
    "depends_on = [google_container_cluster.livekit]",
    `${bindingName} workload-pool ordering dependency`,
  );
}
requireIncludes(
  gkeInfrastructure,
  'name        = "${var.cluster_name}-redis-allow-sfu"',
  "SFU-only Redis allow boundary",
);
requireIncludes(
  gkeInfrastructure,
  'name      = "${var.cluster_name}-redis-deny-non-sfu"',
  "non-SFU Redis deny boundary",
);
requireIncludes(
  observabilityInfrastructure,
  'state=~\\"signal_failed|signal_validation_failed|signal_upgrade_failed|signal_write_initial_response_failed|rtc_failure\\"',
  "exact LiveKit join failure states",
);
requireIncludes(
  observabilityInfrastructure,
  "certmanager_certificate_expiration_timestamp_seconds",
  "TURN certificate expiry telemetry",
);
requireIncludes(
  observabilityInfrastructure,
  "notification_channels = var.notification_channel_ids",
  "Cloud Monitoring notification routing",
);
requireExcludes(
  observabilityInfrastructure,
  "livekit_connection_total",
  "unsupported transport-path attribution",
);
requireExcludes(
  observabilityInfrastructure,
  "livekit_packet_loss_total[5m]",
  "invalid aggregate packet-loss ratio",
);

for (const entry of bundle.manifests) {
  requireKnownKeys(entry, ["path", "sha256"], `manifest ${entry.path ?? "<missing>"}`);
  if (!entry.path.startsWith("infra/livekit/"))
    throw new Error(`manifest outside bundle: ${entry.path}`);
  const bytes = await readFile(resolve(repositoryRoot, entry.path));
  requireEqual(entry.sha256, `sha256:${sha256(bytes)}`, `manifest digest ${entry.path}`);
}

requireIncludes(rendered, "hostNetwork: true", "host networking");
requireIncludes(
  rendered,
  "cloud.google.com/gke-nodepool: oa-livekit-prod-sfu",
  "node-pool placement",
);
requireIncludes(rendered, "openagents.com/livekit-workload: sfu", "semantic node label");
requireIncludes(rendered, "topologyKey: kubernetes.io/hostname", "one-pod-per-node anti-affinity");
requireIncludes(rendered, "topologyKey: topology.kubernetes.io/zone", "zone spread");
requireIncludes(rendered, "minAvailable: 2", "disruption budget");
requireIncludes(rendered, "maxUnavailable: 0", "rolling update availability");
requireIncludes(rendered, "drainingTimeoutSec: 1800", "signaling connection draining");
requireIncludes(rendered, "minReplicas: 3", "minimum replica count");
requireIncludes(rendered, "maxReplicas: 6", "maximum replica count");
requireIncludes(rendered, pins.serverImage.tagAndDigest, "pinned server image");
requireIncludes(rendered, "auto_create: false", "explicit room creation");
requireIncludes(rendered, "mime: audio/opus", "Opus-only room codec");
requireIncludes(rendered, "ca_cert_file: /etc/livekit/redis/ca.crt", "Redis CA reference");
requireIncludes(rendered, "insecure: false", "Redis TLS verification");
requireIncludes(rendered, "name: REDIS_HOST", "private Redis host reference");
requireIncludes(rendered, "name: livekit-server-keys", "LiveKit key-file secret reference");
requireIncludes(rendered, "name: sarah-livekit-agent", "Sarah worker Deployment");
requireIncludes(rendered, "replicas: 3", "Sarah worker replica floor");
requireIncludes(
  rendered,
  "cloud.google.com/gke-nodepool: oa-livekit-prod-app",
  "Sarah worker app-node placement",
);
requireIncludes(rendered, "serviceAccountName: sarah-agent", "Sarah worker KSA");
requireIncludes(rendered, "containerPort: 8081", "Sarah worker production health port");
requireIncludes(rendered, "startupProbe:", "Sarah worker startup probe");
requireIncludes(rendered, "readinessProbe:", "Sarah worker readiness probe");
requireIncludes(rendered, "livenessProbe:", "Sarah worker liveness probe");
requireIncludes(rendered, "terminationGracePeriodSeconds: 90", "Sarah worker drain allowance");
requireIncludes(rendered, "readOnlyRootFilesystem: true", "Sarah worker read-only root");
requireIncludes(rendered, "kind: HorizontalPodAutoscaler", "Sarah worker autoscaling");
requireIncludes(sarahAgentRuntime, "minReplicas: 3", "Sarah worker HPA floor");
requireIncludes(sarahAgentRuntime, "maxReplicas: 4", "Sarah worker HPA ceiling");
requireIncludes(rendered, "kind: NetworkPolicy", "Sarah worker ingress isolation");
requireIncludes(rendered, bundle.workerImage.reference, "Sarah worker image pin");
requireIncludes(sarahAgentRuntime, bundle.workerImage.reference, "Sarah worker source image pin");
requireIncludes(rendered, "key_file: /etc/livekit/keys.yaml", "absolute LiveKit key-file path");
requireIncludes(rendered, "mountPath: /etc/livekit/keys.yaml", "absolute key-file mount");
requireIncludes(rendered, "kind: ClusterIssuer", "TURN certificate issuer");
requireIncludes(rendered, "kind: Certificate", "TURN certificate");
requireIncludes(rendered, "kind: PodMonitoring", "managed Prometheus scrape");
requireExcludes(rendered, "kind: Rules", "unrouted in-cluster alert rules");
requireIncludes(rendered, "kind: Ingress", "signaling ingress");
requireIncludes(rendered, "cloud.google.com/neg: '{\"ingress\": true}'", "signaling NEG");
requireIncludes(
  rendered,
  "kubernetes.io/ingress.global-static-ip-name: oa-livekit-prod-signal",
  "signaling address pin",
);
requireIncludes(
  rendered,
  "networking.gke.io/managed-certificates: livekit-signal",
  "signaling certificate pin",
);
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
requireEqual(
  addons.externalSecrets.resourceApiVersion,
  "external-secrets.io/v1",
  "External Secrets API",
);
requireEqual(
  addons.managedPrometheus.resourceApiVersion,
  "monitoring.googleapis.com/v1",
  "managed Prometheus API",
);

process.stdout.write(`verified ${renderedPath}\n`);
