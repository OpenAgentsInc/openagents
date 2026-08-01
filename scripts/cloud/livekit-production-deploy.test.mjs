import assert from "node:assert/strict";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import test from "node:test";
import {
  isAdmittedSourceCheckout,
  parseArgs,
  parseTriggeredBuildId,
  receiptPath,
  receiptObjectUri,
  resolvedBuildRevision,
  triggerRunArguments,
  validateBuildDescription,
} from "./livekit-production-deploy.mjs";

const buildId = "123e4567-e89b-42d3-a456-426614174000";
const revision = "1".repeat(40);

test("launcher admits any clean checkout at the exact remote main revision", () => {
  assert.equal(isAdmittedSourceCheckout({ clean: true, head: revision, remote: revision }), true);
  assert.equal(isAdmittedSourceCheckout({ clean: false, head: revision, remote: revision }), false);
  assert.equal(
    isAdmittedSourceCheckout({ clean: true, head: revision, remote: "2".repeat(40) }),
    false,
  );
  assert.equal(isAdmittedSourceCheckout({ clean: true, head: "main", remote: "main" }), false);
});

test("receipt retrieval uses Cloud Build's flattened artifact object", () => {
  assert.equal(
    receiptObjectUri("75767c7d-a6eb-4f0b-b259-a122de6a33cb"),
    "gs://openagentsgemini-livekit-deployment-receipts/production-runtime/75767c7d-a6eb-4f0b-b259-a122de6a33cb/receipt.json",
  );
});

test("launcher accepts only fixed start, resumable status, and receipt retrieval", () => {
  assert.deepEqual(parseArgs(["start"]), {
    buildId: undefined,
    command: "start",
    receipt: undefined,
    timeoutSeconds: 3600,
  });
  assert.equal(parseArgs(["status", "--build-id", buildId]).buildId, buildId);
  assert.equal(
    parseArgs([
      "retrieve",
      "--build-id",
      buildId,
      "--receipt",
      "docs/ops/receipts/livekit/result.json",
    ]).receipt,
    "docs/ops/receipts/livekit/result.json",
  );
  assert.throws(() => parseArgs(["start", "--config", "attacker.yaml"]), /unsupported/u);
  assert.throws(() => parseArgs(["start", "--build-id", buildId]), /does not accept --build-id/u);
  assert.throws(
    () => parseArgs(["status", "--build-id", "not-a-build"]),
    /canonical Cloud Build id/u,
  );
  assert.throws(
    () => parseArgs(["status", "--build-id", buildId, "--timeout-seconds", "7201"]),
    /1 through 7200/u,
  );
  assert.throws(
    () =>
      parseArgs([
        "retrieve",
        "--build-id",
        buildId,
        "--receipt",
        "docs/ops/receipts/livekit/result.json",
        "--timeout-seconds",
        "1",
      ]),
    /does not accept --timeout-seconds/u,
  );
});

test("launcher uses the current synchronous trigger result shape", () => {
  const source = readFileSync(
    resolve(import.meta.dirname, "livekit-production-deploy.mjs"),
    "utf8",
  );
  assert.doesNotMatch(source, /"--async"/u);
  assert.match(source, /"--format=json"/u);
});

test("launcher starts the fixed trigger at the exact observed main commit", () => {
  assert.deepEqual(triggerRunArguments(revision), [
    "builds",
    "triggers",
    "run",
    "oa-livekit-prod-runtime",
    "--project",
    "openagentsgemini",
    "--region",
    "us-central1",
    "--sha",
    revision,
    "--format=json",
  ]);
  assert.throws(() => triggerRunArguments("refs/heads/main"), /full lowercase Git commit/u);
});

test("launcher rejects a build that resolves away from the requested commit", () => {
  const build = {
    id: buildId,
    buildTriggerId: "trigger-id",
    serviceAccount:
      "projects/openagentsgemini/serviceAccounts/oa-livekit-prod-deployer@openagentsgemini.iam.gserviceaccount.com",
    sourceProvenance: { resolvedRepoSource: { commitSha: revision } },
  };
  assert.equal(validateBuildDescription(build, buildId, revision), build);
  assert.throws(
    () => validateBuildDescription(build, buildId, "2".repeat(40)),
    /outside the production deployment boundary/u,
  );
});

test("launcher accepts the Cloud Build v2 resolved Git source and operation envelope", () => {
  const build = {
    id: buildId,
    buildTriggerId: "trigger-id",
    serviceAccount:
      "projects/openagentsgemini/serviceAccounts/oa-livekit-prod-deployer@openagentsgemini.iam.gserviceaccount.com",
    sourceProvenance: { resolvedGitSource: { revision } },
  };
  assert.equal(resolvedBuildRevision(build), revision);
  assert.equal(parseTriggeredBuildId(JSON.stringify({ metadata: { build } }), revision), buildId);
  assert.throws(
    () => parseTriggeredBuildId(JSON.stringify({ metadata: { build } }), "2".repeat(40)),
    /outside the production deployment boundary/u,
  );
});

test("receipt retrieval path is exclusive and repository scoped", () => {
  const accepted = receiptPath("docs/ops/receipts/livekit/secure-deployment.json");
  assert.equal(accepted, resolve("docs/ops/receipts/livekit/secure-deployment.json"));
  const external = resolve(mkdtempSync(resolve(tmpdir(), "livekit-receipt-test-")), "receipt.json");
  assert.throws(() => receiptPath(external), /must stay under/u);
});
