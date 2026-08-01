import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import { collectRollbackBaseline, collectRollbackCapture } from "./livekit-rollback-capture.mjs";
import { buildControlledProbeResult } from "./livekit-controlled-probe.mjs";
import { validateDeploymentBundle } from "./livekit-ops-policy.mjs";

const bundle = validateDeploymentBundle(
  JSON.parse(readFileSync(new URL("../../infra/livekit/bundle.json", import.meta.url), "utf8")),
);
const deployedRevision = "a".repeat(40);
const digest = (value) => `sha256:${createHash("sha256").update(value).digest("hex")}`;
const bundleDigest = digest(JSON.stringify(bundle));

const admissionReceipt = () => ({
  schemaVersion: "openagents.livekit_admission_disable.v1",
  stage: "production",
  sourceBaseRevision: bundle.sourceBaseRevision,
  deployedRevision,
  observedAt: "2026-08-01T12:00:00.000Z",
  resourceRef: "livekit-admission-ref://production/livekit-room-v1",
  newAdmissionDisabled: true,
  newDispatchDisabled: true,
  activeRoomCount: 0,
  pendingSettlementCount: 0,
});

const runtimeReceipt = () => ({
  schemaVersion: "openagents.livekit_ops_receipt.v1",
  stage: "production",
  phase: "rollback",
  sourceBaseRevision: bundle.sourceBaseRevision,
  deployedRevision,
  bundleDigest,
  configurationDigest: bundle.configurationDigest,
  startedAt: "2026-08-01T12:02:00.000Z",
  settledAt: "2026-08-01T12:03:00.000Z",
  outcome: "rolled_back",
  evidenceTier: "live_observed",
  liveProof: true,
  resultDigest: digest("runtime-result"),
});

const service = (revisionName = "openagents-monolith-00400-test") => ({
  status: {
    latestReadyRevisionName: revisionName,
    traffic: [{ revisionName, percent: 100 }],
    conditions: [{ type: "Ready", status: "True" }],
  },
});

const revision = (admission = "false") => ({
  metadata: { name: "openagents-monolith-00400-test" },
  spec: {
    containers: [{ env: [{ name: "SARAH_LIVEKIT_NEW_ADMISSIONS_ENABLED", value: admission }] }],
  },
  status: { conditions: [{ type: "Ready", status: "True" }] },
});

const deployment = (name, image, annotations = {}) => ({
  metadata: { name, generation: 7 },
  spec: {
    replicas: 3,
    template: {
      metadata: { annotations },
      spec: { containers: [{ name, image }] },
    },
  },
  status: {
    observedGeneration: 7,
    readyReplicas: 3,
    updatedReplicas: 3,
    availableReplicas: 3,
  },
});

const deployments = () => ({
  items: [
    deployment("livekit-server", bundle.serverImage.reference, {
      "checksum/config": bundle.configurationDigest.slice("sha256:".length),
    }),
    deployment("sarah-livekit-agent", bundle.workerImage.reference),
  ],
});

const withDatabaseEnvironment = (run) => {
  const names = ["PGHOST", "PGUSER", "PGPASSWORD", "PGDATABASE"];
  const held = Object.fromEntries(names.map((name) => [name, process.env[name]]));
  Object.assign(process.env, {
    PGHOST: "127.0.0.1",
    PGUSER: "rollback-reader",
    PGPASSWORD: "never-in-argv",
    PGDATABASE: "khala_sync_prod",
  });
  try {
    return run();
  } finally {
    for (const [name, value] of Object.entries(held)) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
  }
};

const commandRunner =
  ({
    admission = "false",
    counts = "khala_sync_prod,0,0,0\n",
    cloudRunService = service(),
    deploymentInventory = deployments(),
    kubeContext = "gke_openagentsgemini_us-central1_oa-livekit-prod",
    updateGeneration = "11",
  } = {}) =>
  (bin, args, environment) => {
    if (bin === "psql") {
      assert.equal(
        args.some((value) => value.includes("never-in-argv")),
        false,
      );
      assert.equal(environment.PGPASSWORD, "never-in-argv");
      return { status: 0, stdout: counts, stderr: "" };
    }
    if (bin === "kubectl") {
      if (args[0] === "config") {
        return {
          status: 0,
          stdout: JSON.stringify({
            "current-context": kubeContext,
            contexts: [
              {
                name: kubeContext,
                context: { cluster: kubeContext },
              },
            ],
          }),
          stderr: "",
        };
      }
      assert.deepEqual(args.slice(0, 4), ["--namespace", "livekit-system", "get", "deployment"]);
      return { status: 0, stdout: JSON.stringify(deploymentInventory), stderr: "" };
    }
    assert.equal(bin, "gcloud");
    if (args[0] === "run" && args[1] === "services" && args[2] === "describe")
      return { status: 0, stdout: JSON.stringify(cloudRunService), stderr: "" };
    if (args[0] === "run" && args[1] === "revisions")
      return { status: 0, stdout: JSON.stringify(revision(admission)), stderr: "" };
    if (args[0] === "sql")
      return {
        status: 0,
        stdout: JSON.stringify([{ name: "database", settings: { settingsVersion: "22" } }]),
        stderr: "",
      };
    if (args[0] === "compute")
      return {
        status: 0,
        stdout: JSON.stringify([{ name: "sandbox-control", fingerprint: "stable" }]),
        stderr: "",
      };
    const filter = args.find((argument) => argument.startsWith("--filter="));
    const name = filter?.slice("--filter=metadata.name=".length);
    return {
      status: 0,
      stdout: JSON.stringify([
        {
          metadata: { name, generation: name === "oa-updates" ? updateGeneration : "4" },
          status: { latestReadyRevisionName: `${name}-revision`, traffic: [{ percent: 100 }] },
        },
      ]),
      stderr: "",
    };
  };

const baseline = (options) =>
  withDatabaseEnvironment(() =>
    collectRollbackBaseline({
      bundle,
      deployedRevision,
      admissionReceipt: admissionReceipt(),
      runCommand: commandRunner(options),
      now: () => "2026-08-01T12:01:00.000Z",
    }),
  );

const capture = (beforeSnapshot, options) =>
  withDatabaseEnvironment(() =>
    collectRollbackCapture({
      bundle,
      deployedRevision,
      admissionReceipt: admissionReceipt(),
      runtimeReceipt: runtimeReceipt(),
      beforeSnapshot,
      runCommand: commandRunner(options),
      now: () => "2026-08-01T12:04:00.000Z",
    }),
  );

test("collects an ordered live baseline and derives a valid controlled rollback capture", () => {
  const before = baseline();
  assert.equal(before.unrelatedServiceDigests.length, 5);
  assert.ok(before.unrelatedServiceDigests.every((value) => /^sha256:[0-9a-f]{64}$/u.test(value)));

  const result = capture(before);
  assert.equal(result.schemaVersion, "openagents.livekit_controlled_rollback_capture.v1");
  assert.deepEqual(result.runtimeRollbackReceipt, {
    stage: "production",
    phase: "rollback",
    outcome: "rolled_back",
    evidenceTier: "live_observed",
    bundleDigest,
    resultDigest: digest("runtime-result"),
  });
  assert.deepEqual(result.postcheck, {
    newAdmissionStillDisabled: true,
    activeRoomCount: 0,
    pendingSettlementCount: 0,
    silentTransportSwitchCount: 0,
    restoredBundleDigest: bundleDigest,
    restoredConfigurationDigest: bundle.configurationDigest,
    restoredServerImageDigest: bundle.serverImage.digest,
    restoredWorkerImageDigest: bundle.workerImage.digest,
    unrelatedServiceDigestsBefore: before.unrelatedServiceDigests,
    unrelatedServiceDigestsAfter: before.unrelatedServiceDigests,
  });
  assert.equal(JSON.stringify(result).includes("sandbox-control"), false);
  assert.equal(JSON.stringify(result).includes("never-in-argv"), false);
  assert.deepEqual(
    buildControlledProbeResult({
      step: "scoped_rollback",
      capture: result,
      bundle,
      sourceBaseRevision: bundle.sourceBaseRevision,
      deployedRevision,
    }).result,
    {
      newAdmissionDisabled: true,
      existingRoomsTerminal: true,
      noSilentTransportSwitch: true,
      settlementComplete: true,
      previousRevisionRestored: true,
      unrelatedServicesUnchanged: true,
    },
  );
});

test("accepts tagged Cloud Run revisions that receive no traffic", () => {
  const tagged = service();
  tagged.status.traffic.push({
    revisionName: "openagents-monolith-broker-test",
    tag: "broker-test",
  });
  assert.equal(baseline({ cloudRunService: tagged }).unrelatedServiceDigests.length, 5);
});

test("refuses admission drift, nonterminal database state, and silent transport mismatches", () => {
  assert.throws(() => baseline({ admission: "true" }), /admission is no longer disabled/u);
  const before = baseline();
  assert.throws(
    () => capture(before, { counts: "khala_sync_prod,1,0,0\n" }),
    /active LiveKit rooms/u,
  );
  assert.throws(
    () => capture(before, { counts: "khala_sync_prod,0,1,0\n" }),
    /pending LiveKit settlements/u,
  );
  assert.throws(
    () => capture(before, { counts: "khala_sync_prod,0,0,1\n" }),
    /changed transport silently/u,
  );
});

test("refuses pin drift and an unrelated service change", () => {
  const before = baseline();
  assert.throws(
    () => capture(before, { kubeContext: "gke_other-project_us-central1_oa-livekit-prod" }),
    /exact production LiveKit cluster/u,
  );
  const wrongWorker = deployments();
  wrongWorker.items[1].spec.template.spec.containers[0].image = `example.invalid/worker@${digest("wrong-worker")}`;
  assert.throws(() => capture(before, { deploymentInventory: wrongWorker }), /pins do not match/u);
  assert.throws(
    () => capture(before, { updateGeneration: "12" }),
    /unrelated production service changed/u,
  );
});

test("refuses receipt drift and out-of-order observation boundaries", () => {
  const before = baseline();
  const wrongRuntime = runtimeReceipt();
  wrongRuntime.bundleDigest = digest("wrong-bundle");
  assert.throws(
    () =>
      withDatabaseEnvironment(() =>
        collectRollbackCapture({
          bundle,
          deployedRevision,
          admissionReceipt: admissionReceipt(),
          runtimeReceipt: wrongRuntime,
          beforeSnapshot: before,
          runCommand: commandRunner(),
          now: () => "2026-08-01T12:04:00.000Z",
        }),
      ),
    /exact target bundle/u,
  );
  const lateBaseline = { ...before, observedAt: "2026-08-01T12:02:30.000Z" };
  assert.throws(() => capture(lateBaseline), /boundaries are out of order/u);
});
