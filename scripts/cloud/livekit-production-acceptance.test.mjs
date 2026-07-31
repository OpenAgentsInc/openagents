import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { collectProductionAcceptance } from "./livekit-production-acceptance.mjs";
import { validateDeploymentBundle } from "./livekit-ops-policy.mjs";

const bundle = validateDeploymentBundle(
  JSON.parse(readFileSync(new URL("../../infra/livekit/bundle.json", import.meta.url), "utf8")),
);
const deployedRevision = "1".repeat(40);

const probe = (phase, stepId, result) => ({
  schemaVersion: "openagents.livekit_probe_result.v1",
  phase,
  stepId,
  observedAt: new Date().toISOString(),
  result,
});

const connectivityPlan = {
  schemaVersion: "openagents.livekit_production_acceptance_plan.v1",
  phase: "connectivity",
  stage: "production",
  sourceBaseRevision: bundle.sourceBaseRevision,
  deployedRevision,
  resourceRefs: ["gke-cluster-ref://openagentsgemini/us-central1/oa-livekit-prod"],
  steps: [
    {
      id: "production_preflight",
      command: ["probe", "preflight"],
      timeoutSeconds: 60,
    },
    { id: "direct_udp", command: ["probe", "direct"], timeoutSeconds: 60 },
    { id: "tcp_fallback", command: ["probe", "tcp"], timeoutSeconds: 60 },
    { id: "turn_tls", command: ["probe", "turn"], timeoutSeconds: 60 },
  ],
};

const modeResult = {
  roomJoined: true,
  microphonePublished: true,
  sarahAudioSubscribed: true,
  selectedPathObserved: true,
  sessionSettled: true,
  p95JoinMs: 500,
  p95FirstAudioMs: 900,
};

test("collector executes ordered probes and produces a policy-valid opaque receipt", () => {
  const results = new Map([
    [
      "production_preflight",
      {
        packagedOmega: true,
        signaling: true,
        certificate: true,
        publicIpAdvertisement: true,
      },
    ],
    ["direct_udp", modeResult],
    ["tcp_fallback", modeResult],
    ["turn_tls", modeResult],
  ]);
  const calls = [];
  const { observation, receipt } = collectProductionAcceptance({
    bundle,
    plan: connectivityPlan,
    runCommand(step) {
      calls.push(step.id);
      return {
        status: 0,
        stdout: JSON.stringify(probe("connectivity", step.id, results.get(step.id))),
        stderr: "private diagnostic",
      };
    },
  });
  assert.deepEqual(
    calls,
    connectivityPlan.steps.map((step) => step.id),
  );
  assert.equal(observation.results.modes.length, 3);
  assert.equal(
    observation.resourceRefs.filter((ref) => ref.startsWith("livekit-evidence-ref://")).length,
    4,
  );
  assert.equal(receipt.outcome, "passed");
  assert.equal(receipt.liveProof, true);
  assert.ok(!JSON.stringify(receipt).includes("private diagnostic"));
});

test("collector rejects failed, mislabeled, and policy-invalid probe evidence", () => {
  assert.throws(
    () =>
      collectProductionAcceptance({
        bundle,
        plan: connectivityPlan,
        runCommand: () => ({ status: 3, stdout: "", stderr: "secret output" }),
      }),
    /failed with exit status 3/u,
  );
  assert.throws(
    () =>
      collectProductionAcceptance({
        bundle,
        plan: connectivityPlan,
        runCommand: (step) => ({
          status: 0,
          stdout: JSON.stringify(probe("load", step.id, {})),
          stderr: "",
        }),
      }),
    /phase is wrong/u,
  );

  const results = new Map([
    [
      "production_preflight",
      {
        packagedOmega: true,
        signaling: true,
        certificate: true,
        publicIpAdvertisement: true,
      },
    ],
    ["direct_udp", modeResult],
    ["tcp_fallback", { ...modeResult, selectedPathObserved: false }],
    ["turn_tls", modeResult],
  ]);
  assert.throws(
    () =>
      collectProductionAcceptance({
        bundle,
        plan: connectivityPlan,
        runCommand: (step) => ({
          status: 0,
          stdout: JSON.stringify(probe("connectivity", step.id, results.get(step.id))),
          stderr: "",
        }),
      }),
    /tcp_fallback.selectedPathObserved did not pass/u,
  );
});

test("plan rejects shells, reordered steps, and unbound deployment source", () => {
  for (const plan of [
    {
      ...connectivityPlan,
      sourceBaseRevision: "2".repeat(40),
    },
    {
      ...connectivityPlan,
      steps: [
        connectivityPlan.steps[1],
        connectivityPlan.steps[0],
        ...connectivityPlan.steps.slice(2),
      ],
    },
    {
      ...connectivityPlan,
      steps: [
        {
          ...connectivityPlan.steps[0],
          command: ["/bin/bash", "-lc", "echo unsafe"],
        },
        ...connectivityPlan.steps.slice(1),
      ],
    },
  ]) {
    assert.throws(() =>
      collectProductionAcceptance({
        bundle,
        plan,
        runCommand: () => {
          throw new Error("must not execute");
        },
      }),
    );
  }
});
