#!/usr/bin/env node

import { lstatSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  REQUIRED_DRILLS,
  validateDeploymentBundle,
  validateDrillObservation,
  validateRollbackObservation,
} from "./livekit-ops-policy.mjs";

const PROBE_SCHEMA = "openagents.livekit_probe_result.v1";
const COMMIT = /^[0-9a-f]{40}$/u;
const DIGEST = /^sha256:[0-9a-f]{64}$/u;
const TERMINAL_ACCOUNTING = new Set(["settled", "released"]);

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-controlled-probe.mjs \\
    --step <required-drill-id>|scoped_rollback \\
    --bundle infra/livekit/bundle.json \\
    --source-base-revision <40-hex> \\
    --deployed-revision <40-hex> \\
    --input <private-controlled-capture.json>

Projects a closed, revision-bound controlled-mutation capture into the exact
probe result used by livekit-production-acceptance.mjs. It performs no mutation
and accepts no operator-authored pass value. Drill outcomes are derived from
targeting, restoration, overlap, terminal-accounting, and generation evidence.
Rollback outcomes are derived from the admission-disable/runtime receipts and
before/after service digests.
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

const timestamp = (value, label) => {
  assert(typeof value === "string" && Number.isFinite(Date.parse(value)), `${label} is invalid`);
  return value;
};

const integer = (value, minimum, maximum, label) => {
  assert(
    Number.isSafeInteger(value) && value >= minimum && value <= maximum,
    `${label} is invalid`,
  );
  return value;
};

const digest = (value, label) => {
  assert(typeof value === "string" && DIGEST.test(value), `${label} is invalid`);
  return value;
};

const binding = (capture, schema, sourceBaseRevision, deployedRevision) => {
  assert(capture.schemaVersion === schema, "controlled capture schema is unsupported");
  assert(
    capture.sourceBaseRevision === sourceBaseRevision,
    "controlled capture source revision drifted",
  );
  assert(
    capture.deployedRevision === deployedRevision,
    "controlled capture deployed revision drifted",
  );
  timestamp(capture.observedAt, "controlled capture observedAt");
};

const projectDrill = (capture, step, sourceBaseRevision, deployedRevision) => {
  exactKeys(
    capture,
    [
      "schemaVersion",
      "sourceBaseRevision",
      "deployedRevision",
      "observedAt",
      "scenario",
      "action",
      "observations",
    ],
    [],
    "drill capture",
  );
  binding(
    capture,
    "openagents.livekit_controlled_drill_capture.v1",
    sourceBaseRevision,
    deployedRevision,
  );
  assert(capture.scenario === step, "drill capture scenario does not match the plan step");
  exactKeys(
    capture.action,
    [
      "preconditionReceiptDigest",
      "actionReceiptDigest",
      "restorationReceiptDigest",
      "targetScopeValidated",
      "mutationObserved",
      "restorationObserved",
      "unrelatedServicesUnchanged",
    ],
    [],
    "drill action",
  );
  for (const name of [
    "preconditionReceiptDigest",
    "actionReceiptDigest",
    "restorationReceiptDigest",
  ]) {
    digest(capture.action[name], `drill action ${name}`);
  }
  for (const name of [
    "targetScopeValidated",
    "mutationObserved",
    "restorationObserved",
    "unrelatedServicesUnchanged",
  ]) {
    assert(capture.action[name] === true, `drill action ${name} was not observed`);
  }
  exactKeys(
    capture.observations,
    [
      "serviceOutcome",
      "failureVisible",
      "maximumWorkerGenerationCount",
      "maximumProviderSessionCount",
      "accountingState",
      "oldGenerationRejected",
      "freshGenerationAdmitted",
      "speechContinuityClaim",
    ],
    [],
    "drill observations",
  );
  const outcome = capture.observations.serviceOutcome;
  assert(
    ["continued", "bounded_failure"].includes(outcome),
    "drill service outcome is unsupported",
  );
  const maximumWorkerGenerationCount = integer(
    capture.observations.maximumWorkerGenerationCount,
    0,
    100,
    "maximum worker generation count",
  );
  const maximumProviderSessionCount = integer(
    capture.observations.maximumProviderSessionCount,
    0,
    100,
    "maximum provider session count",
  );
  assert(
    capture.observations.speechContinuityClaim === "not_claimed",
    "controlled drill capture made an uninterrupted speech claim",
  );
  const result = {
    scenario: step,
    outcome,
    visibleFailure: capture.observations.failureVisible === true || outcome === "continued",
    noProviderOverlap: maximumWorkerGenerationCount <= 1 && maximumProviderSessionCount <= 1,
    accountingTerminal: TERMINAL_ACCOUNTING.has(capture.observations.accountingState),
    freshAdmissionRequired:
      capture.observations.oldGenerationRejected === true &&
      capture.observations.freshGenerationAdmitted === true,
    uninterruptedSpeechClaimed: false,
  };
  validateDrillObservation({
    schemaVersion: "openagents.livekit_acceptance_observation.v1",
    phase: "drills",
    stage: "production",
    sourceBaseRevision,
    deployedRevision,
    resourceRefs: ["livekit-controlled-drill-ref://production/validation"],
    startedAt: capture.observedAt,
    settledAt: capture.observedAt,
    results: {
      drills: REQUIRED_DRILLS.map((scenario) =>
        scenario === step
          ? result
          : {
              scenario,
              outcome: "continued",
              visibleFailure: false,
              noProviderOverlap: true,
              accountingTerminal: true,
              freshAdmissionRequired: true,
              uninterruptedSpeechClaimed: false,
            },
      ),
    },
  });
  return result;
};

const projectRollback = (capture, bundle, sourceBaseRevision, deployedRevision) => {
  exactKeys(
    capture,
    [
      "schemaVersion",
      "sourceBaseRevision",
      "deployedRevision",
      "observedAt",
      "admissionDisableReceipt",
      "runtimeRollbackReceipt",
      "postcheck",
    ],
    [],
    "rollback capture",
  );
  binding(
    capture,
    "openagents.livekit_controlled_rollback_capture.v1",
    sourceBaseRevision,
    deployedRevision,
  );
  exactKeys(
    capture.admissionDisableReceipt,
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
  const admission = capture.admissionDisableReceipt;
  assert(
    admission.schemaVersion === "openagents.livekit_admission_disable.v1" &&
      admission.stage === "production" &&
      admission.sourceBaseRevision === sourceBaseRevision &&
      admission.deployedRevision === deployedRevision &&
      admission.resourceRef === "livekit-admission-ref://production/livekit-room-v1" &&
      admission.newAdmissionDisabled === true &&
      admission.newDispatchDisabled === true &&
      admission.activeRoomCount === 0 &&
      admission.pendingSettlementCount === 0,
    "rollback admission-disable receipt is not a drained production boundary",
  );
  timestamp(admission.observedAt, "admission-disable receipt observedAt");
  exactKeys(
    capture.runtimeRollbackReceipt,
    ["stage", "phase", "outcome", "evidenceTier", "bundleDigest", "resultDigest"],
    [],
    "runtime rollback receipt projection",
  );
  const runtime = capture.runtimeRollbackReceipt;
  assert(
    runtime.stage === "production" &&
      runtime.phase === "rollback" &&
      runtime.outcome === "rolled_back" &&
      runtime.evidenceTier === "live_observed",
    "runtime rollback receipt did not prove a live production rollback",
  );
  digest(runtime.bundleDigest, "runtime rollback bundle digest");
  digest(runtime.resultDigest, "runtime rollback result digest");
  exactKeys(
    capture.postcheck,
    [
      "newAdmissionStillDisabled",
      "activeRoomCount",
      "pendingSettlementCount",
      "silentTransportSwitchCount",
      "restoredBundleDigest",
      "restoredConfigurationDigest",
      "restoredServerImageDigest",
      "restoredWorkerImageDigest",
      "unrelatedServiceDigestsBefore",
      "unrelatedServiceDigestsAfter",
    ],
    [],
    "rollback postcheck",
  );
  const postcheck = capture.postcheck;
  for (const name of [
    "restoredBundleDigest",
    "restoredConfigurationDigest",
    "restoredServerImageDigest",
    "restoredWorkerImageDigest",
  ])
    digest(postcheck[name], `rollback postcheck ${name}`);
  const before = postcheck.unrelatedServiceDigestsBefore;
  const after = postcheck.unrelatedServiceDigestsAfter;
  assert(
    Array.isArray(before) &&
      before.length > 0 &&
      before.length <= 32 &&
      before.every((value) => DIGEST.test(value)),
    "rollback unrelated-service before digests are invalid",
  );
  assert(
    Array.isArray(after) && after.every((value) => DIGEST.test(value)),
    "rollback unrelated-service after digests are invalid",
  );
  const results = {
    newAdmissionDisabled: postcheck.newAdmissionStillDisabled === true,
    existingRoomsTerminal: postcheck.activeRoomCount === 0,
    noSilentTransportSwitch: postcheck.silentTransportSwitchCount === 0,
    settlementComplete: postcheck.pendingSettlementCount === 0,
    previousRevisionRestored:
      postcheck.restoredBundleDigest === runtime.bundleDigest &&
      postcheck.restoredConfigurationDigest === bundle.configurationDigest &&
      postcheck.restoredServerImageDigest === bundle.serverImage.digest &&
      postcheck.restoredWorkerImageDigest === bundle.workerImage.digest,
    unrelatedServicesUnchanged: JSON.stringify(before) === JSON.stringify(after),
  };
  validateRollbackObservation({
    schemaVersion: "openagents.livekit_acceptance_observation.v1",
    phase: "rollback",
    stage: "production",
    sourceBaseRevision,
    deployedRevision,
    resourceRefs: ["livekit-controlled-rollback-ref://production/validation"],
    startedAt: capture.observedAt,
    settledAt: capture.observedAt,
    results,
  });
  return results;
};

export const buildControlledProbeResult = ({
  step,
  capture,
  bundle,
  sourceBaseRevision,
  deployedRevision,
}) => {
  assert(bundle.sourceBaseRevision === sourceBaseRevision, "bundle source revision drifted");
  assert(COMMIT.test(deployedRevision), "deployed revision is invalid");
  if (REQUIRED_DRILLS.includes(step)) {
    return {
      schemaVersion: PROBE_SCHEMA,
      phase: "drills",
      stepId: step,
      observedAt: capture.observedAt,
      result: projectDrill(capture, step, sourceBaseRevision, deployedRevision),
    };
  }
  assert(step === "scoped_rollback", "controlled probe step is unsupported");
  return {
    schemaVersion: PROBE_SCHEMA,
    phase: "rollback",
    stepId: step,
    observedAt: capture.observedAt,
    result: projectRollback(capture, bundle, sourceBaseRevision, deployedRevision),
  };
};

const parseArgs = (values) => {
  const parsed = {};
  for (let index = 0; index < values.length; index += 1) {
    const argument = values[index];
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    if (
      !["--bundle", "--deployed-revision", "--input", "--source-base-revision", "--step"].includes(
        argument,
      )
    ) {
      throw new Error(`unsupported argument ${argument}`);
    }
    const value = values[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    parsed[
      {
        "--bundle": "bundle",
        "--deployed-revision": "deployedRevision",
        "--input": "input",
        "--source-base-revision": "sourceBaseRevision",
        "--step": "step",
      }[argument]
    ] = value;
    index += 1;
  }
  for (const field of ["bundle", "deployedRevision", "input", "sourceBaseRevision", "step"]) {
    if (!parsed[field]) throw new Error(`missing required argument ${field}`);
  }
  return parsed;
};

const run = () => {
  const args = parseArgs(process.argv.slice(2));
  const input = lstatSync(resolve(args.input));
  assert(
    input.isFile() && !input.isSymbolicLink() && (input.mode & 0o077) === 0,
    "private controlled capture must be a mode-0600-or-stricter regular file",
  );
  const bundle = validateDeploymentBundle(JSON.parse(readFileSync(resolve(args.bundle), "utf8")));
  const capture = JSON.parse(readFileSync(resolve(args.input), "utf8"));
  process.stdout.write(
    `${JSON.stringify(
      buildControlledProbeResult({
        step: args.step,
        capture,
        bundle,
        sourceBaseRevision: args.sourceBaseRevision,
        deployedRevision: args.deployedRevision,
      }),
    )}\n`,
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
