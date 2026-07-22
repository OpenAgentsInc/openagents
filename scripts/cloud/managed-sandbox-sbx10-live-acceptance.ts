#!/usr/bin/env -S pnpm exec tsx

/**
 * Owner-gated SBX-10 acceptance on the admitted Google Cloud staging target.
 *
 * The output contains public-safe refs, digests, verdicts, and counts only.
 * It never records the control token, provider topology, or guest output.
 */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
type RuntimeReceipt = Readonly<{
  receiptRef: string;
  sandboxRef: string;
  generation: number;
  phase: string;
  measuredRunningMs: number;
  measuredCostMicrousd: number;
  cleanupObserved: boolean;
}>;

const required = (name: string): string => {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`missing required environment variable ${name}`);
  return value;
};
const sha256 = (value: string | Buffer): string => createHash("sha256").update(value).digest("hex");
const digest = (value: string | Buffer): string => `sha256:${sha256(value)}`;
const now = (): string => new Date().toISOString();

const argv = process.argv.slice(2);
let apply = false;
let evidence = resolve("artifacts/managed-sandbox-sbx10-live.json");
for (let index = 0; index < argv.length; index += 1) {
  if (argv[index] === "--apply") apply = true;
  else if (argv[index] === "--evidence" && argv[index + 1]) {
    evidence = resolve(argv[index + 1]!);
    index += 1;
  } else {
    throw new Error("usage: managed-sandbox-sbx10-live-acceptance.ts --apply [--evidence PATH]");
  }
}
if (!apply || process.env.OA_MANAGED_SANDBOX_OWNER_GATE !== "I_ACCEPT_LIVE_GCP_COST") {
  throw new Error(
    "live acceptance is default-off; pass --apply and set OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST",
  );
}

const baseUrl = required("OA_MANAGED_SANDBOX_BASE_URL").replace(/\/$/u, "");
const token = required("OA_CODEX_CONTROL_TOKEN");
const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const bucket = required("OA_MANAGED_SANDBOX_CHECKPOINT_BUCKET");
const sourceRevision = required("OA_MANAGED_SANDBOX_SOURCE_REVISION");
const deployedRevision = required("OA_MANAGED_SANDBOX_DEPLOYED_REVISION");
const imageDigest = required("OA_MANAGED_SANDBOX_IMAGE_DIGEST");
const toolchainDigest = required("OA_MANAGED_SANDBOX_TOOLCHAIN_DIGEST");
const profileDigest = required("OA_MANAGED_SANDBOX_PROFILE_DIGEST");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const stamp = `${Date.now()}-${process.pid}`;
const suffix = sha256(stamp).slice(0, 20);
const ownerRef = "owner.openagents.primary";
const tenantRef = "tenant.openagents.primary";
const actorRef = "principal.owner.sbx10-live-acceptance";
const audienceRef = "principal.owner.sbx10-live-preview";
const programRef = "program.managed-agent-sandboxes.sbx10";
const workUnitRef = `work.sbx10.${suffix}`;
const sourceSandboxRef = `sandbox.sbx10.source.${suffix}`;
const checkpointRef = `checkpoint.sbx10.${suffix}`;
const sourceCapabilityRef = `capability.sbx10.source.${sha256(`${stamp}|source`).slice(0, 32)}`;
const previewHtml =
  "<!doctype html><title>SBX-10 private preview</title><main>checkpoint fork verified</main>";
const previewPath = "/workspace/.openagents/preview.html";
const guestPreviewPath = "workspace/.openagents/preview.html";
const requestedAt = now();
const retainedUntil = new Date(Date.now() + 24 * 60 * 60 * 1_000).toISOString();
const capabilityExpiresAt = new Date(Date.now() + 15 * 60 * 1_000).toISOString();

const profile = {
  profileRef: required("OA_MANAGED_SANDBOX_PROFILE_REF"),
  profileDigest,
  targetRef: "target://openagents/google-cloud/managed-sandbox",
  provisionerRef: required("OA_MANAGED_SANDBOX_PROVISIONER_REF"),
  region: required("OA_MANAGED_SANDBOX_REGION"),
  machineClass: required("OA_MANAGED_SANDBOX_MACHINE_CLASS"),
  isolationClass: "gce_vm",
  imageRef: `gce-image-ref://sha256/${imageDigest.replace(/^sha256:/u, "")}`,
  imageDigest,
  networkPolicyRef: required("OA_MANAGED_SANDBOX_NETWORK_POLICY_REF"),
  controlIdentityRef: required("OA_MANAGED_SANDBOX_CONTROL_IDENTITY_REF"),
  guestIdentityRef: "identity-ref://openagents/managed-sandbox/guest-none",
  ttlMs: 15 * 60 * 1_000,
  capacity: {
    minCapacity: 0,
    maxCapacity: 2,
    prewarmCapacity: 0,
    concurrentCapacityCap: 2,
  },
  budget: {
    sandboxBudgetMicrousd: 20_000,
    programBudgetMicrousd: 50_000,
    maxHourlyCostMicrousd: 20_000,
  },
  capabilityRefs: [sourceCapabilityRef],
};

let sequence = 0;
const ref = (kind: string): string => {
  sequence += 1;
  return `${kind}.sbx10.${sha256(`${stamp}|${sequence}|${kind}`).slice(0, 32)}`;
};
const headers = {
  accept: "application/json",
  "cache-control": "no-store",
  "content-type": "application/json",
  "x-openagents-managed-sandbox-token": token,
};

const post = async <T>(path: string, body: Json, expected = 200): Promise<T> => {
  const response = await fetch(`${baseUrl}${path}`, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
  });
  const text = await response.text();
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch {
    throw new Error(`non-JSON response from ${path}`);
  }
  if (response.status !== expected) {
    const publicCode =
      typeof value === "object" && value !== null
        ? String(
            (value as Record<string, unknown>).reasonRef ??
              (value as Record<string, unknown>).code ??
              (value as Record<string, unknown>).error ??
              "unknown",
          )
        : "unknown";
    throw new Error(`${path} returned ${response.status}/${publicCode}; expected ${expected}`);
  }
  return value as T;
};

const runtimeReceipts: RuntimeReceipt[] = [];
const runtime = async (
  sandboxRef: string,
  action: "create" | "probe" | "stop" | "delete" | "reconcile",
  expectedGeneration: number,
  includeProfile = false,
): Promise<RuntimeReceipt> => {
  const operationRef = ref("operation");
  const receipt = await post<RuntimeReceipt>("/v1/managed-sandbox/runtime/operations", {
    actorRef,
    ownerRef,
    tenantRef,
    programRef,
    workUnitRef,
    sandboxRef,
    operationRef,
    idempotencyRef: ref("idempotency"),
    expectedGeneration,
    action,
    ...(includeProfile ? { profile } : {}),
  });
  if (receipt.sandboxRef !== sandboxRef) throw new Error("runtime receipt scope mismatch");
  runtimeReceipts.push(receipt);
  return receipt;
};

const phase2 = async <T>(
  action: string,
  requestRef: string,
  payload: Record<string, Json>,
  expected = 200,
): Promise<T> => {
  const envelope = await post<{ action: string; requestRef: string; result: T }>(
    "/v1/managed-sandbox/runtime/checkpoints",
    {
      schemaVersion: "openagents.managed_sandbox_phase2_target.v1",
      action,
      requestRef,
      ...payload,
    },
    expected,
  );
  if (envelope.action !== action || envelope.requestRef !== requestRef) {
    throw new Error("Phase 2 response scope mismatch");
  }
  return envelope.result;
};

const commandBase = (tag: string, commandRef: string) => ({
  _tag: tag,
  schema: "openagents.managed_sandbox_phase2_command.v1",
  commandRef,
  idempotencyRef: ref("idempotency"),
  ownerRef,
  tenantRef,
  requestedAt: now(),
});
const gcloudLines = (args: string[]): string[] =>
  execFileSync(gcloud, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] })
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
const ignoreFailure = (command: string, commandArgs: string[]) => {
  try {
    execFileSync(command, commandArgs, { stdio: "ignore" });
  } catch {
    // The final exact residue observation decides the verdict.
  }
};
const instanceName = (sandboxRef: string) => `oa-msb-${sha256(sandboxRef).slice(0, 20)}`;
const firewallNames = (sandboxRef: string) => {
  const id = sha256(sandboxRef).slice(0, 20);
  return ["egress", "broker", "metadata", "ssh", "ingress"].map((kind) => `oa-msb-${kind}-${id}`);
};
const checkpointUri =
  `gs://${bucket}/managed-sandbox-checkpoints/v1/` +
  `${sha256(`${ownerRef}|${tenantRef}`).slice(0, 32)}/${sha256(checkpointRef)}.tar`;
const checkpointObjectCount = (): number => {
  try {
    return gcloudLines(["storage", "ls", checkpointUri]).length;
  } catch {
    return 0;
  }
};
const inventory = () => {
  const sandboxes = [sourceSandboxRef, forkSandboxRef].filter(Boolean);
  const instances = sandboxes.reduce(
    (total, sandboxRef) =>
      total +
      gcloudLines([
        "compute",
        "instances",
        "list",
        "--project",
        projectId,
        "--zones",
        zone,
        "--filter",
        `name=${instanceName(sandboxRef)}`,
        "--format",
        "value(name)",
      ]).length,
    0,
  );
  const firewalls = sandboxes
    .flatMap(firewallNames)
    .reduce(
      (total, name) =>
        total +
        gcloudLines([
          "compute",
          "firewall-rules",
          "list",
          "--project",
          projectId,
          "--filter",
          `name=${name}`,
          "--format",
          "value(name)",
        ]).length,
      0,
    );
  const disks = sandboxes.reduce(
    (total, sandboxRef) =>
      total +
      gcloudLines([
        "compute",
        "disks",
        "list",
        "--project",
        projectId,
        "--zones",
        zone,
        "--filter",
        `name=${instanceName(sandboxRef)}`,
        "--format",
        "value(name)",
      ]).length,
    0,
  );
  const checkpointObjects = checkpointObjectCount();
  return { instances, firewalls, disks, checkpointObjects };
};

let sourceGeneration = 0;
let forkSandboxRef = "";
let forkGeneration = 0;
let checkpoint: Record<string, Json> | undefined;
let capability: Record<string, Json> | undefined;
const proof = {
  sourceReady: false,
  fixtureWritten: false,
  checkpointVerified: false,
  sourceStoppedAfterCheckpoint: false,
  forkFreshIdentity: false,
  forkFreshCapabilities: false,
  forkContentRestored: false,
  wrongAudienceDenied: false,
  privatePreviewRead: false,
  privatePreviewNoTopology: false,
  revokeEnforced: false,
  checkpointContentDeleted: false,
  zeroResidue: false,
};
let passed = false;
let failure: string | undefined;
let emergencyCleanupAttempted = false;
let before: ReturnType<typeof inventory> | undefined;
let after: ReturnType<typeof inventory> | undefined;

const safeRuntimeCleanup = async (sandboxRef: string, generation: number) => {
  if (!sandboxRef || generation < 1) return;
  try {
    const probe = await runtime(sandboxRef, "reconcile", generation);
    let current = probe;
    if (current.phase === "ready") current = await runtime(sandboxRef, "stop", current.generation);
    if (current.phase !== "deleted") await runtime(sandboxRef, "delete", current.generation);
  } catch {
    // The exact GCP residue oracle and emergency cleanup remain authoritative.
  }
};

try {
  before = inventory();
  if (Object.values(before).some((value) => value !== 0)) {
    throw new Error("exact SBX-10 acceptance identities were not clean before the run");
  }
  const created = await runtime(sourceSandboxRef, "create", 0, true);
  if (created.phase !== "ready" || created.generation !== 1) {
    throw new Error("source sandbox did not reach generation-1 readiness");
  }
  sourceGeneration = created.generation;
  proof.sourceReady = true;

  const writeOperationRef = ref("operation");
  const writeResponse = await post<Record<string, Json>>("/v1/managed-sandbox/runtime/io", {
    schemaVersion: "openagents.managed_sandbox_guest_io.v1",
    action: "write_file",
    operationRef: writeOperationRef,
    idempotencyRef: ref("idempotency"),
    actorRef,
    ownerRef,
    tenantRef,
    programRef,
    workUnitRef,
    sandboxRef: sourceSandboxRef,
    resourceGeneration: sourceGeneration,
    capabilityRef: sourceCapabilityRef,
    capabilityState: "active",
    capabilityExpiresAt,
    requestedAt,
    limits: {
      workspaceRootRef: "workspace.managed-sandbox",
      maxFileBytes: 1_048_576,
      maxArtifactBytes: 1_048_576,
      maxOutputBytes: 131_072,
      maxDurationMillis: 30_000,
      maxCpuMillis: 30_000,
      maxProcesses: 1,
      maxNetworkBytes: 0,
      networkPolicyRef: "network-policy.managed-sandbox.deny-all",
    },
    path: guestPreviewPath,
    encoding: "utf8",
    content: previewHtml,
    contentDigest: digest(previewHtml),
  });
  proof.fixtureWritten =
    writeResponse.action === "write_file" && writeResponse.contentDigest === digest(previewHtml);
  if (!proof.fixtureWritten) throw new Error("preview fixture write was not proven");

  const archiveCommandRef = ref("command");
  const archive = await phase2<Record<string, Json>>("archive_with_checkpoint", archiveCommandRef, {
    command: {
      ...commandBase("ArchiveWithCheckpoint", archiveCommandRef),
      checkpointRef,
      sourceSandboxRef,
      sourceResourceGeneration: sourceGeneration,
      sourceImageDigest: imageDigest,
      sourceToolchainDigest: toolchainDigest,
      repositoryRef: "repository.openagents",
      repositoryRevisionRef: `commit.${sourceRevision}`,
      repositoryPostImageDigest: digest(`${sourceRevision}|${previewHtml}`),
      formatRef: "format.sbx.content-tar.v1",
      retainedUntil,
      stopRef: ref("stop"),
    },
  });
  if (archive["_tag"] !== "Archived" || archive.archiveClaim !== "allowed") {
    throw new Error("archive did not return exact stopped checkpoint truth");
  }
  checkpoint = archive.checkpoint as Record<string, Json>;
  proof.sourceStoppedAfterCheckpoint = archive.lifecycle === "stopped";
  const verified = await phase2<Record<string, Json>>("verify_checkpoint", checkpointRef, {
    checkpoint,
  });
  proof.checkpointVerified =
    verified.verified === true && verified.contentDigest === checkpoint.contentDigest;
  if (!proof.checkpointVerified) throw new Error("checkpoint readback verification failed");

  const forkCommandRef = ref("command");
  const fork = await phase2<Record<string, Json>>("fork_from_checkpoint", forkCommandRef, {
    command: {
      ...commandBase("ForkFromCheckpoint", forkCommandRef),
      checkpointRef,
      expectedSourceSandboxRef: sourceSandboxRef,
      expectedSourceResourceGeneration: sourceGeneration,
      sourceCapabilityRefs: [sourceCapabilityRef],
    },
    checkpoint,
  });
  forkSandboxRef = String(fork.forkSandboxRef);
  forkGeneration = Number(fork.forkResourceGeneration);
  const forkCapabilities = fork.forkCapabilityRefs as Json[];
  proof.forkFreshIdentity = forkSandboxRef.length > 0 && forkSandboxRef !== sourceSandboxRef;
  proof.forkFreshCapabilities =
    Array.isArray(forkCapabilities) &&
    forkCapabilities.length > 0 &&
    !forkCapabilities.includes(sourceCapabilityRef);
  if (!proof.forkFreshIdentity || !proof.forkFreshCapabilities || forkGeneration !== 1) {
    throw new Error("fork did not produce a fresh generation-1 identity and grants");
  }

  const ingressCommandRef = ref("command");
  capability = await phase2<Record<string, Json>>("create_private_ingress", ingressCommandRef, {
    command: {
      ...commandBase("CreatePrivateIngress", ingressCommandRef),
      sandboxRef: forkSandboxRef,
      resourceGeneration: forkGeneration,
      audienceRef,
      kind: "preview",
      ttlSeconds: 300,
    },
  });
  const capabilityRef = String(capability.capabilityRef);
  const previewRequest = (audience: string, requestRef: string) => ({
    schemaVersion: "openagents.managed_sandbox_private_preview.v1",
    requestRef,
    capabilityRef,
    audienceRef: audience,
    path: previewPath,
    encoding: "utf8",
    capability,
  });
  await post(
    "/v1/managed-sandbox/runtime/private-preview",
    previewRequest("principal.owner.wrong-audience", ref("preview")),
    403,
  );
  proof.wrongAudienceDenied = true;
  const preview = await post<Record<string, Json>>(
    "/v1/managed-sandbox/runtime/private-preview",
    previewRequest(audienceRef, ref("preview")),
  );
  const serializedPreview = JSON.stringify(preview).toLowerCase();
  proof.privatePreviewRead =
    preview.sandboxRef === forkSandboxRef &&
    preview.resourceGeneration === forkGeneration &&
    JSON.stringify(preview).includes("checkpoint fork verified");
  proof.forkContentRestored = proof.privatePreviewRead;
  proof.privatePreviewNoTopology = ![
    projectId.toLowerCase(),
    "serviceaccount.com",
    "googleapis.com/compute",
    "metadata.google.internal",
    "natip",
    "networkip",
    "access_token",
    "private key",
  ].some((marker) => serializedPreview.includes(marker));
  if (!proof.privatePreviewRead || !proof.privatePreviewNoTopology) {
    throw new Error("private preview content or redaction proof failed");
  }

  const revokeCommandRef = ref("command");
  capability = await phase2<Record<string, Json>>("revoke_private_ingress", revokeCommandRef, {
    command: {
      ...commandBase("RevokePrivateIngress", revokeCommandRef),
      capabilityRef,
      sandboxRef: forkSandboxRef,
      resourceGeneration: forkGeneration,
    },
    capability,
  });
  await post(
    "/v1/managed-sandbox/runtime/private-preview",
    previewRequest(audienceRef, ref("preview")),
    403,
  );
  proof.revokeEnforced = true;

  await safeRuntimeCleanup(forkSandboxRef, forkGeneration);
  await safeRuntimeCleanup(sourceSandboxRef, sourceGeneration);
  const deleteCommandRef = ref("command");
  const deleted = await phase2<Record<string, Json>>("delete_checkpoint", deleteCommandRef, {
    command: {
      ...commandBase("DeleteCheckpoint", deleteCommandRef),
      checkpointRef,
      reason: "owner_requested",
    },
    checkpoint,
  });
  proof.checkpointContentDeleted = deleted.contentDeleted === true;
  after = inventory();
  proof.zeroResidue = Object.values(after).every((value) => value === 0);
  if (!proof.zeroResidue || !proof.checkpointContentDeleted) {
    throw new Error("exact Google Cloud or checkpoint residue remained after cleanup");
  }
  passed = Object.values(proof).every(Boolean);
} catch (error) {
  failure = error instanceof Error ? error.message : String(error);
} finally {
  if (capability?.["_tag"] === "Active" && forkSandboxRef) {
    try {
      const commandRef = ref("command");
      capability = await phase2<Record<string, Json>>("revoke_private_ingress", commandRef, {
        command: {
          ...commandBase("RevokePrivateIngress", commandRef),
          capabilityRef: String(capability.capabilityRef),
          sandboxRef: forkSandboxRef,
          resourceGeneration: forkGeneration,
        },
        capability,
      });
    } catch {
      // Exact cleanup and residue checks below remain authoritative.
    }
  }
  await safeRuntimeCleanup(forkSandboxRef, forkGeneration);
  await safeRuntimeCleanup(sourceSandboxRef, sourceGeneration);
  try {
    after = inventory();
  } catch {
    after = undefined;
  }
  if (after && Object.values(after).some((value) => value !== 0)) {
    emergencyCleanupAttempted = true;
    const exactSandboxes = [sourceSandboxRef, forkSandboxRef].filter(Boolean);
    for (const sandboxRef of exactSandboxes) {
      ignoreFailure(gcloud, [
        "compute",
        "instances",
        "delete",
        instanceName(sandboxRef),
        "--project",
        projectId,
        "--zone",
        zone,
        "--quiet",
      ]);
      ignoreFailure(gcloud, [
        "compute",
        "disks",
        "delete",
        instanceName(sandboxRef),
        "--project",
        projectId,
        "--zone",
        zone,
        "--quiet",
      ]);
      ignoreFailure(gcloud, [
        "compute",
        "firewall-rules",
        "delete",
        ...firewallNames(sandboxRef),
        "--project",
        projectId,
        "--quiet",
      ]);
    }
    ignoreFailure(gcloud, ["storage", "rm", checkpointUri]);
    try {
      after = inventory();
    } catch {
      after = undefined;
    }
  }
  if (!after || Object.values(after).some((value) => value !== 0)) {
    passed = false;
    failure = failure ?? "final exact residue observation failed";
  }
}

const totalMeasuredCostMicrousd = runtimeReceipts.reduce(
  (total, receipt) => total + receipt.measuredCostMicrousd,
  0,
);
const publicEvidence = {
  schemaVersion: "openagents.managed_sandbox_sbx10_live_acceptance.v1",
  capturedAt: now(),
  issue: 9032,
  environment: "staging",
  passed,
  failure,
  revisions: { source: sourceRevision, deployed: deployedRevision },
  target: {
    targetRef: profile.targetRef,
    profileRef: profile.profileRef,
    profileDigest,
    imageDigest,
    toolchainDigest,
    region: profile.region,
    machineClass: profile.machineClass,
    isolationClass: profile.isolationClass,
  },
  identityDigests: {
    sourceSandboxRef: digest(sourceSandboxRef),
    forkSandboxRef: forkSandboxRef ? digest(forkSandboxRef) : undefined,
    checkpointRef: digest(checkpointRef),
    audienceRef: digest(audienceRef),
  },
  receiptRefs: runtimeReceipts.map((receipt) => receipt.receiptRef),
  proof,
  cost: {
    totalMeasuredCostMicrousd,
    sandboxBudgetMicrousd: profile.budget.sandboxBudgetMicrousd,
    programBudgetMicrousd: profile.budget.programBudgetMicrousd,
  },
  before,
  after,
  emergencyCleanupAttempted,
  rollback: {
    productionChanged: false,
    publicAvailabilityClaimed: false,
    stagingDefaultOffGateRequired: true,
    exactResourceCleanupAttempted: true,
  },
  limitations: [
    "The checkpoint contains filesystem content only. It excludes credentials, memory, processes, sockets, ports, network identity, and provider hidden state.",
    "The preview route reads one bounded workspace file. It does not proxy a network service.",
    "Public or ungated VNC remains unsupported.",
    "This producer-run receipt does not satisfy the independent-review or owner-observation gates.",
  ],
};
mkdirSync(dirname(evidence), { recursive: true });
writeFileSync(evidence, `${JSON.stringify(publicEvidence, null, 2)}\n`, { mode: 0o600 });
process.stdout.write(
  `${JSON.stringify({ passed, evidence, proof, after, totalMeasuredCostMicrousd })}\n`,
);
if (!passed) process.exit(1);
