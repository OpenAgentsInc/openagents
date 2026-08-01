#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { validateSourceOnlyReceipt } from "./livekit-ops-policy.mjs";

const PROJECT = "openagentsgemini";
const REGION = "us-central1";
const TRIGGER = "oa-livekit-prod-runtime";
const RECEIPT_BUCKET = "openagentsgemini-livekit-deployment-receipts";
const DEPLOYER_SERVICE_ACCOUNT =
  "projects/openagentsgemini/serviceAccounts/oa-livekit-prod-deployer@openagentsgemini.iam.gserviceaccount.com";
const TERMINAL = new Set([
  "SUCCESS",
  "FAILURE",
  "INTERNAL_ERROR",
  "TIMEOUT",
  "CANCELLED",
  "EXPIRED",
]);
const BUILD_ID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

const usage = () => {
  process.stderr.write(`Usage:
  node scripts/cloud/livekit-production-deploy.mjs start [--timeout-seconds N]
  node scripts/cloud/livekit-production-deploy.mjs status --build-id UUID [--timeout-seconds N]
  node scripts/cloud/livekit-production-deploy.mjs retrieve --build-id UUID --receipt docs/ops/receipts/livekit/NAME.json

start invokes only the server-side oa-livekit-prod-runtime trigger at the exact
remote main revision. status is resumable by build id. retrieve copies and
validates the immutable public receipt after a successful build.
`);
};

const fail = (message) => {
  throw new Error(message);
};

export const parseArgs = (arguments_) => {
  const command = arguments_[0];
  if (!["start", "status", "retrieve"].includes(command)) fail("unsupported command");
  const parsed = { buildId: undefined, command, receipt: undefined, timeoutSeconds: 3600 };
  let timeoutProvided = false;
  for (let index = 1; index < arguments_.length; index += 1) {
    const argument = arguments_[index];
    const value = arguments_[index + 1];
    if (argument === "--build-id") parsed.buildId = value;
    else if (argument === "--receipt") parsed.receipt = value;
    else if (argument === "--timeout-seconds") {
      parsed.timeoutSeconds = Number(value);
      timeoutProvided = true;
    } else fail(`unsupported argument ${argument}`);
    index += 1;
  }
  if (
    !Number.isSafeInteger(parsed.timeoutSeconds) ||
    parsed.timeoutSeconds < 1 ||
    parsed.timeoutSeconds > 7200
  ) {
    fail("--timeout-seconds must be an integer from 1 through 7200");
  }
  if (command !== "start" && (!parsed.buildId || !BUILD_ID.test(parsed.buildId))) {
    fail("status and retrieve require a canonical Cloud Build id");
  }
  if (command === "start" && parsed.buildId) fail("start does not accept --build-id");
  if (command === "retrieve" && !parsed.receipt) fail("retrieve requires --receipt");
  if (command === "retrieve" && timeoutProvided) {
    fail("retrieve does not accept --timeout-seconds");
  }
  if (command !== "retrieve" && parsed.receipt) fail("--receipt is accepted only by retrieve");
  return parsed;
};

const run = (bin, args, label) => {
  const result = spawnSync(bin, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) fail(`${label} failed`);
  return result.stdout.trim();
};

const git = (...args) => run("git", args, `git ${args[0]}`);

export const isAdmittedSourceCheckout = ({ clean, head, remote }) =>
  clean && /^[0-9a-f]{40}$/u.test(head) && head === remote;

const remoteMainRevision = () => {
  const clean = git("status", "--porcelain") === "";
  const head = git("rev-parse", "HEAD");
  const revision = git("ls-remote", "--exit-code", "origin", "refs/heads/main").split(/\s+/u).at(0);
  if (!isAdmittedSourceCheckout({ clean, head, remote: revision })) {
    fail("start requires a clean checkout at current remote main");
  }
  return revision;
};

export const validateBuildDescription = (build, buildId, expectedRevision) => {
  const resolvedRevision = resolvedBuildRevision(build);
  if (
    build.id !== buildId ||
    build.serviceAccount !== DEPLOYER_SERVICE_ACCOUNT ||
    typeof build.buildTriggerId !== "string" ||
    build.buildTriggerId.length === 0 ||
    (expectedRevision !== undefined && resolvedRevision !== expectedRevision)
  ) {
    fail("Cloud Build returned a build outside the production deployment boundary");
  }
  return build;
};

export const resolvedBuildRevision = (build) =>
  build.sourceProvenance?.resolvedGitSource?.revision ??
  build.sourceProvenance?.resolvedRepoSource?.commitSha;

export const parseTriggeredBuildId = (value, expectedRevision) => {
  let operation;
  try {
    operation = JSON.parse(value);
  } catch {
    fail("trigger did not return readable Cloud Build operation JSON");
  }
  const build = operation?.metadata?.build;
  const buildId = build?.id;
  if (!BUILD_ID.test(buildId ?? "")) {
    fail("trigger did not return a canonical Cloud Build id");
  }
  validateBuildDescription(build, buildId, expectedRevision);
  return buildId;
};

const describeBuild = (buildId, expectedRevision) => {
  const value = run(
    "gcloud",
    [
      "builds",
      "describe",
      buildId,
      "--project",
      PROJECT,
      "--region",
      REGION,
      "--format=json(id,status,buildTriggerId,serviceAccount,sourceProvenance.resolvedGitSource.revision,sourceProvenance.resolvedRepoSource.commitSha)",
    ],
    "describe production deployment build",
  );
  const build = JSON.parse(value);
  return validateBuildDescription(build, buildId, expectedRevision);
};

const waitForBuild = (buildId, timeoutSeconds, expectedRevision) => {
  const deadline = Date.now() + timeoutSeconds * 1000;
  while (true) {
    const build = describeBuild(buildId, expectedRevision);
    if (TERMINAL.has(build.status)) {
      process.stdout.write(`${JSON.stringify(build)}\n`);
      if (build.status !== "SUCCESS") fail(`deployment build settled as ${build.status}`);
      return build;
    }
    if (Date.now() >= deadline) {
      process.stdout.write(
        `Build ${buildId} is still ${build.status}. Resume with:\n` +
          `node scripts/cloud/livekit-production-deploy.mjs status --build-id ${buildId}\n`,
      );
      return undefined;
    }
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 5000);
  }
};

export const triggerRunArguments = (revision) => {
  if (!/^[0-9a-f]{40}$/u.test(revision)) {
    fail("trigger revision must be a full lowercase Git commit");
  }
  return [
    "builds",
    "triggers",
    "run",
    TRIGGER,
    "--project",
    PROJECT,
    "--region",
    REGION,
    "--sha",
    revision,
    "--format=json",
  ];
};

export const receiptPath = (path) => {
  const absolute = resolve(path);
  const root = `${resolve("docs/ops/receipts/livekit")}${sep}`;
  if (!absolute.startsWith(root)) fail("receipt must stay under docs/ops/receipts/livekit");
  if (existsSync(absolute)) fail("receipt path already exists");
  return absolute;
};

export const receiptObjectUri = (buildId) => {
  if (!BUILD_ID.test(buildId)) fail("receipt object requires a canonical Cloud Build id");
  return `gs://${RECEIPT_BUCKET}/production-runtime/${buildId}/receipt.json`;
};

const retrieveReceipt = (buildId, path) => {
  const build = describeBuild(buildId);
  if (build.status !== "SUCCESS") fail("receipt retrieval requires a successful build");
  const sourceRevision = resolvedBuildRevision(build);
  if (!/^[0-9a-f]{40}$/u.test(sourceRevision ?? "")) {
    fail("successful build does not match the production deployment identity");
  }
  const target = receiptPath(path);
  const temporary = `${target}.download-${buildId}`;
  mkdirSync(dirname(target), { recursive: true });
  run(
    "gcloud",
    ["storage", "cp", receiptObjectUri(buildId), temporary, "--project", PROJECT],
    "retrieve production deployment receipt",
  );
  try {
    const receipt = validateSourceOnlyReceipt(JSON.parse(readFileSync(temporary, "utf8")));
    const expectedBuildRef = `gcp-cloud-build-ref://openagentsgemini/us-central1/${buildId}`;
    if (
      receipt?.execution?.buildRef !== expectedBuildRef ||
      receipt?.execution?.serviceAccountRef !==
        "gcp-service-account-ref://openagentsgemini/oa-livekit-prod-deployer" ||
      receipt?.execution?.triggerRef !==
        "gcp-cloud-build-trigger-ref://openagentsgemini/us-central1/oa-livekit-prod-runtime" ||
      receipt.execution.sourceRevision !== sourceRevision
    ) {
      fail("receipt provenance does not match the fixed production deployment boundary");
    }
    writeFileSync(target, `${JSON.stringify(receipt, null, 2)}\n`, {
      encoding: "utf8",
      flag: "wx",
      mode: 0o600,
    });
  } finally {
    if (existsSync(temporary)) unlinkSync(temporary);
  }
  process.stdout.write(`${target}\n`);
};

const main = () => {
  const args = parseArgs(process.argv.slice(2));
  if (args.command === "retrieve") {
    retrieveReceipt(args.buildId, args.receipt);
    return;
  }
  let buildId = args.buildId;
  let expectedRevision;
  if (args.command === "start") {
    const revision = remoteMainRevision();
    expectedRevision = revision;
    const operation = run(
      "gcloud",
      triggerRunArguments(revision),
      "start fixed production deployment trigger",
    );
    buildId = parseTriggeredBuildId(operation, revision);
    process.stdout.write(`Cloud Build id: ${buildId}\n`);
  }
  waitForBuild(buildId, args.timeoutSeconds, expectedRevision);
};

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
    usage();
    process.exitCode = 1;
  }
}
