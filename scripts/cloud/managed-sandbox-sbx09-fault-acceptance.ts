#!/usr/bin/env -S pnpm exec tsx

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

type Action = "create" | "probe" | "stop" | "delete" | "reconcile";

type RuntimeReceipt = Readonly<{
  schemaVersion: string;
  receiptRef: string;
  sandboxRef: string;
  generation: number;
  phase: string;
  cleanupObserved: boolean;
  measuredRunningMs: number;
  measuredCostMicrousd: number;
}>;

type RuntimeErrorBody = Readonly<{
  schemaVersion?: string;
  error?: string;
  status?: string;
}>;

const required = (name: string): string => {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`missing required environment variable ${name}`);
  return value;
};

const sha256 = (value: string): string =>
  createHash("sha256").update(value).digest("hex");

const args = process.argv.slice(2);
let apply = false;
let evidence = resolve("artifacts/managed-sandbox-sbx09-fault-live.json");
for (let index = 0; index < args.length; index += 1) {
  if (args[index] === "--apply") {
    apply = true;
  } else if (args[index] === "--evidence" && args[index + 1]) {
    evidence = resolve(args[index + 1]!);
    index += 1;
  } else {
    throw new Error(
      "usage: managed-sandbox-sbx09-fault-acceptance.ts --apply [--evidence PATH]",
    );
  }
}
if (
  !apply ||
  process.env.OA_MANAGED_SANDBOX_OWNER_GATE !== "I_ACCEPT_LIVE_GCP_COST"
) {
  throw new Error(
    "live acceptance is default-off; pass --apply and set OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST",
  );
}

const baseUrl = required("OA_MANAGED_SANDBOX_BASE_URL").replace(/\/$/u, "");
const controlToken = required("OA_CODEX_CONTROL_TOKEN");
const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const sourceRevision = required("OA_MANAGED_SANDBOX_SOURCE_REVISION");
const deployedRevision = required("OA_MANAGED_SANDBOX_DEPLOYED_REVISION");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const stamp = `${Date.now()}-${process.pid}`;
const anchorSandboxRef = `sandbox-ref://owner-live/sbx09-fault-${sha256(stamp).slice(0, 20)}`;
const scope = {
  actorRef: "principal-ref://owner/sbx09-fault-acceptance",
  ownerRef: "owner-ref://openagents/primary",
  tenantRef: "tenant-ref://openagents/primary",
  programRef: "program-ref://managed-agent-sandboxes/sbx09",
  workUnitRef: `work-unit-ref://managed-agent-sandboxes/${sha256(stamp).slice(0, 20)}`,
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
    concurrentCapacityCap: 1,
  },
  budget: {
    sandboxBudgetMicrousd: 10_000,
    programBudgetMicrousd: 40_000,
    maxHourlyCostMicrousd: 20_000,
  },
  capabilityRefs: [
    `capability-ref://run/${sha256(`${stamp}|anchor`).slice(0, 32)}`,
  ],
};

const gcloudJson = (args: ReadonlyArray<string>): unknown =>
  JSON.parse(
    execFileSync(gcloud, [...args, "--format=json"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }),
  );

const inventory = () => {
  const instances = gcloudJson([
    "compute",
    "instances",
    "list",
    "--project",
    projectId,
    "--filter",
    "name~^oa-msb-",
  ]) as ReadonlyArray<Record<string, unknown>>;
  const disks = gcloudJson([
    "compute",
    "disks",
    "list",
    "--project",
    projectId,
    "--filter",
    "name~^oa-msb-",
  ]) as ReadonlyArray<Record<string, unknown>>;
  const firewalls = gcloudJson([
    "compute",
    "firewall-rules",
    "list",
    "--project",
    projectId,
    "--filter",
    "name~^oa-msb-",
  ]) as ReadonlyArray<Record<string, unknown>>;
  const serviceAccounts = gcloudJson([
    "iam",
    "service-accounts",
    "list",
    "--project",
    projectId,
  ]) as ReadonlyArray<Record<string, unknown>>;
  const images = gcloudJson([
    "compute",
    "images",
    "list",
    "--project",
    projectId,
    "--filter",
    "name~^oa-msb-guest-",
  ]) as ReadonlyArray<Record<string, unknown>>;
  const digestOf = (value: unknown) =>
    `sha256:${sha256(JSON.stringify(value))}`;
  return {
    managedInstances: instances.length,
    managedDisks: disks.length,
    managedFirewalls: firewalls.length,
    managedExternalIps: instances.filter((instance) =>
      JSON.stringify(instance).includes('"natIP"'),
    ).length,
    managedServiceIdentityGrants: 0,
    managedCapabilityGrants: 0,
    retainedGuestImages: images.length,
    serviceAccountInventoryDigest: digestOf(serviceAccounts),
    relevantIamMutationSurface: "none_runtime_has_no_iam_mutation_path",
    retainedGuestImageInventoryDigest: digestOf(images),
  };
};

let sequence = 0;
const requestFor = (
  sandboxRef: string,
  action: Action,
  expectedGeneration: number,
  options: Readonly<{
    requestProfile?: typeof profile;
    operationRef?: string;
    idempotencyRef?: string;
    requestScope?: typeof scope;
  }> = {},
) => {
  sequence += 1;
  const suffix = sha256(`${stamp}|${sequence}|${action}|${sandboxRef}`).slice(
    0,
    32,
  );
  return {
    ...(options.requestScope ?? scope),
    sandboxRef,
    operationRef:
      options.operationRef ?? `operation-ref://sbx09-fault/${suffix}`,
    idempotencyRef:
      options.idempotencyRef ?? `idempotency-ref://sbx09-fault/${suffix}`,
    expectedGeneration,
    action,
    ...(options.requestProfile === undefined
      ? {}
      : { profile: options.requestProfile }),
  };
};

const post = async (
  body: unknown,
): Promise<{ status: number; body: RuntimeReceipt | RuntimeErrorBody }> => {
  const response = await fetch(
    `${baseUrl}/v1/managed-sandbox/runtime/operations`,
    {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-openagents-managed-sandbox-token": controlToken,
      },
      body: JSON.stringify(body),
    },
  );
  return {
    status: response.status,
    body: (await response.json()) as RuntimeReceipt | RuntimeErrorBody,
  };
};

const expectError = async (
  body: unknown,
  status: number,
  code: string,
): Promise<void> => {
  const response = await post(body);
  if (
    response.status !== status ||
    (response.body as RuntimeErrorBody).error !== code
  ) {
    throw new Error(
      `expected ${status}/${code}, received ${response.status}/${String((response.body as RuntimeErrorBody).error)}`,
    );
  }
};

const proof = {
  ttlRefusal: false,
  budgetRefusal: false,
  capabilityQuotaRefusal: false,
  lostAcknowledgementReplay: false,
  duplicateReplay: false,
  idempotencyDriftRefusal: false,
  hardCapacityRefusal: false,
  programBudgetRefusal: false,
  ownerTenantIsolation: false,
  generationIsolation: false,
  cleanup: false,
};
const before = inventory();
let anchorPhase: string | undefined;
let anchorGeneration = 0;
let finalReceipt: RuntimeReceipt | undefined;
let passed = false;
let failure: string | undefined;

try {
  await expectError(
    requestFor(
      `sandbox-ref://owner-live/ttl-${sha256(stamp).slice(0, 16)}`,
      "create",
      0,
      {
        requestProfile: { ...profile, ttlMs: 86_400_001 },
      },
    ),
    400,
    "ttl_out_of_bounds",
  );
  proof.ttlRefusal = true;

  await expectError(
    requestFor(
      `sandbox-ref://owner-live/budget-${sha256(stamp).slice(0, 16)}`,
      "create",
      0,
      {
        requestProfile: {
          ...profile,
          budget: { ...profile.budget, sandboxBudgetMicrousd: 1_000 },
        },
      },
    ),
    409,
    "budget_refused",
  );
  proof.budgetRefusal = true;

  await expectError(
    requestFor(
      `sandbox-ref://owner-live/quota-${sha256(stamp).slice(0, 16)}`,
      "create",
      0,
      {
        requestProfile: {
          ...profile,
          capabilityRefs: Array.from(
            { length: 65 },
            (_, index) =>
              `capability-ref://run/${sha256(`${stamp}|${index}`).slice(0, 32)}`,
          ),
        },
      },
    ),
    400,
    "capability_ref_count_out_of_bounds",
  );
  proof.capabilityQuotaRefusal = true;

  const create = requestFor(anchorSandboxRef, "create", 0, {
    requestProfile: profile,
  });
  const discarded = await post(create);
  if (discarded.status !== 200)
    throw new Error("anchor create was not admitted");
  const replay = await post(create);
  if (replay.status !== 200)
    throw new Error("lost acknowledgement replay failed");
  const replayReceipt = replay.body as RuntimeReceipt;
  if (replayReceipt.phase !== "ready" || !replayReceipt.receiptRef) {
    throw new Error("anchor replay did not return exact readiness");
  }
  anchorPhase = replayReceipt.phase;
  anchorGeneration = replayReceipt.generation;
  proof.lostAcknowledgementReplay = true;
  const duplicate = await post(create);
  if (
    duplicate.status !== 200 ||
    (duplicate.body as RuntimeReceipt).receiptRef !== replayReceipt.receiptRef
  ) {
    throw new Error("exact duplicate did not replay the same receipt");
  }
  proof.duplicateReplay = true;

  await expectError(
    {
      ...create,
      profile: {
        ...profile,
        capabilityRefs: [
          `capability-ref://run/${sha256(`${stamp}|drift`).slice(0, 32)}`,
        ],
      },
    },
    409,
    "idempotency_conflict",
  );
  proof.idempotencyDriftRefusal = true;

  await expectError(
    requestFor(
      `sandbox-ref://owner-live/capacity-${sha256(stamp).slice(0, 16)}`,
      "create",
      0,
      { requestProfile: profile },
    ),
    409,
    "capacity_refused",
  );
  proof.hardCapacityRefusal = true;

  await expectError(
    requestFor(
      `sandbox-ref://owner-live/program-budget-${sha256(stamp).slice(0, 16)}`,
      "create",
      0,
      {
        requestProfile: {
          ...profile,
          capacity: { ...profile.capacity, concurrentCapacityCap: 2 },
          budget: {
            ...profile.budget,
            sandboxBudgetMicrousd: 5_000,
            programBudgetMicrousd: 5_000,
          },
        },
      },
    ),
    409,
    "program_budget_refused",
  );
  proof.programBudgetRefusal = true;

  await expectError(
    requestFor(anchorSandboxRef, "probe", 1, {
      requestScope: {
        ...scope,
        ownerRef: "owner-ref://openagents/foreign",
        tenantRef: "tenant-ref://openagents/foreign",
      },
    }),
    403,
    "scope_mismatch",
  );
  proof.ownerTenantIsolation = true;

  await expectError(
    requestFor(anchorSandboxRef, "probe", 2),
    409,
    "generation_conflict",
  );
  proof.generationIsolation = true;

  const stopped = await post(requestFor(anchorSandboxRef, "stop", 1));
  if (
    stopped.status !== 200 ||
    (stopped.body as RuntimeReceipt).phase !== "stopped"
  ) {
    throw new Error("anchor stop did not settle");
  }
  anchorPhase = "stopped";
  const deleted = await post(requestFor(anchorSandboxRef, "delete", 1));
  finalReceipt = deleted.body as RuntimeReceipt;
  if (
    deleted.status !== 200 ||
    finalReceipt.phase !== "deleted" ||
    !finalReceipt.cleanupObserved
  ) {
    throw new Error("anchor delete did not prove cleanup");
  }
  anchorPhase = "deleted";
  proof.cleanup = true;
  passed = Object.values(proof).every(Boolean);
} catch (error) {
  failure = error instanceof Error ? error.message : String(error);
} finally {
  if (anchorPhase !== undefined && anchorPhase !== "deleted") {
    try {
      if (anchorPhase === "ready") {
        const stopped = await post(
          requestFor(anchorSandboxRef, "stop", anchorGeneration || 1),
        );
        anchorPhase = (stopped.body as RuntimeReceipt).phase;
      }
      const deleted = await post(
        requestFor(anchorSandboxRef, "delete", anchorGeneration || 1),
      );
      anchorPhase = (deleted.body as RuntimeReceipt).phase;
    } catch {
      try {
        await post(
          requestFor(anchorSandboxRef, "reconcile", anchorGeneration || 1),
        );
      } catch {
        // The independent inventory below remains authoritative.
      }
    }
  }
}

const after = inventory();
const resourceSuffix = sha256(anchorSandboxRef).slice(0, 20);
const residue = {
  compute: Number(
    execFileSync(
      gcloud,
      [
        "compute",
        "instances",
        "list",
        "--project",
        projectId,
        "--zones",
        zone,
        "--filter",
        `name=oa-msb-${resourceSuffix}`,
        "--format=value(name)",
      ],
      { encoding: "utf8" },
    ).trim().length > 0,
  ),
  disk: Number(
    execFileSync(
      gcloud,
      [
        "compute",
        "disks",
        "list",
        "--project",
        projectId,
        "--zones",
        zone,
        "--filter",
        `name=oa-msb-${resourceSuffix}`,
        "--format=value(name)",
      ],
      { encoding: "utf8" },
    ).trim().length > 0,
  ),
  firewall: (
    gcloudJson([
      "compute",
      "firewall-rules",
      "list",
      "--project",
      projectId,
      "--filter",
      `name~${resourceSuffix}`,
    ]) as ReadonlyArray<unknown>
  ).length,
};
if (Object.values(residue).some((value) => value !== 0)) {
  passed = false;
  failure ??= "exact native fault resource left GCP residue";
}

const publicEvidence = {
  schemaVersion: "openagents.managed_sandbox_sbx09_fault_acceptance.v1",
  capturedAt: new Date().toISOString(),
  environment: "staging",
  sourceRevision,
  deployedRevision,
  passed,
  ...(failure === undefined ? {} : { failure }),
  profile: {
    profileRef: profile.profileRef,
    profileDigest: profile.profileDigest,
    imageDigest: profile.imageDigest,
    provisionerRef: profile.provisionerRef,
    region: profile.region,
    machineClass: profile.machineClass,
    networkPolicyRef: profile.networkPolicyRef,
  },
  sandboxRefDigest: `sha256:${sha256(anchorSandboxRef)}`,
  proof,
  finalReceipt: finalReceipt
    ? {
        receiptRef: finalReceipt.receiptRef,
        generation: finalReceipt.generation,
        phase: finalReceipt.phase,
        cleanupObserved: finalReceipt.cleanupObserved,
        measuredRunningMs: finalReceipt.measuredRunningMs,
        measuredCostMicrousd: finalReceipt.measuredCostMicrousd,
      }
    : null,
  before,
  after,
  residue,
};
mkdirSync(dirname(evidence), { recursive: true });
writeFileSync(evidence, `${JSON.stringify(publicEvidence, null, 2)}\n`, {
  mode: 0o600,
});
process.stdout.write(`${JSON.stringify({ passed, evidence, residue })}\n`);
if (!passed) {
  process.stderr.write(`${failure ?? "SBX-09 fault acceptance failed"}\n`);
  process.exit(1);
}
