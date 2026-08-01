#!/usr/bin/env node

import { lstatSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { REQUIRED_DRILLS, validateDeploymentBundle } from "./livekit-ops-policy.mjs";

const PLAN_SCHEMA = "openagents.livekit_production_acceptance_plan.v1";
const COMMIT = /^[0-9a-f]{40}$/u;
const OPAQUE_REF = /^[a-z][a-z0-9_.-]*-ref:\/\/[a-zA-Z0-9._~:/?#@!$&'()*+,;=%-]+$/u;
const PHASE_STEPS = Object.freeze({
  drills: REQUIRED_DRILLS,
  rollback: Object.freeze(["scoped_rollback"]),
});

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-controlled-plan.mjs \\
    --phase drills|rollback --bundle infra/livekit/bundle.json \\
    --deployed-revision <40-hex> --resource-ref <opaque-ref> \\
    --input <step-id>=<private-capture.json> [--input ...] \\
    --output <private-plan.json>

Writes an exclusive mode-0600 plan for the controlled-mutation acceptance
adapter. It performs no mutation. The production acceptance runner still
requires both its owner gate and --allow-controlled-mutation before execution.
`);
};

const isWithin = (parent, candidate) => {
  const path = relative(parent, candidate);
  return path === "" || (!path.startsWith("..") && !isAbsolute(path));
};

const parseArgs = (values) => {
  const parsed = { inputs: {}, resourceRefs: [] };
  for (let index = 0; index < values.length; index += 1) {
    const argument = values[index];
    if (argument === "--help" || argument === "-h") {
      usage();
      process.exit(0);
    }
    const value = values[index + 1];
    if (!value || value.startsWith("--")) throw new Error(`${argument} requires a value`);
    if (argument === "--input") {
      const separator = value.indexOf("=");
      if (separator <= 0 || separator === value.length - 1)
        throw new Error("--input must use step-id=path");
      const step = value.slice(0, separator);
      if (parsed.inputs[step]) throw new Error(`duplicate input for ${step}`);
      parsed.inputs[step] = resolve(value.slice(separator + 1));
    } else if (argument === "--resource-ref") {
      parsed.resourceRefs.push(value);
    } else if (["--bundle", "--deployed-revision", "--output", "--phase"].includes(argument)) {
      parsed[argument.slice(2).replace("deployed-revision", "deployedRevision")] = value;
    } else {
      throw new Error(`unsupported argument ${argument}`);
    }
    index += 1;
  }
  for (const field of ["bundle", "deployedRevision", "output", "phase"]) {
    if (!parsed[field]) throw new Error(`missing required argument ${field}`);
  }
  return parsed;
};

export const buildControlledPlan = ({
  phase,
  bundlePath,
  bundle,
  deployedRevision,
  resourceRefs,
  inputs,
}) => {
  const steps = PHASE_STEPS[phase];
  if (!steps) throw new Error("controlled plan supports only drills and rollback");
  if (!COMMIT.test(deployedRevision))
    throw new Error("deployed revision must be a full Git commit");
  if (
    !Array.isArray(resourceRefs) ||
    resourceRefs.length === 0 ||
    !resourceRefs.every((ref) => OPAQUE_REF.test(ref))
  ) {
    throw new Error("at least one opaque resource ref is required");
  }
  if (JSON.stringify(Object.keys(inputs).toSorted()) !== JSON.stringify([...steps].toSorted())) {
    throw new Error("private inputs must match the exact controlled phase steps");
  }
  return {
    schemaVersion: PLAN_SCHEMA,
    phase,
    stage: "production",
    sourceBaseRevision: bundle.sourceBaseRevision,
    deployedRevision,
    resourceRefs,
    steps: steps.map((step) => ({
      id: step,
      command: [
        process.execPath,
        "scripts/cloud/livekit-controlled-probe.mjs",
        "--step",
        step,
        "--bundle",
        bundlePath,
        "--source-base-revision",
        bundle.sourceBaseRevision,
        "--deployed-revision",
        deployedRevision,
        "--input",
        inputs[step],
      ],
      timeoutSeconds: 60,
    })),
  };
};

const run = () => {
  const args = parseArgs(process.argv.slice(2));
  const repositoryRoot = resolve(import.meta.dirname, "../..");
  const bundlePath = resolve(args.bundle);
  const bundle = validateDeploymentBundle(JSON.parse(readFileSync(bundlePath, "utf8")));
  const output = resolve(args.output);
  if (isWithin(repositoryRoot, output))
    throw new Error("private controlled plan must stay outside the repository");
  if (Object.values(args.inputs).some((input) => isWithin(repositoryRoot, input))) {
    throw new Error("private controlled captures must stay outside the repository");
  }
  for (const inputPath of Object.values(args.inputs)) {
    const input = lstatSync(inputPath);
    if (!input.isFile() || input.isSymbolicLink() || (input.mode & 0o077) !== 0) {
      throw new Error("private controlled captures must be mode-0600-or-stricter regular files");
    }
  }
  const plan = buildControlledPlan({
    phase: args.phase,
    bundlePath,
    bundle,
    deployedRevision: args.deployedRevision,
    resourceRefs: args.resourceRefs,
    inputs: args.inputs,
  });
  mkdirSync(dirname(output), { recursive: true, mode: 0o700 });
  writeFileSync(output, `${JSON.stringify(plan, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
    mode: 0o600,
  });
  process.stdout.write(
    `${JSON.stringify({ phase: plan.phase, stepCount: plan.steps.length, output })}\n`,
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
