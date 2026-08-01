#!/usr/bin/env node

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { assertPublicSafe, validateDeploymentBundle } from "./livekit-ops-policy.mjs";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";
const PROJECT = "openagentsgemini";
const REGION = "us-central1";
const NAMESPACE = "livekit-system";
const COMMIT = /^[0-9a-f]{40}$/u;
const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const COUNTS_QUERY = `
SELECT
  current_database() AS database_name,
  (SELECT COUNT(*)
     FROM sarah_livekit_room_bindings AS binding
     INNER JOIN sarah_realtime_voice_sessions AS session USING (session_ref)
    WHERE binding.state IN ('prepared', 'active')
      AND session.state IN ('reserved', 'connected')) AS active_room_count,
  (SELECT COUNT(*)
     FROM sarah_realtime_voice_sessions
    WHERE transport_kind = 'livekit_room_v1'
      AND (
        state IN ('reserved', 'connected')
        OR (state = 'accounting_uncertain' AND credit_mode <> 'owner_waived_unmetered')
      ))
    AS pending_settlement_count,
  (SELECT COUNT(*)
     FROM sarah_livekit_room_bindings AS binding
     INNER JOIN sarah_realtime_voice_sessions AS session USING (session_ref)
    WHERE session.transport_kind <> 'livekit_room_v1') AS silent_transport_switch_count;
`;

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-rollback-capture.mjs \\
    --phase baseline \\
    --bundle <last-healthy-bundle.json> \\
    --deployed-revision <40-hex> \\
    --admission-receipt <private-admission-disable-receipt.json> \\
    --output <outside-repository/private-rollback-baseline.json> [--apply]

  node scripts/cloud/livekit-rollback-capture.mjs \\
    --phase postcheck \\
    --bundle <last-healthy-bundle.json> \\
    --deployed-revision <40-hex> \\
    --admission-receipt <private-admission-disable-receipt.json> \\
    --runtime-receipt <production-runtime-rollback-receipt.json> \\
    --before-snapshot <private-rollback-baseline.json> \\
    --output <outside-repository/private-controlled-rollback-capture.json> [--apply]

Default mode prints the closed read-only plan. --apply requires the LiveKit
owner gate and standard libpq variables; postcheck also requires a
preconfigured KUBECONFIG for the exact production cluster. The baseline must
be collected after admission is disabled and drained but before the runtime
rollback. The postcheck derives the controlled rollback capture from live
Cloud Run, Kubernetes, database, relay, update, and managed-sandbox
observations. It writes only opaque digests and bounded counts to an exclusive
mode-0600 file outside the repository. It never mutates production.
`);
};

const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const exactKeys = (value, required, optional, label) => {
  assert(value && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value))
    assert(allowed.has(key), `${label} has unsupported field ${key}`);
  for (const key of required) assert(Object.hasOwn(value, key), `${label} is missing ${key}`);
};

const readJson = (path, label) => {
  try {
    return JSON.parse(readFileSync(resolve(path), "utf8"));
  } catch (error) {
    throw new Error(`${label} is not readable JSON`, { cause: error });
  }
};

const canonical = (value) => {
  if (Array.isArray(value))
    return value
      .map(canonical)
      .sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, canonical(value[key])]),
    );
  return value;
};

const sha256 = (value) =>
  `sha256:${createHash("sha256")
    .update(typeof value === "string" ? value : JSON.stringify(value))
    .digest("hex")}`;

const observationDigest = (value) => sha256(canonical(value));

const timestamp = (value, label) => {
  assert(typeof value === "string" && Number.isFinite(Date.parse(value)), `${label} is invalid`);
  return value;
};

const parseArgs = (values) => {
  const parsed = { apply: false };
  for (let index = 0; index < values.length; index += 1) {
    const argument = values[index];
    if (argument === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    if (
      ![
        "--phase",
        "--bundle",
        "--deployed-revision",
        "--admission-receipt",
        "--runtime-receipt",
        "--before-snapshot",
        "--output",
      ].includes(argument)
    )
      throw new Error(`unsupported argument ${argument}`);
    const value = values[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    parsed[
      {
        "--phase": "phase",
        "--bundle": "bundle",
        "--deployed-revision": "deployedRevision",
        "--admission-receipt": "admissionReceipt",
        "--runtime-receipt": "runtimeReceipt",
        "--before-snapshot": "beforeSnapshot",
        "--output": "output",
      }[argument]
    ] = value;
    index += 1;
  }
  for (const field of ["phase", "bundle", "deployedRevision", "admissionReceipt", "output"])
    assert(parsed[field], `missing required argument ${field}`);
  assert(["baseline", "postcheck"].includes(parsed.phase), "phase must be baseline or postcheck");
  assert(COMMIT.test(parsed.deployedRevision), "deployed revision must be a full Git commit");
  if (parsed.phase === "postcheck") {
    assert(parsed.runtimeReceipt, "postcheck requires runtimeReceipt");
    assert(parsed.beforeSnapshot, "postcheck requires beforeSnapshot");
  } else {
    assert(
      !parsed.runtimeReceipt && !parsed.beforeSnapshot,
      "baseline does not accept postcheck inputs",
    );
  }
  return parsed;
};

const isWithin = (parent, candidate) => {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
};

const exactCommand = (bin, args, environment = process.env) =>
  spawnSync(bin, args, {
    encoding: "utf8",
    env: environment,
    maxBuffer: 4 * 1024 * 1024,
    shell: false,
    timeout: 60_000,
  });

const requireSuccess = (execution, label) => {
  if (execution.error) throw new Error(`${label} could not execute`, { cause: execution.error });
  if (execution.status !== 0)
    throw new Error(`${label} failed with exit status ${execution.status}`);
  return execution.stdout ?? "";
};

const commandJson = (runCommand, bin, args, label) => {
  try {
    return JSON.parse(requireSuccess(runCommand(bin, args), label));
  } catch (error) {
    throw new Error(`${label} returned invalid JSON`, { cause: error });
  }
};

const validateAdmissionReceipt = (receipt, bundle, deployedRevision) => {
  exactKeys(
    receipt,
    [
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
    ],
    [],
    "admission-disable receipt",
  );
  assert(
    receipt.schemaVersion === "openagents.livekit_admission_disable.v1" &&
      receipt.stage === "production" &&
      receipt.sourceBaseRevision === bundle.sourceBaseRevision &&
      receipt.deployedRevision === deployedRevision &&
      receipt.resourceRef === "livekit-admission-ref://production/livekit-room-v1" &&
      receipt.newAdmissionDisabled === true &&
      receipt.newDispatchDisabled === true &&
      receipt.activeRoomCount === 0 &&
      receipt.pendingSettlementCount === 0,
    "admission-disable receipt does not prove the exact drained production boundary",
  );
  timestamp(receipt.observedAt, "admission-disable receipt observedAt");
  return receipt;
};

const validateRuntimeReceipt = (receipt, bundle, deployedRevision) => {
  assert(
    receipt && typeof receipt === "object" && !Array.isArray(receipt),
    "runtime rollback receipt must be an object",
  );
  for (const field of [
    "schemaVersion",
    "stage",
    "phase",
    "sourceBaseRevision",
    "deployedRevision",
    "bundleDigest",
    "configurationDigest",
    "startedAt",
    "settledAt",
    "outcome",
    "evidenceTier",
    "liveProof",
    "resultDigest",
  ])
    assert(Object.hasOwn(receipt, field), `runtime rollback receipt is missing ${field}`);
  const bundleDigest = sha256(bundle);
  assert(
    receipt.schemaVersion === "openagents.livekit_ops_receipt.v1" &&
      receipt.stage === "production" &&
      receipt.phase === "rollback" &&
      receipt.sourceBaseRevision === bundle.sourceBaseRevision &&
      receipt.deployedRevision === deployedRevision &&
      receipt.bundleDigest === bundleDigest &&
      receipt.configurationDigest === bundle.configurationDigest &&
      receipt.outcome === "rolled_back" &&
      receipt.evidenceTier === "live_observed" &&
      receipt.liveProof === true &&
      DIGEST.test(receipt.resultDigest),
    "runtime rollback receipt does not prove the exact target bundle",
  );
  timestamp(receipt.startedAt, "runtime rollback receipt startedAt");
  timestamp(receipt.settledAt, "runtime rollback receipt settledAt");
  assert(
    Date.parse(receipt.startedAt) <= Date.parse(receipt.settledAt),
    "runtime rollback receipt time order is invalid",
  );
  return receipt;
};

const latestServingRevision = (service) => {
  const ready = (service.status?.conditions ?? []).some(
    (condition) => condition.type === "Ready" && condition.status === "True",
  );
  assert(ready, "production Cloud Run service is not Ready");
  const revisionName = service.status?.latestReadyRevisionName;
  assert(
    typeof revisionName === "string" && revisionName !== "",
    "production Cloud Run has no Ready revision",
  );
  const traffic = service.status?.traffic ?? [];
  // Cloud Run retains tagged revisions as zero-traffic entries without a
  // percentage, so they do not make the serving revision a split deployment.
  const servingTraffic = traffic.filter((entry) => (entry.percent ?? 0) > 0);
  assert(
    servingTraffic.length === 1 &&
      servingTraffic[0]?.revisionName === revisionName &&
      servingTraffic[0]?.percent === 100,
    "production Cloud Run traffic is not wholly on its latest Ready revision",
  );
  return revisionName;
};

const admissionStillDisabled = (runCommand) => {
  const service = commandJson(
    runCommand,
    "gcloud",
    [
      "run",
      "services",
      "describe",
      "openagents-monolith",
      "--project",
      PROJECT,
      "--region",
      REGION,
      "--format=json",
    ],
    "read production Cloud Run service",
  );
  const revisionName = latestServingRevision(service);
  const revision = commandJson(
    runCommand,
    "gcloud",
    [
      "run",
      "revisions",
      "describe",
      revisionName,
      "--project",
      PROJECT,
      "--region",
      REGION,
      "--format=json",
    ],
    "read production Cloud Run revision",
  );
  assert(revision.metadata?.name === revisionName, "Cloud Run revision observation drifted");
  assert(
    (revision.status?.conditions ?? []).some(
      (condition) => condition.type === "Ready" && condition.status === "True",
    ),
    "serving Cloud Run revision is not Ready",
  );
  const containers = revision.spec?.containers;
  assert(Array.isArray(containers) && containers.length === 1, "Cloud Run container shape drifted");
  const environment = new Map((containers[0].env ?? []).map((entry) => [entry.name, entry.value]));
  assert(
    environment.get("SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED") === "false",
    "production LiveKit admission is no longer disabled",
  );
  return true;
};

const databaseCounts = (runCommand) => {
  for (const name of ["PGHOST", "PGUSER", "PGPASSWORD", "PGDATABASE"])
    assert(process.env[name] !== undefined, `${name} is required for rollback aggregate reads`);
  const execution = runCommand(
    "psql",
    [
      "--no-psqlrc",
      "--no-align",
      "--tuples-only",
      "--field-separator=,",
      "--command",
      COUNTS_QUERY,
    ],
    process.env,
  );
  const match = /^\s*khala_sync_prod\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*$/u.exec(
    requireSuccess(execution, "read rollback aggregate counts"),
  );
  assert(match !== null, "rollback aggregate count query returned an invalid shape");
  const [activeRoomCount, pendingSettlementCount, silentTransportSwitchCount] = match
    .slice(1)
    .map(Number);
  assert(activeRoomCount === 0, "production still has active LiveKit rooms");
  assert(pendingSettlementCount === 0, "production still has pending LiveKit settlements");
  assert(silentTransportSwitchCount === 0, "a LiveKit binding changed transport silently");
  return { activeRoomCount, pendingSettlementCount, silentTransportSwitchCount };
};

const deploymentReady = (deployment, name) => {
  assert(deployment.metadata?.name === name, `${name} deployment is missing`);
  assert(
    deployment.status?.observedGeneration === deployment.metadata?.generation,
    `${name} deployment generation is not observed`,
  );
  const replicas = deployment.spec?.replicas;
  assert(Number.isSafeInteger(replicas) && replicas > 0, `${name} desired replicas are invalid`);
  assert(
    deployment.status?.readyReplicas === replicas &&
      deployment.status?.updatedReplicas === replicas &&
      deployment.status?.availableReplicas === replicas &&
      (deployment.status?.unavailableReplicas ?? 0) === 0,
    `${name} deployment is not fully converged`,
  );
};

const restoredPins = (runCommand, bundle) => {
  const kubeconfig = commandJson(
    runCommand,
    "kubectl",
    ["config", "view", "--minify", "-o", "json"],
    "read controlled Kubernetes context",
  );
  const expectedContext = "gke_openagentsgemini_us-central1_oa-livekit-prod";
  assert(
    kubeconfig["current-context"] === expectedContext &&
      Array.isArray(kubeconfig.contexts) &&
      kubeconfig.contexts.length === 1 &&
      kubeconfig.contexts[0]?.name === expectedContext &&
      kubeconfig.contexts[0]?.context?.cluster === expectedContext,
    "Kubernetes context is not the exact production LiveKit cluster",
  );
  const deployments = commandJson(
    runCommand,
    "kubectl",
    [
      "--namespace",
      NAMESPACE,
      "get",
      "deployment",
      "livekit-server",
      "sarah-livekit-agent",
      "-o",
      "json",
    ],
    "read restored LiveKit deployments",
  );
  assert(
    Array.isArray(deployments.items) && deployments.items.length === 2,
    "restored deployment inventory is incomplete",
  );
  const byName = new Map(
    deployments.items.map((deployment) => [deployment.metadata?.name, deployment]),
  );
  const server = byName.get("livekit-server");
  const worker = byName.get("sarah-livekit-agent");
  deploymentReady(server, "livekit-server");
  deploymentReady(worker, "sarah-livekit-agent");
  const imageFor = (deployment, containerName, label) => {
    const containers = deployment.spec?.template?.spec?.containers;
    assert(Array.isArray(containers), `${label} containers are invalid`);
    const matches = containers.filter((container) => container.name === containerName);
    assert(matches.length === 1, `${label} container inventory drifted`);
    const match = /@(sha256:[0-9a-f]{64})$/u.exec(matches[0].image ?? "");
    assert(match !== null, `${label} image is not digest-pinned`);
    return match[1];
  };
  const configuration = server.spec?.template?.metadata?.annotations?.["checksum/config"];
  assert(
    typeof configuration === "string" && /^[0-9a-f]{64}$/u.test(configuration),
    "restored LiveKit configuration checksum is invalid",
  );
  const result = {
    restoredBundleDigest: sha256(bundle),
    restoredConfigurationDigest: `sha256:${configuration}`,
    restoredServerImageDigest: imageFor(server, "livekit-server", "LiveKit server"),
    restoredWorkerImageDigest: imageFor(worker, "sarah-livekit-agent", "Sarah worker"),
  };
  assert(
    result.restoredConfigurationDigest === bundle.configurationDigest &&
      result.restoredServerImageDigest === bundle.serverImage.digest &&
      result.restoredWorkerImageDigest === bundle.workerImage.digest,
    "restored runtime pins do not match the target bundle",
  );
  return result;
};

const unrelatedServiceDigests = (runCommand) => {
  const cloudRun = (name, label, required = true) => {
    const services = commandJson(
      runCommand,
      "gcloud",
      [
        "run",
        "services",
        "list",
        "--project",
        PROJECT,
        "--region",
        REGION,
        `--filter=metadata.name=${name}`,
        "--format=json(metadata.name,metadata.generation,spec.template.metadata.name,status.latestReadyRevisionName,status.traffic)",
      ],
      `read ${label} service boundary`,
    );
    assert(
      Array.isArray(services) && services.length <= 1 && (!required || services.length === 1),
      `${label} service boundary is unavailable or ambiguous`,
    );
    return services;
  };
  const database = commandJson(
    runCommand,
    "gcloud",
    [
      "sql",
      "instances",
      "list",
      "--project",
      PROJECT,
      "--filter=name=khala-sync-pg",
      "--format=json(name,databaseVersion,settings.settingsVersion,settings.tier,settings.availabilityType,settings.dataDiskType,settings.dataDiskSizeGb,settings.backupConfiguration,settings.ipConfiguration.privateNetwork,settings.databaseFlags)",
    ],
    "read database service boundary",
  );
  assert(
    Array.isArray(database) && database.length === 1,
    "database service boundary is unavailable",
  );
  const sandboxBridge = cloudRun("oa-managed-sandbox-bridge", "managed sandbox bridge");
  const sandboxControl = commandJson(
    runCommand,
    "gcloud",
    [
      "compute",
      "instances",
      "list",
      "--project",
      PROJECT,
      "--filter=name~'^oa-managed-sandbox-control.*'",
      "--format=json(name,zone,machineType,status,fingerprint,metadata.fingerprint,disks.source,disks.sourceImage,networkInterfaces.network,serviceAccounts.email,tags.fingerprint)",
    ],
    "read managed sandbox control boundary",
  );
  assert(
    Array.isArray(sandboxControl) && sandboxControl.length > 0,
    "managed sandbox control boundary is unavailable",
  );
  return [
    observationDigest(cloudRun("openagents-monolith", "OpenAgents Cloud Run")),
    observationDigest(database),
    observationDigest(cloudRun("openagents-nostr-relay", "relay", false)),
    observationDigest(cloudRun("oa-updates", "update")),
    observationDigest({ bridge: sandboxBridge, control: sandboxControl }),
  ];
};

const validateSnapshot = (snapshot, bundle, deployedRevision, admissionReceipt) => {
  exactKeys(
    snapshot,
    [
      "schemaVersion",
      "sourceBaseRevision",
      "deployedRevision",
      "bundleDigest",
      "admissionReceiptDigest",
      "observedAt",
      "unrelatedServiceDigests",
    ],
    [],
    "rollback baseline",
  );
  assert(
    snapshot.schemaVersion === "openagents.livekit_rollback_baseline.v1" &&
      snapshot.sourceBaseRevision === bundle.sourceBaseRevision &&
      snapshot.deployedRevision === deployedRevision &&
      snapshot.bundleDigest === sha256(bundle) &&
      snapshot.admissionReceiptDigest === sha256(admissionReceipt),
    "rollback baseline binding drifted",
  );
  timestamp(snapshot.observedAt, "rollback baseline observedAt");
  assert(
    Array.isArray(snapshot.unrelatedServiceDigests) &&
      snapshot.unrelatedServiceDigests.length === 5 &&
      snapshot.unrelatedServiceDigests.every((value) => DIGEST.test(value)),
    "rollback baseline service digests are invalid",
  );
  return snapshot;
};

const writePrivate = (path, value) => {
  const repositoryRoot = resolve(import.meta.dirname, "../..");
  const output = resolve(path);
  assert(
    !isWithin(repositoryRoot, output),
    "private rollback evidence must stay outside the repository",
  );
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(output, `${JSON.stringify(value, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
};

export const collectRollbackBaseline = ({
  bundle,
  deployedRevision,
  admissionReceipt,
  runCommand = exactCommand,
  now = () => new Date().toISOString(),
}) => {
  validateAdmissionReceipt(admissionReceipt, bundle, deployedRevision);
  admissionStillDisabled(runCommand);
  databaseCounts(runCommand);
  const snapshot = {
    schemaVersion: "openagents.livekit_rollback_baseline.v1",
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision,
    bundleDigest: sha256(bundle),
    admissionReceiptDigest: sha256(admissionReceipt),
    observedAt: now(),
    unrelatedServiceDigests: unrelatedServiceDigests(runCommand),
  };
  assert(
    Date.parse(snapshot.observedAt) >= Date.parse(admissionReceipt.observedAt),
    "rollback baseline predates the admission-disable boundary",
  );
  assertPublicSafe(snapshot);
  return snapshot;
};

export const collectRollbackCapture = ({
  bundle,
  deployedRevision,
  admissionReceipt,
  runtimeReceipt,
  beforeSnapshot,
  runCommand = exactCommand,
  now = () => new Date().toISOString(),
}) => {
  validateAdmissionReceipt(admissionReceipt, bundle, deployedRevision);
  validateRuntimeReceipt(runtimeReceipt, bundle, deployedRevision);
  validateSnapshot(beforeSnapshot, bundle, deployedRevision, admissionReceipt);
  const observedAt = now();
  timestamp(observedAt, "rollback postcheck observedAt");
  assert(
    Date.parse(admissionReceipt.observedAt) <= Date.parse(beforeSnapshot.observedAt) &&
      Date.parse(beforeSnapshot.observedAt) <= Date.parse(runtimeReceipt.startedAt) &&
      Date.parse(runtimeReceipt.settledAt) <= Date.parse(observedAt),
    "rollback evidence boundaries are out of order",
  );
  const postcheck = {
    newAdmissionStillDisabled: admissionStillDisabled(runCommand),
    ...databaseCounts(runCommand),
    ...restoredPins(runCommand, bundle),
    unrelatedServiceDigestsBefore: beforeSnapshot.unrelatedServiceDigests,
    unrelatedServiceDigestsAfter: unrelatedServiceDigests(runCommand),
  };
  assert(
    JSON.stringify(postcheck.unrelatedServiceDigestsBefore) ===
      JSON.stringify(postcheck.unrelatedServiceDigestsAfter),
    "an unrelated production service changed during rollback",
  );
  const capture = {
    schemaVersion: "openagents.livekit_controlled_rollback_capture.v1",
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision,
    observedAt,
    admissionDisableReceipt: admissionReceipt,
    runtimeRollbackReceipt: {
      stage: runtimeReceipt.stage,
      phase: runtimeReceipt.phase,
      outcome: runtimeReceipt.outcome,
      evidenceTier: runtimeReceipt.evidenceTier,
      bundleDigest: runtimeReceipt.bundleDigest,
      resultDigest: runtimeReceipt.resultDigest,
    },
    postcheck,
  };
  assertPublicSafe(capture);
  return capture;
};

const run = () => {
  const args = parseArgs(process.argv.slice(2));
  const bundle = validateDeploymentBundle(readJson(args.bundle, "deployment bundle"));
  const admissionReceipt = readJson(args.admissionReceipt, "admission-disable receipt");
  if (!args.apply) {
    process.stdout.write(
      `${JSON.stringify({
        mode: "dry-run",
        phase: args.phase,
        liveStateRead: false,
        mutationExecuted: false,
        outputKind:
          args.phase === "baseline"
            ? "private unrelated-service baseline"
            : "private controlled rollback capture",
        checks:
          args.phase === "baseline"
            ? ["drained admission boundary", "ordered unrelated-service baseline"]
            : [
                "ordered evidence boundaries",
                "continued admission disable",
                "terminal rooms and accounting",
                "no transport mismatch",
                "restored target pins",
                "ordered unrelated-service equality",
              ],
      })}\n`,
    );
    return;
  }
  assert(
    process.env.OA_LIVEKIT_OWNER_GATE === OWNER_GATE,
    `--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`,
  );
  if (args.phase === "postcheck")
    assert(process.env.KUBECONFIG, "postcheck --apply requires an explicit KUBECONFIG");
  const value =
    args.phase === "baseline"
      ? collectRollbackBaseline({
          bundle,
          deployedRevision: args.deployedRevision,
          admissionReceipt,
        })
      : collectRollbackCapture({
          bundle,
          deployedRevision: args.deployedRevision,
          admissionReceipt,
          runtimeReceipt: readJson(args.runtimeReceipt, "runtime rollback receipt"),
          beforeSnapshot: readJson(args.beforeSnapshot, "rollback baseline"),
        });
  writePrivate(args.output, value);
  process.stdout.write(
    `${JSON.stringify({
      phase: args.phase,
      outcome: "passed",
      evidenceDigest: sha256(value),
      liveStateRead: true,
      mutationExecuted: false,
    })}\n`,
  );
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
