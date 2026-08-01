import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";
import { buildControlledPlan } from "./livekit-controlled-plan.mjs";
import { buildControlledProbeResult } from "./livekit-controlled-probe.mjs";
import { REQUIRED_DRILLS, validateDeploymentBundle } from "./livekit-ops-policy.mjs";

const bundle = validateDeploymentBundle(
  JSON.parse(readFileSync(new URL("../../infra/livekit/bundle.json", import.meta.url), "utf8")),
);
const deployedRevision = "c".repeat(40);
const observedAt = "2026-08-01T12:00:00.000Z";
const digest = (value) => `sha256:${createHash("sha256").update(value).digest("hex")}`;

const drillCapture = (scenario = "sfu_pod_drain") => ({
  schemaVersion: "openagents.livekit_controlled_drill_capture.v1",
  sourceBaseRevision: bundle.sourceBaseRevision,
  deployedRevision,
  observedAt,
  scenario,
  action: {
    preconditionReceiptDigest: digest("precondition"),
    actionReceiptDigest: digest("action"),
    restorationReceiptDigest: digest("restoration"),
    targetScopeValidated: true,
    mutationObserved: true,
    restorationObserved: true,
    unrelatedServicesUnchanged: true,
  },
  observations: {
    serviceOutcome: "bounded_failure",
    failureVisible: true,
    maximumWorkerGenerationCount: 1,
    maximumProviderSessionCount: 1,
    accountingState: "settled",
    oldGenerationRejected: true,
    freshGenerationAdmitted: true,
    speechContinuityClaim: "not_claimed",
  },
});

test("derives one controlled drill row without accepting a pass boolean", () => {
  const result = buildControlledProbeResult({
    step: "sfu_pod_drain",
    capture: drillCapture(),
    bundle,
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision,
  });
  assert.deepEqual(result.result, {
    scenario: "sfu_pod_drain",
    outcome: "bounded_failure",
    visibleFailure: true,
    noProviderOverlap: true,
    accountingTerminal: true,
    freshAdmissionRequired: true,
    uninterruptedSpeechClaimed: false,
  });
});

test("controlled drill projection refuses scope, overlap, accounting, and unsupported continuity claims", () => {
  const cases = [
    ["scope", (capture) => (capture.action.targetScopeValidated = false)],
    ["overlap", (capture) => (capture.observations.maximumProviderSessionCount = 2)],
    ["accounting", (capture) => (capture.observations.accountingState = "accounting_uncertain")],
    ["continuity", (capture) => (capture.observations.speechContinuityClaim = "uninterrupted")],
  ];
  for (const [, mutate] of cases) {
    const capture = drillCapture();
    mutate(capture);
    assert.throws(() =>
      buildControlledProbeResult({
        step: "sfu_pod_drain",
        capture,
        bundle,
        sourceBaseRevision: bundle.sourceBaseRevision,
        deployedRevision,
      }),
    );
  }
});

const rollbackCapture = () => ({
  schemaVersion: "openagents.livekit_controlled_rollback_capture.v1",
  sourceBaseRevision: bundle.sourceBaseRevision,
  deployedRevision,
  observedAt,
  admissionDisableReceipt: {
    schemaVersion: "openagents.livekit_admission_disable.v1",
    stage: "production",
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision,
    observedAt,
    resourceRef: "livekit-admission-ref://production/livekit-room-v1",
    newAdmissionDisabled: true,
    newDispatchDisabled: true,
    activeRoomCount: 0,
    pendingSettlementCount: 0,
  },
  runtimeRollbackReceipt: {
    stage: "production",
    phase: "rollback",
    outcome: "rolled_back",
    evidenceTier: "live_observed",
    bundleDigest: digest(JSON.stringify(bundle)),
    resultDigest: digest("runtime-result"),
  },
  postcheck: {
    newAdmissionStillDisabled: true,
    activeRoomCount: 0,
    pendingSettlementCount: 0,
    silentTransportSwitchCount: 0,
    restoredBundleDigest: digest(JSON.stringify(bundle)),
    restoredConfigurationDigest: bundle.configurationDigest,
    restoredServerImageDigest: bundle.serverImage.digest,
    restoredWorkerImageDigest: bundle.workerImage.digest,
    unrelatedServiceDigestsBefore: [digest("cloud-run"), digest("database"), digest("relay")],
    unrelatedServiceDigestsAfter: [digest("cloud-run"), digest("database"), digest("relay")],
  },
});

test("derives rollback from admission, runtime, pin, settlement, and unrelated-service evidence", () => {
  const result = buildControlledProbeResult({
    step: "scoped_rollback",
    capture: rollbackCapture(),
    bundle,
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision,
  });
  assert.deepEqual(result.result, {
    newAdmissionDisabled: true,
    existingRoomsTerminal: true,
    noSilentTransportSwitch: true,
    settlementComplete: true,
    previousRevisionRestored: true,
    unrelatedServicesUnchanged: true,
  });
});

test("rollback projection refuses a changed unrelated service or wrong restored pin", () => {
  const changedService = rollbackCapture();
  changedService.postcheck.unrelatedServiceDigestsAfter[1] = digest("changed-database");
  assert.throws(() =>
    buildControlledProbeResult({
      step: "scoped_rollback",
      capture: changedService,
      bundle,
      sourceBaseRevision: bundle.sourceBaseRevision,
      deployedRevision,
    }),
  );
  const wrongPin = rollbackCapture();
  wrongPin.postcheck.restoredWorkerImageDigest = digest("wrong-worker");
  assert.throws(() =>
    buildControlledProbeResult({
      step: "scoped_rollback",
      capture: wrongPin,
      bundle,
      sourceBaseRevision: bundle.sourceBaseRevision,
      deployedRevision,
    }),
  );
});

test("controlled plan requires the exact drill inventory and orders every step", () => {
  const inputs = Object.fromEntries(REQUIRED_DRILLS.map((step) => [step, `/private/${step}.json`]));
  const plan = buildControlledPlan({
    phase: "drills",
    bundlePath: "/repo/infra/livekit/bundle.json",
    bundle,
    deployedRevision,
    resourceRefs: ["gke-cluster-ref://openagentsgemini/us-central1/oa-livekit-prod"],
    inputs,
  });
  assert.deepEqual(
    plan.steps.map((step) => step.id),
    REQUIRED_DRILLS,
  );
  assert.ok(
    plan.steps.every((step) => step.command.includes("scripts/cloud/livekit-controlled-probe.mjs")),
  );
  assert.throws(() =>
    buildControlledPlan({
      phase: "drills",
      bundlePath: "/repo/infra/livekit/bundle.json",
      bundle,
      deployedRevision,
      resourceRefs: ["gke-cluster-ref://openagentsgemini/us-central1/oa-livekit-prod"],
      inputs: { sfu_pod_drain: "/private/only-one.json" },
    }),
  );
});
