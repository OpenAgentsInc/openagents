#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, isAbsolute, relative, resolve } from "node:path";
import {
  REQUIRED_DRILLS,
  assertPublicSafe,
  buildPublicReceipt,
  sha256,
  validateDeploymentBundle,
} from "./livekit-ops-policy.mjs";

const OWNER_GATE = "I_ACCEPT_EP263_LIVEKIT_GCP_COST";
const PLAN_SCHEMA = "openagents.livekit_production_acceptance_plan.v1";
const PROBE_SCHEMA = "openagents.livekit_probe_result.v1";
const MUTATING_PHASES = new Set(["drills", "rollback"]);
const MAX_OUTPUT_BYTES = 16 * 1024 * 1024;
const OPAQUE_REF = /^[a-z][a-z0-9_.-]*-ref:\/\/[a-zA-Z0-9._~:/?#@!$&'()*+,;=%-]+$/u;
const PHASE_STEPS = Object.freeze({
  connectivity: Object.freeze(["production_preflight", "direct_udp", "tcp_fallback", "turn_tls"]),
  load: Object.freeze(["alpha_load"]),
  drills: REQUIRED_DRILLS,
  secret_scan: Object.freeze(["runtime_secret_scan"]),
  cost: Object.freeze(["billing_reconciliation"]),
  rollback: Object.freeze(["scoped_rollback"]),
});

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-production-acceptance.mjs \\
    --plan <private-plan.json> \\
    --bundle infra/livekit/bundle.json \\
    [--private-observation <path> --receipt docs/ops/receipts/livekit/<name>.json \\
      --apply [--allow-controlled-mutation]]

Default mode validates the closed plan and prints the exact ordered step IDs.
It executes no commands and writes no files.

--apply executes every probe as exact argv without a shell, captures stdout and
stderr privately, requires one closed-schema JSON result on stdout, and binds
each captured result to the observation through an opaque SHA-256 evidence ref.
The public receipt is written only after the existing LiveKit policy validator
accepts the aggregate observation.

Plans for drills and rollback additionally require --allow-controlled-mutation.
That flag is intentionally separate from the owner cost gate. It authorizes the
listed controlled production actions, not arbitrary infrastructure changes.
`);
};

const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const exactKeys = (value, required, optional, label) => {
  assert(value && typeof value === "object" && !Array.isArray(value), `${label} must be an object`);
  const allowed = new Set([...required, ...optional]);
  for (const key of Object.keys(value)) {
    assert(allowed.has(key), `${label} has unsupported field ${key}`);
  }
  for (const key of required) assert(Object.hasOwn(value, key), `${label} is missing ${key}`);
};

const readJson = (path, label) => {
  try {
    return JSON.parse(readFileSync(resolve(path), "utf8"));
  } catch (error) {
    throw new Error(`${label} is not readable JSON`, { cause: error });
  }
};

const parseArgs = (args) => {
  const parsed = {
    allowControlledMutation: false,
    apply: false,
    bundle: undefined,
    plan: undefined,
    privateObservation: undefined,
    receipt: undefined,
  };
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--apply") {
      parsed.apply = true;
      continue;
    }
    if (argument === "--allow-controlled-mutation") {
      parsed.allowControlledMutation = true;
      continue;
    }
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    if (["--bundle", "--plan", "--private-observation", "--receipt"].includes(argument)) {
      const value = args[index + 1];
      if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
      parsed[argument === "--private-observation" ? "privateObservation" : argument.slice(2)] =
        value;
      index += 1;
      continue;
    }
    throw new Error(`unsupported argument ${argument}`);
  }
  if (!parsed.bundle || !parsed.plan) throw new Error("--bundle and --plan are required");
  if (parsed.apply && (!parsed.privateObservation || !parsed.receipt)) {
    throw new Error("--apply requires --private-observation and --receipt");
  }
  if (!parsed.apply && (parsed.privateObservation || parsed.receipt)) {
    throw new Error("output paths are accepted only with --apply");
  }
  if (!parsed.apply && parsed.allowControlledMutation) {
    throw new Error("--allow-controlled-mutation is accepted only with --apply");
  }
  return parsed;
};

const validatePlan = (value, bundle) => {
  exactKeys(
    value,
    [
      "schemaVersion",
      "phase",
      "stage",
      "sourceBaseRevision",
      "deployedRevision",
      "resourceRefs",
      "steps",
    ],
    ["notes"],
    "plan",
  );
  assert(value.schemaVersion === PLAN_SCHEMA, "plan schema is unsupported");
  assert(Object.hasOwn(PHASE_STEPS, value.phase), "plan phase is unsupported");
  assert(value.stage === "production", "production acceptance plan stage must be production");
  assert(
    value.sourceBaseRevision === bundle.sourceBaseRevision,
    "plan source base does not match the deployment bundle",
  );
  assert(/^[0-9a-f]{40}$/u.test(value.deployedRevision), "plan deployed revision is invalid");
  assert(
    Array.isArray(value.resourceRefs) &&
      value.resourceRefs.length > 0 &&
      value.resourceRefs.length <= 32 - PHASE_STEPS[value.phase].length &&
      value.resourceRefs.every((resourceRef) => OPAQUE_REF.test(resourceRef)),
    "plan resourceRefs must be bounded opaque refs",
  );
  if (value.notes !== undefined) {
    assert(
      Array.isArray(value.notes) &&
        value.notes.length <= 16 &&
        value.notes.every(
          (note) => typeof note === "string" && note.length > 0 && note.length <= 240,
        ),
      "plan notes must be a bounded string array",
    );
  }
  assertPublicSafe({
    resourceRefs: value.resourceRefs,
    notes: value.notes ?? [],
  });

  const expectedSteps = PHASE_STEPS[value.phase];
  assert(
    Array.isArray(value.steps) && value.steps.length === expectedSteps.length,
    `plan ${value.phase} must contain exactly ${expectedSteps.length} steps`,
  );
  value.steps.forEach((step, index) => {
    exactKeys(step, ["id", "command", "timeoutSeconds"], [], `plan.steps[${index}]`);
    assert(step.id === expectedSteps[index], `plan step ${index} must be ${expectedSteps[index]}`);
    assert(
      Array.isArray(step.command) &&
        step.command.length > 0 &&
        step.command.length <= 64 &&
        step.command.every(
          (argument) =>
            typeof argument === "string" &&
            argument.length > 0 &&
            argument.length <= 4_096 &&
            !argument.includes("\0"),
        ),
      `plan.steps[${index}].command must be bounded exact argv`,
    );
    assert(
      !["bash", "sh", "zsh", "fish"].includes(basename(step.command[0])),
      `plan.steps[${index}] may not invoke a shell`,
    );
    assert(
      Number.isSafeInteger(step.timeoutSeconds) &&
        step.timeoutSeconds >= 1 &&
        step.timeoutSeconds <= 3_600,
      `plan.steps[${index}].timeoutSeconds must be 1 through 3600`,
    );
  });
  return value;
};

const parseProbeOutput = (stdout, step, phase, startedAt, completedAt) => {
  let probe;
  try {
    probe = JSON.parse(stdout);
  } catch (error) {
    throw new Error(`probe ${step.id} did not emit one JSON object`, { cause: error });
  }
  exactKeys(probe, ["schemaVersion", "phase", "stepId", "observedAt", "result"], [], step.id);
  assert(probe.schemaVersion === PROBE_SCHEMA, `probe ${step.id} schema is unsupported`);
  assert(probe.phase === phase, `probe ${step.id} phase is wrong`);
  assert(probe.stepId === step.id, `probe ${step.id} identity is wrong`);
  const observedAt = Date.parse(probe.observedAt);
  assert(Number.isFinite(observedAt), `probe ${step.id} observedAt is invalid`);
  assert(
    observedAt >= Date.parse(startedAt) - 300_000 &&
      observedAt <= Date.parse(completedAt) + 300_000,
    `probe ${step.id} observedAt is outside the execution window`,
  );
  return probe;
};

const projectResults = (phase, probes) => {
  const result = (id) => probes.find((probe) => probe.stepId === id).result;
  if (phase === "connectivity") {
    const preflight = result("production_preflight");
    exactKeys(
      preflight,
      ["packagedOmega", "signaling", "certificate", "publicIpAdvertisement"],
      [],
      "production_preflight.result",
    );
    return {
      ...preflight,
      modes: [
        { mode: "direct_udp", ...result("direct_udp") },
        { mode: "tcp_fallback", ...result("tcp_fallback") },
        { mode: "turn_tls", ...result("turn_tls") },
      ],
    };
  }
  if (phase === "drills") return { drills: REQUIRED_DRILLS.map((id) => result(id)) };
  return result(PHASE_STEPS[phase][0]);
};

const defaultRunCommand = (step, cwd) =>
  spawnSync(step.command[0], step.command.slice(1), {
    cwd,
    encoding: "utf8",
    env: process.env,
    maxBuffer: MAX_OUTPUT_BYTES,
    shell: false,
    timeout: step.timeoutSeconds * 1_000,
  });

const writeExclusive = (path, value) => {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
};

const isWithin = (parent, candidate) => {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
};

export const collectProductionAcceptance = ({
  bundle,
  plan,
  repositoryRoot = process.cwd(),
  runCommand = defaultRunCommand,
}) => {
  validatePlan(plan, bundle);
  const startedAt = new Date().toISOString();
  const probes = [];
  const evidenceRefs = [];
  for (const step of plan.steps) {
    const execution = runCommand(step, repositoryRoot);
    const completedAt = new Date().toISOString();
    if (execution.error) {
      throw new Error(`probe ${step.id} could not execute`, { cause: execution.error });
    }
    if (execution.status !== 0) {
      throw new Error(`probe ${step.id} failed with exit status ${execution.status}`);
    }
    const stdout = execution.stdout ?? "";
    const stderr = execution.stderr ?? "";
    const probe = parseProbeOutput(stdout, step, plan.phase, startedAt, completedAt);
    probes.push(probe);
    evidenceRefs.push(
      `livekit-evidence-ref://sha256/${sha256(
        JSON.stringify({
          stepId: step.id,
          command: step.command,
          stdout,
          stderr,
        }),
      ).slice("sha256:".length)}`,
    );
  }
  const settledAt = new Date().toISOString();
  const observation = {
    schemaVersion: "openagents.livekit_acceptance_observation.v1",
    phase: plan.phase,
    stage: plan.stage,
    sourceBaseRevision: plan.sourceBaseRevision,
    deployedRevision: plan.deployedRevision,
    resourceRefs: [...plan.resourceRefs, ...evidenceRefs],
    startedAt,
    settledAt,
    results: projectResults(plan.phase, probes),
    ...(plan.notes === undefined ? {} : { notes: plan.notes }),
  };
  const receipt = buildPublicReceipt(observation, bundle);
  return { observation, receipt };
};

const run = () => {
  const args = parseArgs(process.argv.slice(2));
  const repositoryRoot = resolve(import.meta.dirname, "../..");
  const bundle = validateDeploymentBundle(readJson(args.bundle, "deployment bundle"));
  const plan = validatePlan(readJson(args.plan, "private acceptance plan"), bundle);
  if (!args.apply) {
    process.stdout.write(
      `${JSON.stringify(
        {
          mode: "dry-run",
          phase: plan.phase,
          commandCount: plan.steps.length,
          orderedStepIds: plan.steps.map((step) => step.id),
          controlledMutationRequired: MUTATING_PHASES.has(plan.phase),
          commandsExecuted: false,
          filesWritten: false,
        },
        null,
        2,
      )}\n`,
    );
    return;
  }
  if (process.env.OA_LIVEKIT_OWNER_GATE !== OWNER_GATE) {
    throw new Error(`--apply requires OA_LIVEKIT_OWNER_GATE=${OWNER_GATE}`);
  }
  if (MUTATING_PHASES.has(plan.phase) && !args.allowControlledMutation) {
    throw new Error(`${plan.phase} requires --allow-controlled-mutation`);
  }

  const privateObservationPath = resolve(args.privateObservation);
  const receiptPath = resolve(args.receipt);
  const receiptRoot = resolve(repositoryRoot, "docs/ops/receipts/livekit");
  assert(
    !isWithin(repositoryRoot, privateObservationPath),
    "private observation must stay outside the repository",
  );
  assert(
    isWithin(receiptRoot, receiptPath),
    "public receipt must stay under the LiveKit receipt root",
  );
  const { observation, receipt } = collectProductionAcceptance({
    bundle,
    plan,
    repositoryRoot,
  });
  writeExclusive(privateObservationPath, observation);
  writeExclusive(receiptPath, receipt);
  process.stdout.write(
    `${JSON.stringify({
      phase: receipt.phase,
      outcome: receipt.outcome,
      receiptRef: receipt.receiptRef,
      resultDigest: receipt.resultDigest,
      receiptPath,
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
