#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, resolve, sep } from "node:path";
import {
  LIVEKIT_OPS,
  assertPublicSafe,
  sha256,
  validateAddonLock,
  validateDeploymentBundle,
  validateHistoricalDeploymentBundle,
  validateServerKeyProjection,
  validateSourceOnlyReceipt,
} from "./livekit-ops-policy.mjs";

const OPERATIONS = new Set([
  "validate-source",
  "canary-plan",
  "canary-infra-apply",
  "canary-apply",
  "canary-destroy",
  "production-plan",
  "production-infra-apply",
  "production-runtime-apply",
  "production-rollback",
]);

const CANARY_TERRAFORM_ROOT = "infra/livekit-staging";
const PRODUCTION_TERRAFORM_ROOT = "infra/livekit-production";
const RENDER_SCRIPT = "infra/livekit/render.sh";
const VERIFY_SCRIPT = "infra/livekit/verify.sh";
const ADDON_LOCK = "infra/livekit/addons.lock.json";
const RENDERED_MANIFEST = "livekit-production.yaml";
const CANARY_ZONE = "us-central1-a";
const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";
const CANARY_SIGNAL_HOSTNAME = "livekit-staging.openagents.com";
const CANARY_TURN_HOSTNAME = "turn-livekit-staging.openagents.com";
const PRODUCTION_SIGNAL_HOSTNAME = "livekit.openagents.com";
const PRODUCTION_TURN_HOSTNAME = "turn.livekit.openagents.com";
const CANARY_SECRET_IDS = Object.freeze([
  "oa-livekit-canary-api-key",
  "oa-livekit-canary-api-secret",
  "oa-livekit-canary-tls-certificate",
  "oa-livekit-canary-tls-private-key",
]);
const PRODUCTION_SECRET_IDS = Object.freeze([
  "oa-livekit-prod-server-keys",
  "oa-livekit-prod-redis-auth",
  "oa-livekit-prod-cloudflare-dns",
  "oa-livekit-prod-openai-api-key",
]);
const SARAH_OPENAI_SOURCE_SECRET = "sarah-openai-api-key";

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-gcp-ops.mjs \\
    --operation validate-source|canary-plan|canary-infra-apply|canary-apply|canary-destroy|production-plan|production-infra-apply|production-runtime-apply|production-rollback \\
    --bundle infra/livekit/bundle.json \\
    [--receipt docs/ops/receipts/livekit/<name>.json] \\
    [--current-manifest PATH --previous-bundle PATH --previous-manifest PATH \\
      --previous-deployment-receipt PATH --admission-receipt PATH] \\
    [--apply]

Default mode validates local source and prints an exact command plan. It does
not read or mutate Google Cloud, Kubernetes, DNS, or provider state.

Every live read or mutation requires both --apply and:
  OA_LIVEKIT_OWNER_GATE=I_ACCEPT_EP263_LIVEKIT_GCP_COST

canary-apply is the second phase after canary-infra-apply and refuses until
the four secret versions, DNS, trusted certificate, and empty VM slot pass.

production-runtime-apply is the second phase after production-infra-apply. It
refuses until secret versions and DNS match the reserved addresses, then uses
Helm 3 to install the pinned addons before the runtime. production-rollback
additionally requires a previous immutable bundle, its rendered manifest, and
a private admission-disable receipt.
`);
};

const parseArgs = (args) => {
  const parsed = {
    admissionReceipt: undefined,
    apply: false,
    bundle: undefined,
    currentManifest: undefined,
    operation: undefined,
    previousBundle: undefined,
    previousDeploymentReceipt: undefined,
    previousManifest: undefined,
    receipt: undefined,
  };
  const valueFlags = new Map([
    ["--admission-receipt", "admissionReceipt"],
    ["--bundle", "bundle"],
    ["--current-manifest", "currentManifest"],
    ["--operation", "operation"],
    ["--previous-bundle", "previousBundle"],
    ["--previous-deployment-receipt", "previousDeploymentReceipt"],
    ["--previous-manifest", "previousManifest"],
    ["--receipt", "receipt"],
  ]);
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    const key = valueFlags.get(argument);
    if (!key) throw new Error(`unsupported argument ${argument}`);
    const value = args[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    parsed[key] = value;
    index += 1;
  }
  if (!parsed.operation || !OPERATIONS.has(parsed.operation)) {
    throw new Error("--operation is missing or unsupported");
  }
  if (!parsed.bundle) throw new Error("--bundle is required");
  if (parsed.apply && parsed.operation !== "validate-source" && !parsed.receipt) {
    throw new Error("a live operation requires --receipt");
  }
  if (!parsed.apply && parsed.receipt) {
    throw new Error("--receipt is accepted only with --apply");
  }
  if (parsed.operation === "production-rollback") {
    if (
      !parsed.currentManifest ||
      !parsed.previousBundle ||
      !parsed.previousDeploymentReceipt ||
      !parsed.previousManifest ||
      !parsed.admissionReceipt
    ) {
      throw new Error(
        "production-rollback requires current/previous manifests, previous bundle/deployment receipt, and admission receipt",
      );
    }
  } else if (
    parsed.currentManifest ||
    parsed.previousBundle ||
    parsed.previousDeploymentReceipt ||
    parsed.previousManifest ||
    parsed.admissionReceipt
  ) {
    throw new Error("previous/admission inputs are accepted only for production-rollback");
  }
  return parsed;
};

const readJson = (path, label) => {
  try {
    return JSON.parse(readFileSync(resolve(path), "utf8"));
  } catch (error) {
    throw new Error(`${label} is not readable JSON`, { cause: error });
  }
};

const requireRepositoryPath = (path, expected) => {
  const absolute = resolve(path);
  const expectedAbsolute = resolve(expected);
  if (absolute !== expectedAbsolute) {
    throw new Error(`${path} must resolve to the exact repository path ${expected}`);
  }
  if (!existsSync(absolute)) throw new Error(`required path does not exist: ${expected}`);
  return absolute;
};

const validateManifestDigests = (bundle) => {
  for (const manifest of bundle.manifests) {
    const path = resolve(manifest.path);
    if (!path.startsWith(`${resolve("infra/livekit")}${sep}`)) {
      throw new Error(`manifest escapes infra/livekit: ${manifest.path}`);
    }
    if (!existsSync(path)) throw new Error(`bundle manifest is missing: ${manifest.path}`);
    const observed = sha256(readFileSync(path));
    if (observed !== manifest.sha256) {
      throw new Error(`bundle manifest digest changed: ${manifest.path}`);
    }
  }
};

const validateRenderedManifest = (path, expectedDigest) => {
  const bytes = readFileSync(path);
  const observedDigest = sha256(bytes);
  if (observedDigest !== expectedDigest) {
    throw new Error("rendered production manifest does not match the immutable bundle");
  }
  const text = bytes.toString("utf8");
  const forbidden = [
    /\bstringData\s*:/u,
    /^\s*data\s*:\s*$/mu,
    /-----BEGIN [A-Z ]*PRIVATE KEY-----/u,
    /(?:sk|sess|pat|ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_-]{12,}/u,
    /\b(?:OPENAI_API_KEY|LIVEKIT_API_SECRET|REDIS_PASSWORD)\s*:\s*[^$<{]/u,
  ];
  for (const pattern of forbidden) {
    if (pattern.test(text)) {
      throw new Error(
        `rendered production manifest contains forbidden inline material: ${pattern}`,
      );
    }
  }
  for (const requirement of [
    "namespace: livekit-system",
    "hostNetwork: true",
    "livekit-server-keys",
    "livekit-redis-auth",
    "livekit-turn-tls",
    "cloudflare-dns-token",
  ]) {
    if (!text.includes(requirement)) {
      throw new Error(`rendered production manifest is missing ${requirement}`);
    }
  }
  return observedDigest;
};

const ADMITTED_RUNTIME_KINDS = new Set([
  "apps/v1/Deployment",
  "autoscaling/v2/HorizontalPodAutoscaler",
  "cert-manager.io/v1/Certificate",
  "cert-manager.io/v1/ClusterIssuer",
  "cloud.google.com/v1/BackendConfig",
  "external-secrets.io/v1/ExternalSecret",
  "external-secrets.io/v1/SecretStore",
  "monitoring.googleapis.com/v1/PodMonitoring",
  "networking.gke.io/v1/ManagedCertificate",
  "networking.k8s.io/v1/Ingress",
  "policy/v1/PodDisruptionBudget",
  "scheduling.k8s.io/v1/PriorityClass",
  "v1/ConfigMap",
  "v1/Namespace",
  "v1/Service",
  "v1/ServiceAccount",
]);
const ADMITTED_CLUSTER_SCOPED_KINDS = new Set(["ClusterIssuer", "Namespace", "PriorityClass"]);

const inventoryKey = ({ apiVersion, kind, namespace, name }) =>
  `${apiVersion}/${kind}/${namespace ?? "<cluster>"}/${name}`;

const validateManifestInventory = (value, label) => {
  if (!Array.isArray(value) || value.length === 0) {
    throw new Error(`${label} must contain at least one Kubernetes resource`);
  }
  const keys = new Set();
  for (const [index, resource] of value.entries()) {
    requireExactKeys(
      resource,
      ["apiVersion", "kind", "namespace", "name"],
      `${label}[${index}]`,
    );
    if (
      typeof resource.apiVersion !== "string" ||
      typeof resource.kind !== "string" ||
      typeof resource.name !== "string" ||
      resource.name === "" ||
      !ADMITTED_RUNTIME_KINDS.has(`${resource.apiVersion}/${resource.kind}`)
    ) {
      throw new Error(`${label}[${index}] is outside the admitted runtime kind set`);
    }
    if (ADMITTED_CLUSTER_SCOPED_KINDS.has(resource.kind)) {
      if (resource.namespace !== null) {
        throw new Error(`${label}[${index}] cluster-scoped resource has a namespace`);
      }
    } else if (!["livekit-system", "cert-manager"].includes(resource.namespace)) {
      throw new Error(`${label}[${index}] has an unsupported namespace`);
    }
    const key = inventoryKey(resource);
    if (keys.has(key)) throw new Error(`${label} contains duplicate resource ${key}`);
    keys.add(key);
  }
  return [...value].sort((left, right) => inventoryKey(left).localeCompare(inventoryKey(right)));
};

const manifestInventory = (path, label) => {
  const expression =
    '[. | select(. != null) | {"apiVersion": .apiVersion, "kind": .kind, "namespace": (.metadata.namespace // null), "name": .metadata.name}]';
  let inventory;
  try {
    inventory = JSON.parse(
      captureCommand(
        "yq",
        ["eval-all", "-o=json", "-I=0", expression, resolve(path)],
        `read ${label} inventory`,
      ),
    );
  } catch (error) {
    throw new Error(`${label} inventory is not readable JSON`, { cause: error });
  }
  return validateManifestInventory(inventory, label);
};

const writePruneInventory = (path, inventory) => {
  const value = {
    apiVersion: "v1",
    kind: "List",
    items: inventory.map((resource) => ({
      apiVersion: resource.apiVersion,
      kind: resource.kind,
      metadata: {
        name: resource.name,
        ...(resource.namespace === null ? {} : { namespace: resource.namespace }),
      },
    })),
  };
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, {
    encoding: "utf8",
    mode: 0o600,
  });
};

const validatePreviousDeploymentReceipt = (value, bundle, expectedAddonRefs) => {
  validateSourceOnlyReceipt(value);
  if (
    value.stage !== "production" ||
    value.phase !== "deployment" ||
    value.outcome !== "passed" ||
    value.evidenceTier !== "live_observed" ||
    value.bundleDigest !== sha256(JSON.stringify(bundle)) ||
    value.configurationDigest !== bundle.configurationDigest ||
    expectedAddonRefs.some((ref) => !value.resourceRefs.includes(ref))
  ) {
    throw new Error("previous deployment receipt does not bind the rollback target bundle");
  }
  return value;
};

const validateAdmissionReceipt = (value, bundle) => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("admission receipt must be an object");
  }
  const exactKeys = [
    "schemaVersion",
    "stage",
    "sourceBaseRevision",
    "deployedRevision",
    "observedAt",
    "resourceRef",
    "newAdmissionDisabled",
    "newDispatchDisabled",
    "activeRoomCount",
    "pendingSettlementCount",
  ];
  const keys = Object.keys(value).toSorted();
  if (JSON.stringify(keys) !== JSON.stringify(exactKeys.toSorted())) {
    throw new Error("admission receipt fields do not match the closed schema");
  }
  if (value.schemaVersion !== "openagents.livekit_admission_disable.v1") {
    throw new Error("admission receipt schema is unsupported");
  }
  if (
    value.stage !== "production" ||
    value.sourceBaseRevision !== bundle.sourceBaseRevision ||
    !/^[0-9a-f]{40}$/u.test(value.deployedRevision)
  ) {
    throw new Error("admission receipt does not bind the current production revision");
  }
  if (!Number.isFinite(Date.parse(value.observedAt))) {
    throw new Error("admission receipt observedAt is invalid");
  }
  if (Date.now() - Date.parse(value.observedAt) > 15 * 60_000) {
    throw new Error("admission receipt is older than 15 minutes");
  }
  if (
    value.resourceRef !== "livekit-admission-ref://production/livekit-room-v1" ||
    value.newAdmissionDisabled !== true ||
    value.newDispatchDisabled !== true ||
    value.activeRoomCount !== 0 ||
    value.pendingSettlementCount !== 0
  ) {
    throw new Error("admission receipt does not prove a fully drained production boundary");
  }
  assertPublicSafe(value);
  return value;
};

const command = (bin, args, options = {}) => ({
  bin,
  args,
  cwd: options.cwd,
  label: options.label ?? `${bin} ${args[0] ?? ""}`,
});

const terraformCommands = (root, action, extraArguments = []) => {
  requireRepositoryPath(root, root);
  const base = [`-chdir=${root}`];
  return [
    command("tofu", [...base, "init", "-input=false"], { label: `initialize ${root}` }),
    command(
      "tofu",
      action === "plan"
        ? [...base, "plan", "-input=false", "-lock-timeout=60s", ...extraArguments]
        : action === "destroy"
          ? [...base, "destroy", "-auto-approve", "-input=false", "-lock-timeout=60s"]
          : [
              ...base,
              "apply",
              "-auto-approve",
              "-input=false",
              "-lock-timeout=60s",
              ...extraArguments,
            ],
      { label: `${action} ${root}` },
    ),
  ];
};

const productionInfrastructureValidationCommands = () => [
  command("gcloud", [
    "container",
    "clusters",
    "describe",
    "oa-livekit-prod",
    "--project",
    LIVEKIT_OPS.project,
    "--region",
    LIVEKIT_OPS.region,
    "--format=value(name,location,autopilot.enabled)",
  ]),
  command("gcloud", [
    "container",
    "node-pools",
    "describe",
    "oa-livekit-prod-sfu",
    "--cluster",
    "oa-livekit-prod",
    "--project",
    LIVEKIT_OPS.project,
    "--region",
    LIVEKIT_OPS.region,
    "--format=value(name,config.machineType,initialNodeCount)",
  ]),
  command("gcloud", [
    "container",
    "node-pools",
    "describe",
    "oa-livekit-prod-app",
    "--cluster",
    "oa-livekit-prod",
    "--project",
    LIVEKIT_OPS.project,
    "--region",
    LIVEKIT_OPS.region,
    "--format=value(name,config.machineType,initialNodeCount)",
  ]),
  command("gcloud", [
    "redis",
    "instances",
    "describe",
    "oa-livekit-redis",
    "--project",
    LIVEKIT_OPS.project,
    "--region",
    LIVEKIT_OPS.region,
    "--format=value(name,region,tier,state,transitEncryptionMode)",
  ]),
];

const kubectlEnv = (kubeconfig) => ({ ...process.env, KUBECONFIG: kubeconfig });

const productionCredentialsCommand = (kubeconfig) =>
  command(
    "gcloud",
    [
      "container",
      "clusters",
      "get-credentials",
      "oa-livekit-prod",
      "--project",
      LIVEKIT_OPS.project,
      "--region",
      LIVEKIT_OPS.region,
    ],
    { label: `bind temporary kubeconfig ${kubeconfig}` },
  );

const renderProduction = (bundle, temporaryDirectory, execute) => {
  requireRepositoryPath(VERIFY_SCRIPT, VERIFY_SCRIPT);
  requireRepositoryPath(RENDER_SCRIPT, RENDER_SCRIPT);
  const plan = [
    command("bash", [VERIFY_SCRIPT]),
    command("bash", [RENDER_SCRIPT, "--output", temporaryDirectory]),
  ];
  if (execute) {
    executeCommands(plan, process.env);
    validateRenderedManifest(
      resolve(temporaryDirectory, RENDERED_MANIFEST),
      bundle.renderedManifestDigest,
    );
  }
  return plan;
};

const publicSafeLines = (output) =>
  (output ?? "")
    .split("\n")
    .filter(
      (line) =>
        !/(address|authorization|cookie|credential|email|endpoint|password|private.?key|secret|token|transcript|audio|prompt)/iu.test(
          line,
        ) && !/(?:https?|wss?):\/\/|\b(?:\d{1,3}\.){3}\d{1,3}\b/iu.test(line),
    )
    .slice(0, 20)
    .join("\n")
    .trim();

const executeCommands = (commands, environment) => {
  for (const step of commands) {
    process.stdout.write(`livekit-ops: ${step.label}\n`);
    const result = spawnSync(step.bin, step.args, {
      cwd: step.cwd,
      env: environment,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    });
    if (result.status !== 0) {
      const message = publicSafeLines(result.stderr || result.stdout).slice(0, 2_000);
      throw new Error(`${step.label} failed${message ? `: ${message}` : ""}`);
    }
    const safeOutput = publicSafeLines(result.stdout);
    if (safeOutput) process.stdout.write(`${safeOutput}\n`);
  }
};

const captureCommand = (bin, args, label) => {
  const result = spawnSync(bin, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) {
    const message = publicSafeLines(result.stderr || result.stdout).slice(0, 2_000);
    throw new Error(`${label} failed${message ? `: ${message}` : ""}`);
  }
  return result.stdout;
};

const captureCommandWithEnvironment = (bin, args, label, environment) => {
  const result = spawnSync(bin, args, {
    encoding: "utf8",
    env: environment,
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) {
    const message = publicSafeLines(result.stderr || result.stdout).slice(0, 2_000);
    throw new Error(`${label} failed${message ? `: ${message}` : ""}`);
  }
  return result.stdout;
};

const inventoryFromKubernetesList = (value, label) => {
  const items = value?.kind === "List" && Array.isArray(value.items) ? value.items : [value];
  return validateManifestInventory(
    items.map((item) => ({
      apiVersion: item.apiVersion,
      kind: item.kind,
      namespace: item.metadata?.namespace ?? null,
      name: item.metadata?.name,
    })),
    label,
  );
};

const verifyRollbackRuntime = ({
  kubeconfig,
  previousManifest,
  pruneManifest,
  prunedInventory,
  targetBundle,
  targetInventory,
}) => {
  const environment = kubectlEnv(kubeconfig);
  let observedResources;
  try {
    observedResources = JSON.parse(
      captureCommandWithEnvironment(
        "kubectl",
        ["get", "-f", previousManifest, "-o", "json"],
        "read restored runtime inventory",
        environment,
      ),
    );
  } catch (error) {
    throw new Error("restored runtime inventory is not valid JSON", { cause: error });
  }
  const observedInventory = inventoryFromKubernetesList(
    observedResources,
    "restored runtime inventory",
  );
  if (
    JSON.stringify(observedInventory.map(inventoryKey)) !==
    JSON.stringify(targetInventory.map(inventoryKey))
  ) {
    throw new Error("restored runtime inventory does not equal the target manifest");
  }
  if (
    prunedInventory.length > 0 &&
    captureCommandWithEnvironment(
      "kubectl",
      ["get", "-f", pruneManifest, "--ignore-not-found", "-o", "name"],
      "verify current-only runtime resources were pruned",
      environment,
    ).trim() !== ""
  ) {
    throw new Error("current-only runtime resources survived rollback pruning");
  }

  let deployment;
  let pods;
  try {
    deployment = JSON.parse(
      captureCommandWithEnvironment(
        "kubectl",
        [
          "--namespace",
          LIVEKIT_OPS.namespace,
          "get",
          `deployment/${LIVEKIT_OPS.release}`,
          "-o",
          "json",
        ],
        "read restored LiveKit deployment",
        environment,
      ),
    );
    pods = JSON.parse(
      captureCommandWithEnvironment(
        "kubectl",
        [
          "--namespace",
          LIVEKIT_OPS.namespace,
          "get",
          "pods",
          "--selector=app.kubernetes.io/name=livekit-server,app.kubernetes.io/instance=livekit-server",
          "-o",
          "json",
        ],
        "read restored LiveKit pods",
        environment,
      ),
    );
  } catch (error) {
    throw new Error("restored LiveKit workload status is not valid JSON", { cause: error });
  }
  const desiredReplicas = deployment.spec?.replicas;
  const container = deployment.spec?.template?.spec?.containers?.find(
    (candidate) => candidate.name === LIVEKIT_OPS.release,
  );
  if (
    !Number.isSafeInteger(desiredReplicas) ||
    desiredReplicas < 1 ||
    deployment.status?.observedGeneration !== deployment.metadata?.generation ||
    deployment.status?.updatedReplicas !== desiredReplicas ||
    deployment.status?.availableReplicas !== desiredReplicas ||
    (deployment.status?.unavailableReplicas ?? 0) !== 0 ||
    typeof container?.image !== "string" ||
    !container.image.includes("/livekit-server:") ||
    !container.image.endsWith(`@${targetBundle.serverImage.digest}`)
  ) {
    throw new Error("restored LiveKit deployment has not converged on the target image");
  }
  if (!Array.isArray(pods.items) || pods.items.length !== desiredReplicas) {
    throw new Error("restored LiveKit pod count does not match the deployment");
  }
  const imageIds = new Set();
  for (const pod of pods.items) {
    const ready = pod.status?.conditions?.some(
      (condition) => condition.type === "Ready" && condition.status === "True",
    );
    const status = pod.status?.containerStatuses?.find(
      (candidate) => candidate.name === LIVEKIT_OPS.release,
    );
    if (
      pod.status?.phase !== "Running" ||
      ready !== true ||
      status?.ready !== true ||
      typeof status?.imageID !== "string" ||
      !status.imageID.endsWith(targetBundle.serverImage.digest)
    ) {
      throw new Error("a restored LiveKit pod is not ready on the target image digest");
    }
    imageIds.add(status.imageID);
  }
  if (imageIds.size !== 1) {
    throw new Error("restored LiveKit pods contain mixed image digests");
  }
};

const requireExactKeys = (value, expectedKeys, label) => {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error(`${label} must be a JSON object`);
  }
  const actual = Object.keys(value).toSorted();
  const expected = [...expectedKeys].toSorted();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    throw new Error(`${label} does not have the exact admitted fields`);
  }
};

const accessLatestSecret = (secretId) =>
  captureCommand(
    "gcloud",
    [
      "secrets",
      "versions",
      "access",
      "latest",
      "--secret",
      secretId,
      "--project",
      LIVEKIT_OPS.project,
    ],
    `read ${secretId} for in-memory preflight`,
  );

const requireEnabledLatestSecretVersions = (secretIds) => {
  for (const secretId of secretIds) {
    const state = captureCommand(
      "gcloud",
      [
        "secrets",
        "versions",
        "describe",
        "latest",
        "--secret",
        secretId,
        "--project",
        LIVEKIT_OPS.project,
        "--format=value(state)",
      ],
      `inspect ${secretId} latest version metadata`,
    ).trim();
    if (state !== "ENABLED") {
      throw new Error(`${secretId} latest version is not enabled`);
    }
  }
};

const resolveARecords = (hostname) => {
  const addresses = captureCommand("dig", ["+short", "A", hostname], `resolve ${hostname}`)
    .split(/\s+/u)
    .filter(Boolean);
  if (
    addresses.length === 0 ||
    addresses.some((address) => !/^(?:\d{1,3}\.){3}\d{1,3}$/u.test(address))
  ) {
    throw new Error(`${hostname} has no closed IPv4 answer set`);
  }
  return [...new Set(addresses)].toSorted();
};

const reservedAddress = (name, region) =>
  captureCommand(
    "gcloud",
    [
      "compute",
      "addresses",
      "describe",
      name,
      "--project",
      LIVEKIT_OPS.project,
      ...(region === "global" ? ["--global"] : ["--region", region]),
      "--format=value(address)",
    ],
    `inspect reserved address ${name}`,
  ).trim();

const requireHostnameAtAddress = (hostname, address) => {
  const answers = resolveARecords(hostname);
  if (answers.length !== 1 || answers[0] !== address) {
    throw new Error(`${hostname} does not resolve only to its exact reserved address`);
  }
};

const requireHostnameAbsent = (hostname) => {
  const answers = captureCommand("dig", ["+short", hostname], `verify ${hostname} is absent`)
    .split(/\s+/u)
    .filter(Boolean);
  if (answers.length !== 0) {
    throw new Error(`${hostname} must be removed before canary destruction`);
  }
  const nameServers = captureCommand(
    "dig",
    ["+short", "NS", "openagents.com"],
    "discover authoritative openagents.com name servers",
  )
    .split(/\s+/u)
    .filter(Boolean);
  if (nameServers.length === 0) {
    throw new Error("openagents.com has no observable authoritative name servers");
  }
  for (const nameServer of nameServers) {
    for (const recordType of ["A", "AAAA", "CNAME", "HTTPS", "SVCB"]) {
      const authoritativeAnswers = captureCommand(
        "dig",
        ["+short", `@${nameServer}`, hostname, recordType],
        `verify authoritative ${recordType} absence for ${hostname}`,
      )
        .split(/\s+/u)
        .filter(Boolean);
      if (authoritativeAnswers.length !== 0) {
        throw new Error(
          `${hostname} ${recordType} must be removed from every authoritative name server`,
        );
      }
    }
  }
};

const requireEmptyCommandOutput = (bin, args, label) => {
  if (captureCommand(bin, args, label).trim() !== "") {
    throw new Error(`${label} found unexpected residue`);
  }
};

const runOpenSsl = (args, label) => captureCommand("openssl", args, label);

const validateCanaryCertificate = (temporaryDirectory) => {
  const certificatePath = resolve(temporaryDirectory, "canary-fullchain.pem");
  const privateKeyPath = resolve(temporaryDirectory, "canary-private-key.pem");
  writeFileSync(certificatePath, accessLatestSecret("oa-livekit-canary-tls-certificate"), {
    encoding: "utf8",
    mode: 0o600,
  });
  writeFileSync(privateKeyPath, accessLatestSecret("oa-livekit-canary-tls-private-key"), {
    encoding: "utf8",
    mode: 0o600,
  });
  runOpenSsl(
    ["x509", "-in", certificatePath, "-noout", "-checkend", "21600"],
    "validate canary certificate lifetime",
  );
  for (const hostname of [CANARY_SIGNAL_HOSTNAME, CANARY_TURN_HOSTNAME]) {
    runOpenSsl(
      [
        "verify",
        "-purpose",
        "sslserver",
        "-verify_hostname",
        hostname,
        "-untrusted",
        certificatePath,
        certificatePath,
      ],
      `validate canary certificate trust and SAN for ${hostname}`,
    );
  }
  runOpenSsl(["pkey", "-in", privateKeyPath, "-check", "-noout"], "validate canary private key");
  const certificatePublicKey = runOpenSsl(
    ["x509", "-in", certificatePath, "-pubkey", "-noout"],
    "read canary certificate public key",
  );
  const privatePublicKey = runOpenSsl(
    ["pkey", "-in", privateKeyPath, "-pubout"],
    "derive canary private-key public key",
  );
  if (sha256(certificatePublicKey) !== sha256(privatePublicKey)) {
    throw new Error("canary certificate and private key do not match");
  }
};

const requireNoCanaryInstance = () => {
  const names = captureCommand(
    "gcloud",
    [
      "compute",
      "instances",
      "list",
      "--project",
      LIVEKIT_OPS.project,
      "--zones",
      CANARY_ZONE,
      "--filter=name=oa-livekit-canary",
      "--format=value(name)",
    ],
    "verify the disposable canary VM slot is empty",
  ).trim();
  if (names !== "") throw new Error("the canary VM already exists before the gated apply");
};

const validateCanaryDestroyPreflight = () => {
  requireHostnameAbsent(CANARY_SIGNAL_HOSTNAME);
  requireHostnameAbsent(CANARY_TURN_HOSTNAME);
};

const verifyCanaryDestroyed = () => {
  requireHostnameAbsent(CANARY_SIGNAL_HOSTNAME);
  requireHostnameAbsent(CANARY_TURN_HOSTNAME);
  requireEmptyCommandOutput(
    "tofu",
    [`-chdir=${CANARY_TERRAFORM_ROOT}`, "state", "list"],
    "verify canary OpenTofu state is empty",
  );
  const exactAbsenceChecks = [
    [
      "gcloud",
      [
        "compute",
        "instances",
        "list",
        "--project",
        LIVEKIT_OPS.project,
        "--filter=name=oa-livekit-canary",
        "--format=value(name)",
      ],
      "verify canary instance absence",
    ],
    [
      "gcloud",
      [
        "compute",
        "disks",
        "list",
        "--project",
        LIVEKIT_OPS.project,
        "--filter=name=oa-livekit-canary",
        "--format=value(name)",
      ],
      "verify canary boot disk absence",
    ],
    [
      "gcloud",
      [
        "compute",
        "addresses",
        "list",
        "--project",
        LIVEKIT_OPS.project,
        "--regions",
        LIVEKIT_OPS.region,
        "--filter=name=oa-livekit-canary",
        "--format=value(name)",
      ],
      "verify canary address absence",
    ],
    [
      "gcloud",
      [
        "compute",
        "firewall-rules",
        "list",
        "--project",
        LIVEKIT_OPS.project,
        "--filter=name=(oa-livekit-canary-media oa-livekit-canary-iap-ssh)",
        "--format=value(name)",
      ],
      "verify canary firewall absence",
    ],
    [
      "gcloud",
      [
        "compute",
        "networks",
        "list",
        "--project",
        LIVEKIT_OPS.project,
        "--filter=name=oa-livekit-staging",
        "--format=value(name)",
      ],
      "verify canary network absence",
    ],
    [
      "gcloud",
      [
        "compute",
        "networks",
        "subnets",
        "list",
        "--project",
        LIVEKIT_OPS.project,
        "--regions",
        LIVEKIT_OPS.region,
        "--filter=name=oa-livekit-staging-nodes",
        "--format=value(name)",
      ],
      "verify canary subnet absence",
    ],
    [
      "gcloud",
      [
        "iam",
        "service-accounts",
        "list",
        "--project",
        LIVEKIT_OPS.project,
        "--filter=email=oa-livekit-canary-runtime@openagentsgemini.iam.gserviceaccount.com",
        "--format=value(email)",
      ],
      "verify canary service account absence",
    ],
  ];
  for (const [bin, args, label] of exactAbsenceChecks) {
    requireEmptyCommandOutput(bin, args, label);
  }
  const canarySecretIds = new Set(CANARY_SECRET_IDS);
  const remainingCanarySecrets = captureCommand(
    "gcloud",
    [
      "secrets",
      "list",
      "--project",
      LIVEKIT_OPS.project,
      "--format=value(name)",
    ],
    "inspect canary secret container absence",
  )
    .split(/\s+/u)
    .filter((name) => canarySecretIds.has(name.split("/").at(-1)));
  if (remainingCanarySecrets.length !== 0) {
    throw new Error("verify canary secret container absence found unexpected residue");
  }
};

const validateCanaryPreflight = (temporaryDirectory) => {
  requireEnabledLatestSecretVersions(CANARY_SECRET_IDS);
  requireNoCanaryInstance();
  const address = reservedAddress("oa-livekit-canary", LIVEKIT_OPS.region);
  requireHostnameAtAddress(CANARY_SIGNAL_HOSTNAME, address);
  requireHostnameAtAddress(CANARY_TURN_HOSTNAME, address);
  validateCanaryCertificate(temporaryDirectory);
};

const parseStructuredSecret = (secretId, expectedKeys) => {
  let value;
  try {
    value = JSON.parse(accessLatestSecret(secretId));
  } catch {
    throw new Error(`${secretId} latest version is not structured JSON`);
  }
  requireExactKeys(value, expectedKeys, secretId);
  for (const key of expectedKeys) {
    if (typeof value[key] !== "string" || value[key].trim() === "") {
      throw new Error(`${secretId}.${key} must be a non-empty string`);
    }
  }
  return value;
};

const validateProductionPreflight = () => {
  requireEnabledLatestSecretVersions(PRODUCTION_SECRET_IDS);
  const serverKeys = parseStructuredSecret("oa-livekit-prod-server-keys", [
    "api_key",
    "api_secret",
    "keys_yaml",
  ]);
  validateServerKeyProjection(serverKeys);
  const redisSecret = parseStructuredSecret("oa-livekit-prod-redis-auth", ["host", "ca_cert"]);
  let redis;
  try {
    redis = JSON.parse(
      captureCommand(
        "gcloud",
        [
          "redis",
          "instances",
          "describe",
          "oa-livekit-redis",
          "--project",
          LIVEKIT_OPS.project,
          "--region",
          LIVEKIT_OPS.region,
          "--format=json(name,region,tier,state,transitEncryptionMode,authEnabled,host,serverCaCerts)",
        ],
        "inspect the exact production Redis instance",
      ),
    );
  } catch {
    throw new Error("production Redis metadata is not valid JSON");
  }
  const redisCertificate = redis.serverCaCerts?.[0]?.cert;
  if (
    !(
      redis.name === "oa-livekit-redis" ||
      String(redis.name).endsWith("/instances/oa-livekit-redis")
    ) ||
    redis.region !== LIVEKIT_OPS.region ||
    redis.tier !== "STANDARD_HA" ||
    redis.state !== "READY" ||
    redis.transitEncryptionMode !== "SERVER_AUTHENTICATION" ||
    redis.authEnabled !== false ||
    redisSecret.host !== redis.host ||
    redisSecret.ca_cert.trim() !== String(redisCertificate).trim()
  ) {
    throw new Error(
      "production Redis or its exact host/CA projection is outside the admitted TLS no-AUTH shape",
    );
  }
  parseStructuredSecret("oa-livekit-prod-cloudflare-dns", ["api_token"]);
  const sourceOpenAiKey = accessLatestSecret(SARAH_OPENAI_SOURCE_SECRET).trim();
  const liveKitOpenAiKey = accessLatestSecret("oa-livekit-prod-openai-api-key").trim();
  if (sourceOpenAiKey === "" || sourceOpenAiKey !== liveKitOpenAiKey) {
    throw new Error("the LiveKit Sarah OpenAI key is not the current copied Sarah key");
  }
  const signalAddress = reservedAddress("oa-livekit-prod-signal", "global");
  const turnAddress = reservedAddress("oa-livekit-prod-turn", LIVEKIT_OPS.region);
  requireHostnameAtAddress(PRODUCTION_SIGNAL_HOSTNAME, signalAddress);
  requireHostnameAtAddress(PRODUCTION_TURN_HOSTNAME, turnAddress);
};

const validateHelm3 = () => {
  const version = captureCommand(
    "helm",
    ["version", "--template", "{{.Version}}"],
    "read Helm client version",
  ).trim();
  if (!/^v3\.\d+\.\d+(?:[-+].*)?$/u.test(version)) {
    throw new Error("production runtime apply requires Helm 3; Helm 4 is not admitted");
  }
};

const addonDownloadCommands = (addonLock, temporaryDirectory) =>
  [addonLock.certManager, addonLock.externalSecrets].map((addon) =>
    command("helm", [
      "pull",
      addon.chart,
      "--repo",
      addon.repository,
      "--version",
      addon.version,
      "--destination",
      temporaryDirectory,
    ]),
  );

const validateDownloadedAddonCharts = (addonLock, temporaryDirectory) => {
  for (const addon of [addonLock.certManager, addonLock.externalSecrets]) {
    const archive = resolve(temporaryDirectory, `${addon.chart}-${addon.version}.tgz`);
    if (!existsSync(archive)) throw new Error(`${addon.chart} archive was not downloaded`);
    if (sha256(readFileSync(archive)) !== `sha256:${addon.chartSha256}`) {
      throw new Error(`${addon.chart} archive digest does not match addons.lock.json`);
    }
  }
};

const addonInstallCommands = (addonLock, temporaryDirectory) => {
  const certManagerArchive = resolve(
    temporaryDirectory,
    `cert-manager-${addonLock.certManager.version}.tgz`,
  );
  const externalSecretsArchive = resolve(
    temporaryDirectory,
    `external-secrets-${addonLock.externalSecrets.version}.tgz`,
  );
  const appPoolSelector = "global.nodeSelector.cloud\\.google\\.com/gke-nodepool";
  const externalSecretsSelector = "nodeSelector.cloud\\.google\\.com/gke-nodepool";
  return [
    command("helm", [
      "upgrade",
      "--install",
      "cert-manager",
      certManagerArchive,
      "--namespace",
      "cert-manager",
      "--create-namespace",
      "--wait",
      "--timeout",
      "20m",
      "--set",
      "crds.enabled=true",
      "--set-string",
      `${appPoolSelector}=oa-livekit-prod-app`,
    ]),
    command("kubectl", [
      "wait",
      "--for=condition=Established",
      "--timeout=5m",
      "crd/certificates.cert-manager.io",
      "crd/clusterissuers.cert-manager.io",
    ]),
    command("kubectl", [
      "--namespace",
      "cert-manager",
      "rollout",
      "status",
      "deployment/cert-manager",
      "deployment/cert-manager-cainjector",
      "deployment/cert-manager-webhook",
      "--timeout=10m",
    ]),
    command("helm", [
      "upgrade",
      "--install",
      "external-secrets",
      externalSecretsArchive,
      "--namespace",
      "external-secrets",
      "--create-namespace",
      "--wait",
      "--timeout",
      "20m",
      "--set",
      "installCRDs=true",
      "--set-string",
      `${externalSecretsSelector}=oa-livekit-prod-app`,
      "--set-string",
      `webhook.${externalSecretsSelector}=oa-livekit-prod-app`,
      "--set-string",
      `certController.${externalSecretsSelector}=oa-livekit-prod-app`,
    ]),
    command("kubectl", [
      "wait",
      "--for=condition=Established",
      "--timeout=5m",
      "crd/externalsecrets.external-secrets.io",
      "crd/secretstores.external-secrets.io",
    ]),
    command("kubectl", [
      "--namespace",
      "external-secrets",
      "rollout",
      "status",
      "deployment/external-secrets",
      "deployment/external-secrets-cert-controller",
      "deployment/external-secrets-webhook",
      "--timeout=10m",
    ]),
  ];
};

const addonResourceRefs = (addonLock) => [
  `helm-chart-ref://cert-manager/${addonLock.certManager.version}/sha256/${addonLock.certManager.chartSha256}`,
  `helm-chart-ref://external-secrets/${addonLock.externalSecrets.version}/sha256/${addonLock.externalSecrets.chartSha256}`,
];

const printPlan = (operation, bundle, commands, extra = {}) => {
  const plan = {
    mode: "dry-run",
    operation,
    project: LIVEKIT_OPS.project,
    region: LIVEKIT_OPS.region,
    bundleDigest: sha256(JSON.stringify(bundle)),
    liveStateRead: false,
    mutationExecuted: false,
    commands: commands.map((step) => ({
      bin: step.bin,
      args: step.args.map((argument) =>
        /receipt|manifest|kubeconfig|output/iu.test(argument)
          ? "<scoped-path>"
          : argument.startsWith("https://")
            ? "<pinned-repository>"
            : argument,
      ),
      label: step.label,
    })),
    ...extra,
  };
  assertPublicSafe(plan);
  process.stdout.write(`${JSON.stringify(plan, null, 2)}\n`);
};

const git = (...args) => {
  const result = spawnSync("git", args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) throw new Error(`git provenance check failed: ${args[0]}`);
  return result.stdout.trim();
};

const validateGitProvenance = () => {
  const deployedRevision = git("rev-parse", "HEAD");
  if (!/^[0-9a-f]{40}$/u.test(deployedRevision)) {
    throw new Error("deployed source revision is invalid");
  }
  if (git("status", "--porcelain") !== "") {
    throw new Error("live operation requires a clean working tree");
  }
  if (git("branch", "--show-current") !== "main") {
    throw new Error("live operation must run from main");
  }
  const remoteMain = git("ls-remote", "--exit-code", "origin", "refs/heads/main")
    .split(/\s+/u)
    .at(0);
  if (remoteMain !== deployedRevision) {
    throw new Error("live operation source commit is not the current remote main");
  }
  return deployedRevision;
};

const writeReceipt = (
  path,
  bundle,
  { stage, phase, outcome, resourceRefs, result, deployedRevision },
) => {
  const resultDigest = sha256(JSON.stringify(result));
  const now = new Date().toISOString();
  const receipt = {
    schemaVersion: "openagents.livekit_ops_receipt.v1",
    receiptRef: `livekit-ops-receipt-ref://sha256/${resultDigest.slice("sha256:".length)}`,
    issueRef: "github-issue-ref://OpenAgentsInc/openagents/9284",
    stage,
    phase,
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision,
    bundleDigest: sha256(JSON.stringify(bundle)),
    configurationDigest: bundle.configurationDigest,
    resourceRefs,
    startedAt: now,
    settledAt: now,
    outcome,
    evidenceTier: "live_observed",
    liveProof: true,
    resultDigest,
    limitations: [
      "This operation receipt proves only the scoped deployment lifecycle step.",
      "Connectivity, load, drills, scans, cost, and rollback acceptance require separate receipts.",
    ],
  };
  validateSourceOnlyReceipt(receipt);
  const receiptPath = resolve(path);
  const receiptsRoot = `${resolve("docs/ops/receipts/livekit")}${sep}`;
  if (!receiptPath.startsWith(receiptsRoot)) {
    throw new Error("receipt must stay under docs/ops/receipts/livekit");
  }
  mkdirSync(dirname(receiptPath), { recursive: true });
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  return receipt;
};

const verifyCanary = () => {
  const result = spawnSync(
    "gcloud",
    [
      "compute",
      "instances",
      "describe",
      "oa-livekit-canary",
      "--project",
      LIVEKIT_OPS.project,
      "--zone",
      CANARY_ZONE,
      "--format=json(name,zone,status,deletionProtection,labels)",
    ],
    { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  );
  if (result.status !== 0) throw new Error("the exact canary instance is not observable");
  const instance = JSON.parse(result.stdout);
  const expiry = Number(instance.labels?.["openagents-expires-at"]);
  if (
    instance.name !== "oa-livekit-canary" ||
    !String(instance.zone).endsWith(`/zones/${CANARY_ZONE}`) ||
    instance.deletionProtection !== false ||
    instance.labels?.["openagents-component"] !== "livekit-canary" ||
    !Number.isSafeInteger(expiry) ||
    expiry <= Math.floor(Date.now() / 1_000) ||
    expiry > Math.floor(Date.now() / 1_000) + LIVEKIT_OPS.canaryMaximumLifetimeSeconds
  ) {
    throw new Error("canary identity, deletion protection, or six-hour expiry is invalid");
  }
};

const validateProductionRuntimeVariables = () => {
  const authorizedNetworks = process.env.TF_VAR_master_authorized_networks;
  const notificationChannels = process.env.TF_VAR_notification_channel_ids;
  const billingAccount = process.env.TF_VAR_billing_account_id;
  if (!authorizedNetworks || !notificationChannels || !billingAccount) {
    throw new Error(
      "production OpenTofu requires untracked TF_VAR_master_authorized_networks, TF_VAR_notification_channel_ids, and TF_VAR_billing_account_id",
    );
  }
  let networks;
  let channels;
  try {
    networks = JSON.parse(authorizedNetworks);
    channels = JSON.parse(notificationChannels);
  } catch {
    throw new Error("production OpenTofu list variables must be valid JSON");
  }
  if (
    !Array.isArray(networks) ||
    networks.length === 0 ||
    networks.some(
      (network) =>
        typeof network !== "object" ||
        network === null ||
        typeof network.cidr_block !== "string" ||
        !/^(?:\d{1,3}\.){3}\d{1,3}\/(?:[0-9]|[12][0-9]|3[0-2])$/u.test(network.cidr_block) ||
        network.cidr_block === "0.0.0.0/0" ||
        typeof network.display_name !== "string" ||
        network.display_name.length === 0,
    )
  ) {
    throw new Error("master authorized networks must be bounded named IPv4 CIDRs");
  }
  if (
    !Array.isArray(channels) ||
    channels.length !== 1 ||
    channels[0] !== "projects/openagentsgemini/notificationChannels/1554456325732494481"
  ) {
    throw new Error("notification channels must contain the exact redacted LiveKit on-call ref");
  }
  if (!/^\d{6}-\d{6}-\d{6}$/u.test(billingAccount)) {
    throw new Error("billing account variable has an invalid identifier shape");
  }
};

const run = () => {
  const args = parseArgs(process.argv.slice(2));
  const bundlePath = requireRepositoryPath(args.bundle, "infra/livekit/bundle.json");
  const bundle = validateDeploymentBundle(readJson(bundlePath, "deployment bundle"));
  const addonLockPath = requireRepositoryPath(ADDON_LOCK, ADDON_LOCK);
  const addonLock = validateAddonLock(readJson(addonLockPath, "addon lock"));
  validateManifestDigests(bundle);
  const temporaryDirectory = mkdtempSync(resolve(tmpdir(), "oa-livekit-ops-"));
  const kubeconfig = resolve(temporaryDirectory, "kubeconfig");
  let commands = [];
  let receipt;
  let rollbackAdmissionReceipt;
  let rollbackPreviousManifest;
  let rollbackPruneManifest;
  let rollbackPrunedInventory;
  let rollbackTargetBundle;
  let rollbackTargetInventory;
  try {
    if (args.operation === "validate-source") {
      if (args.apply) {
        if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
          throw new Error(`--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`);
        }
        renderProduction(bundle, temporaryDirectory, true);
      }
      process.stdout.write(
        `${JSON.stringify({
          operation: args.operation,
          outcome: "passed",
          bundleDigest: sha256(JSON.stringify(bundle)),
          renderedManifestDigest: bundle.renderedManifestDigest,
          liveStateRead: false,
          mutationExecuted: false,
          upstreamArchiveRead: args.apply,
        })}\n`,
      );
      return;
    }

    if (args.operation === "canary-plan") {
      commands = terraformCommands(CANARY_TERRAFORM_ROOT, "plan", [
        "-var=enable_canary_instance=false",
      ]);
    } else if (args.operation === "canary-infra-apply") {
      commands = terraformCommands(CANARY_TERRAFORM_ROOT, "apply", [
        "-var=enable_canary_instance=false",
      ]);
    } else if (args.operation === "canary-apply") {
      const expiry = Math.floor(Date.now() / 1_000) + LIVEKIT_OPS.canaryMaximumLifetimeSeconds;
      commands = terraformCommands(CANARY_TERRAFORM_ROOT, "apply", [
        "-var=enable_canary_instance=true",
        `-var=canary_expires_at_unix=${expiry}`,
      ]);
    } else if (args.operation === "canary-destroy") {
      commands = terraformCommands(CANARY_TERRAFORM_ROOT, "destroy");
    } else if (args.operation === "production-plan") {
      commands = [
        ...terraformCommands(PRODUCTION_TERRAFORM_ROOT, "plan"),
        ...renderProduction(bundle, temporaryDirectory, false),
      ];
    } else if (args.operation === "production-infra-apply") {
      commands = [
        ...terraformCommands(PRODUCTION_TERRAFORM_ROOT, "apply"),
        ...productionInfrastructureValidationCommands(),
      ];
    } else if (args.operation === "production-runtime-apply") {
      commands = [
        ...productionInfrastructureValidationCommands(),
        productionCredentialsCommand(kubeconfig),
        ...renderProduction(bundle, temporaryDirectory, false),
        ...addonDownloadCommands(addonLock, temporaryDirectory),
        ...addonInstallCommands(addonLock, temporaryDirectory),
        command("kubectl", [
          "--namespace",
          LIVEKIT_OPS.namespace,
          "apply",
          "--server-side",
          "--field-manager=openagents-livekit-ops",
          "-f",
          resolve(temporaryDirectory, RENDERED_MANIFEST),
        ]),
        command("kubectl", [
          "--namespace",
          LIVEKIT_OPS.namespace,
          "rollout",
          "status",
          `deployment/${LIVEKIT_OPS.release}`,
          "--timeout=20m",
        ]),
      ];
    } else if (args.operation === "production-rollback") {
      rollbackTargetBundle = validateHistoricalDeploymentBundle(
        readJson(args.previousBundle, "previous deployment bundle"),
      );
      rollbackAdmissionReceipt = validateAdmissionReceipt(
        readJson(args.admissionReceipt, "admission-disable receipt"),
        bundle,
      );
      validatePreviousDeploymentReceipt(
        readJson(args.previousDeploymentReceipt, "previous deployment receipt"),
        rollbackTargetBundle,
        addonResourceRefs(addonLock),
      );
      if (
        rollbackTargetBundle.renderedManifestDigest === bundle.renderedManifestDigest &&
        rollbackTargetBundle.configurationDigest === bundle.configurationDigest &&
        rollbackTargetBundle.serverImage.digest === bundle.serverImage.digest
      ) {
        throw new Error("rollback previous bundle does not change the pinned deployment");
      }
      const currentManifest = resolve(args.currentManifest);
      rollbackPreviousManifest = resolve(args.previousManifest);
      validateRenderedManifest(currentManifest, bundle.renderedManifestDigest);
      validateRenderedManifest(
        rollbackPreviousManifest,
        rollbackTargetBundle.renderedManifestDigest,
      );
      const currentInventory = manifestInventory(currentManifest, "current runtime manifest");
      rollbackTargetInventory = manifestInventory(
        rollbackPreviousManifest,
        "rollback target manifest",
      );
      const targetKeys = new Set(rollbackTargetInventory.map(inventoryKey));
      rollbackPrunedInventory = currentInventory.filter(
        (resource) => !targetKeys.has(inventoryKey(resource)),
      );
      rollbackPruneManifest = resolve(temporaryDirectory, "rollback-prune-list.json");
      writePruneInventory(rollbackPruneManifest, rollbackPrunedInventory);
      commands = [
        ...productionInfrastructureValidationCommands(),
        productionCredentialsCommand(kubeconfig),
        command("kubectl", [
          "--namespace",
          LIVEKIT_OPS.namespace,
          "apply",
          "--server-side",
          "--field-manager=openagents-livekit-ops",
          "-f",
          rollbackPreviousManifest,
        ]),
        ...(rollbackPrunedInventory.length === 0
          ? []
          : [
              command("kubectl", [
                "delete",
                "-f",
                rollbackPruneManifest,
                "--ignore-not-found=true",
                "--wait=true",
                "--timeout=10m",
              ]),
            ]),
        command("kubectl", [
          "--namespace",
          LIVEKIT_OPS.namespace,
          "rollout",
          "status",
          `deployment/${LIVEKIT_OPS.release}`,
          "--timeout=20m",
        ]),
      ];
    }

    if (!args.apply) {
      printPlan(args.operation, bundle, commands, {
        requiresOwnerGate: args.operation !== "validate-source",
        exactResourceScope: args.operation.startsWith("canary")
          ? "gcp-resource-ref://livekit/canary"
          : "gcp-resource-ref://livekit/production",
        ...(rollbackTargetBundle === undefined
          ? {}
          : { rollbackTargetBundleDigest: sha256(JSON.stringify(rollbackTargetBundle)) }),
      });
      return;
    }
    if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
      throw new Error(`--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`);
    }
    const deployedRevision = validateGitProvenance();
    if (
      rollbackAdmissionReceipt !== undefined &&
      rollbackAdmissionReceipt.deployedRevision !== deployedRevision
    ) {
      throw new Error("admission receipt does not bind the deployed origin/main revision");
    }
    if (args.operation === "production-plan" || args.operation === "production-infra-apply") {
      validateProductionRuntimeVariables();
    }

    if (args.operation === "canary-apply") {
      validateCanaryPreflight(temporaryDirectory);
    }
    if (args.operation === "canary-destroy") {
      validateCanaryDestroyPreflight();
    }
    if (args.operation === "production-runtime-apply") {
      validateProductionPreflight();
      validateHelm3();
      const firstPullIndex = commands.findIndex(
        (step) => step.bin === "helm" && step.args[0] === "pull",
      );
      if (firstPullIndex < 0) throw new Error("addon download stage is missing");
      executeCommands(commands.slice(0, firstPullIndex), kubectlEnv(kubeconfig));
      validateRenderedManifest(
        resolve(temporaryDirectory, RENDERED_MANIFEST),
        bundle.renderedManifestDigest,
      );
      executeCommands(commands.slice(firstPullIndex, firstPullIndex + 2), kubectlEnv(kubeconfig));
      validateDownloadedAddonCharts(addonLock, temporaryDirectory);
      executeCommands(commands.slice(firstPullIndex + 2), kubectlEnv(kubeconfig));
    } else {
      executeCommands(commands, kubectlEnv(kubeconfig));
    }

    if (args.operation === "canary-apply") verifyCanary();
    if (args.operation === "canary-destroy") verifyCanaryDestroyed();
    if (
      args.operation === "production-rollback" &&
      rollbackPreviousManifest !== undefined &&
      rollbackPruneManifest !== undefined &&
      rollbackPrunedInventory !== undefined &&
      rollbackTargetBundle !== undefined &&
      rollbackTargetInventory !== undefined
    ) {
      verifyRollbackRuntime({
        kubeconfig,
        previousManifest: rollbackPreviousManifest,
        pruneManifest: rollbackPruneManifest,
        prunedInventory: rollbackPrunedInventory,
        targetBundle: rollbackTargetBundle,
        targetInventory: rollbackTargetInventory,
      });
    }

    const canary = args.operation.startsWith("canary");
    const destroy = args.operation === "canary-destroy";
    const rollback = args.operation === "production-rollback";
    const runtime = args.operation === "production-runtime-apply";
    const resourceRefs = [
      canary ? "gcp-resource-ref://livekit/canary" : "gcp-resource-ref://livekit/production",
    ];
    if (args.operation === "canary-apply") {
      resourceRefs.push("gcp-preflight-ref://livekit/canary/secrets-dns-certificate-empty-vm");
    }
    if (runtime) {
      resourceRefs.push(
        "gcp-preflight-ref://livekit/production/secrets-dns",
        ...addonResourceRefs(addonLock),
      );
    }
    const receiptBundle = rollback ? rollbackTargetBundle : bundle;
    if (receiptBundle === undefined) {
      throw new Error("rollback target bundle is unavailable at receipt boundary");
    }
    receipt = writeReceipt(args.receipt, receiptBundle, {
      stage: canary ? "canary" : "production",
      phase: destroy ? "destroy" : rollback ? "rollback" : "deployment",
      outcome: destroy ? "destroyed" : rollback ? "rolled_back" : "passed",
      resourceRefs,
      result: {
        operation: args.operation,
        bundleDigest: sha256(JSON.stringify(receiptBundle)),
        commandPlanDigest: sha256(JSON.stringify(commands)),
        exactProject: true,
        exactRegion: true,
        ...(rollbackTargetInventory === undefined
          ? {}
          : { targetInventoryDigest: sha256(JSON.stringify(rollbackTargetInventory)) }),
        ...(rollbackPrunedInventory === undefined
          ? {}
          : { prunedInventoryDigest: sha256(JSON.stringify(rollbackPrunedInventory)) }),
      },
      deployedRevision,
    });
    process.stdout.write(
      `${JSON.stringify({
        operation: args.operation,
        outcome: receipt.outcome,
        receiptRef: receipt.receiptRef,
        resultDigest: receipt.resultDigest,
      })}\n`,
    );
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
};

try {
  run();
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  usage();
  process.exitCode = 1;
}
