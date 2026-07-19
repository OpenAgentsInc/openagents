#!/usr/bin/env -S pnpm exec tsx

/**
 * Owner-gated live component acceptance for SBX-02.
 *
 * This script calls the native oa-codex-control runtime route. It does not
 * deploy the service and it does not make a public availability claim. The
 * later SBX-09 gate owns independent end-to-end acceptance and rollout.
 */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

type Phase =
  | "provisioning"
  | "ready"
  | "stopping"
  | "stopped"
  | "resuming"
  | "failed"
  | "recovery_required"
  | "deleting"
  | "deleted";

interface RuntimeReceipt {
  readonly schemaVersion: string;
  readonly receiptRef: string;
  readonly operationRef: string;
  readonly action: "create" | "probe" | "stop" | "resume" | "delete" | "reconcile";
  readonly sandboxRef: string;
  readonly generation: number;
  readonly phase: Phase;
  readonly targetRef: string;
  readonly profileRef: string;
  readonly profileDigest: string;
  readonly imageRef: string;
  readonly imageDigest: string;
  readonly isolationClass: string;
  readonly networkPolicyRef: string;
  readonly controlIdentityRef: string;
  readonly guestIdentityRef: string;
  readonly resourceRef: string;
  readonly firewallRef: string;
  readonly diskRef: string;
  readonly providerKind: string;
  readonly readinessObserved: boolean;
  readonly cleanupObserved: boolean;
  readonly measuredRunningMs: number;
  readonly measuredCostMicrousd: number;
  readonly sandboxBudgetMicrousd: number;
  readonly programBudgetMicrousd: number;
  readonly emittedAtMs: number;
  readonly errorCode?: string;
}

function required(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`missing required environment variable ${name}`);
  return value;
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function parseArgs(): { apply: boolean; evidence: string } {
  const args = process.argv.slice(2);
  let apply = false;
  let evidence = resolve("artifacts/managed-sandbox-sbx02-live.json");
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--apply") {
      apply = true;
    } else if (argument === "--evidence" && args[index + 1]) {
      evidence = resolve(args[index + 1]!);
      index += 1;
    } else {
      throw new Error("usage: managed-sandbox-live-acceptance.ts --apply [--evidence PATH]");
    }
  }
  return { apply, evidence };
}

const { apply, evidence } = parseArgs();
if (!apply || process.env.OA_MANAGED_SANDBOX_OWNER_GATE !== "I_ACCEPT_LIVE_GCP_COST") {
  throw new Error(
    "live acceptance is default-off; pass --apply and set OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST",
  );
}

const baseUrl = required("OA_MANAGED_SANDBOX_BASE_URL").replace(/\/$/, "");
const token = required("OA_CODEX_CONTROL_TOKEN");
const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const stamp = `${Date.now()}-${process.pid}`;
const sandboxRef = `sandbox-ref://owner-live/sbx02-${sha256(stamp).slice(0, 20)}`;
const scope = {
  actorRef: "principal-ref://owner/sbx02-live-acceptance",
  ownerRef: "owner-ref://openagents/primary",
  tenantRef: "tenant-ref://openagents/primary",
  programRef: "program-ref://managed-agent-sandboxes/sbx02",
  workUnitRef: `work-unit-ref://managed-agent-sandboxes/${sha256(stamp).slice(0, 20)}`,
  sandboxRef,
};
const profile = {
  profileRef: required("OA_MANAGED_SANDBOX_PROFILE_REF"),
  profileDigest: required("OA_MANAGED_SANDBOX_PROFILE_DIGEST"),
  targetRef: "target://openagents/google-cloud/managed-sandbox",
  provisionerRef: required("OA_MANAGED_SANDBOX_PROVISIONER_REF"),
  region: required("OA_MANAGED_SANDBOX_REGION"),
  machineClass: required("OA_MANAGED_SANDBOX_MACHINE_CLASS"),
  isolationClass: "gce_vm",
  imageRef: `gce-image-ref://sha256/${required("OA_MANAGED_SANDBOX_IMAGE_DIGEST").replace("sha256:", "")}`,
  imageDigest: required("OA_MANAGED_SANDBOX_IMAGE_DIGEST"),
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
    sandboxBudgetMicrousd: 10_000,
    programBudgetMicrousd: 40_000,
    maxHourlyCostMicrousd: 20_000,
  },
  capabilityRefs: [`capability-ref://run/${sha256(`${stamp}|probe`).slice(0, 32)}`],
};

let generation = 0;
let phase: Phase | undefined;
let counter = 0;
let receipts: RuntimeReceipt[] = [];

async function operate(
  action: RuntimeReceipt["action"],
  expectedGeneration: number,
  includeProfile = false,
): Promise<RuntimeReceipt> {
  counter += 1;
  const operationId = sha256(`${stamp}|${counter}|${action}`).slice(0, 32);
  const request = {
    ...scope,
    operationRef: `operation-ref://sbx02-live/${operationId}`,
    idempotencyRef: `idempotency-ref://sbx02-live/${operationId}`,
    expectedGeneration,
    action,
    ...(includeProfile ? { profile } : {}),
  };
  const response = await fetch(`${baseUrl}/v1/managed-sandbox/runtime/operations`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(request),
  });
  const body = (await response.json()) as RuntimeReceipt | { error?: string; message?: string };
  if (!response.ok) {
    throw new Error(
      `managed-sandbox ${action} refused with ${response.status}: ${"error" in body ? body.error : "unknown"}`,
    );
  }
  const receipt = body as RuntimeReceipt;
  if (receipt.schemaVersion !== "openagents.managed_sandbox_runtime.v1") {
    throw new Error(`unexpected runtime receipt schema ${receipt.schemaVersion}`);
  }
  if (receipt.sandboxRef !== sandboxRef || receipt.providerKind !== "live_gce") {
    throw new Error("runtime receipt scope or provider identity mismatch");
  }
  const serialized = JSON.stringify(receipt).toLowerCase();
  for (const forbidden of [
    projectId.toLowerCase(),
    "serviceaccount.com",
    "access_token",
    "private key",
    "googleapis.com/compute",
  ]) {
    if (serialized.includes(forbidden)) {
      throw new Error(`runtime receipt leaked forbidden material: ${forbidden}`);
    }
  }
  receipts.push(receipt);
  generation = receipt.generation;
  phase = receipt.phase;
  return receipt;
}

function count(collection: "instances" | "firewall-rules" | "disks", filter: string): number {
  const args = ["compute", collection, "list", "--project", projectId];
  if (collection !== "firewall-rules") args.push("--zones", zone);
  args.push("--filter", filter, "--format", "value(name)");
  const output = execFileSync(gcloud, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
  return output.split("\n").filter((line) => line.trim().length > 0).length;
}

async function guaranteedCleanup(): Promise<void> {
  try {
    if (phase === "ready") await operate("stop", generation);
    if (phase && phase !== "deleted") {
      if (!["stopped", "failed", "recovery_required", "deleting"].includes(phase)) {
        await operate("reconcile", generation);
      }
      if (String(phase) !== "deleted") await operate("delete", generation);
    }
  } catch {
    try {
      await operate("reconcile", generation);
    } catch {
      // The final independent residue check still fails the acceptance.
    }
  }
}

let passed = false;
let failure: string | undefined;
try {
  const create = await operate("create", 0, true);
  if (create.phase !== "ready" || !create.readinessObserved) {
    throw new Error("create did not produce observed ready state");
  }
  const probe1 = await operate("probe", 1);
  if (!probe1.readinessObserved) throw new Error("bounded generation-1 probe was not observed");
  const stop1 = await operate("stop", 1);
  if (stop1.phase !== "stopped") throw new Error("stop did not settle as stopped");
  const resume = await operate("resume", 1);
  if (resume.phase !== "ready" || resume.generation !== 2 || !resume.readinessObserved) {
    throw new Error("resume did not produce observed generation-2 readiness");
  }
  const probe2 = await operate("probe", 2);
  if (!probe2.readinessObserved) throw new Error("bounded generation-2 probe was not observed");
  await operate("stop", 2);
  const deleted = await operate("delete", 2);
  if (deleted.phase !== "deleted" || !deleted.cleanupObserved) {
    throw new Error("delete did not produce observed cleanup");
  }
  if (deleted.measuredCostMicrousd > deleted.sandboxBudgetMicrousd) {
    throw new Error("measured incremental cost exceeded the sandbox budget");
  }
  passed = true;
} catch (error) {
  failure = error instanceof Error ? error.message : String(error);
} finally {
  await guaranteedCleanup();
}

const resourceSuffix = sha256(sandboxRef).slice(0, 20);
const firewallNames = [
  `oa-msb-egress-${resourceSuffix}`,
  `oa-msb-broker-${resourceSuffix}`,
  `oa-msb-ssh-${resourceSuffix}`,
  `oa-msb-ingress-${resourceSuffix}`,
];
const residue = {
  compute: count("instances", `name=oa-msb-${resourceSuffix}`),
  firewall: firewallNames.reduce(
    (total, name) => total + count("firewall-rules", `name=${name}`),
    0,
  ),
  scratch: count("disks", `name=oa-msb-${resourceSuffix}`),
  ingress: firewallNames
    .filter((name) => name.includes("-ssh-") || name.includes("-ingress-"))
    .reduce(
      (total, name) => total + count("firewall-rules", `name=${name}`),
      0,
    ),
  grants: 0,
};
if (Object.values(residue).some((value) => value !== 0)) {
  passed = false;
  failure = "independent GCP residue oracle found managed-sandbox resources";
}

const publicEvidence = {
  schemaVersion: "openagents.managed_sandbox_sbx02_live_acceptance.v1",
  capturedAt: new Date().toISOString(),
  passed,
  failure,
  sandboxRef,
  profile: {
    profileRef: profile.profileRef,
    profileDigest: profile.profileDigest,
    targetRef: profile.targetRef,
    region: profile.region,
    machineClass: profile.machineClass,
    isolationClass: profile.isolationClass,
    imageRef: profile.imageRef,
    imageDigest: profile.imageDigest,
    networkPolicyRef: profile.networkPolicyRef,
    controlIdentityRef: profile.controlIdentityRef,
    guestIdentityRef: profile.guestIdentityRef,
    capacity: profile.capacity,
    budget: profile.budget,
  },
  receiptRefs: receipts.map((receipt) => receipt.receiptRef),
  finalReceipt: receipts.at(-1),
  residue,
};
mkdirSync(dirname(evidence), { recursive: true });
writeFileSync(evidence, `${JSON.stringify(publicEvidence, null, 2)}\n`, { mode: 0o600 });
process.stdout.write(
  `${JSON.stringify({ passed, evidence, receiptCount: receipts.length, residue })}\n`,
);
if (!passed) {
  process.stderr.write(`${failure ?? "managed-sandbox live acceptance failed"}\n`);
  process.exit(1);
}
