#!/usr/bin/env -S pnpm exec tsx

/**
 * Owner-gated live Coldcard artifact-witness run (openagents #9296, OFR-014).
 *
 * For one build variant this script provisions a live `live_gce` managed
 * sandbox on the admitted profile, runs the pinned Coldcard MK4 firmware build
 * inside the guest, reads the capture the guest produced, stamps it with the
 * provenance of that exact run, and deletes the sandbox.
 *
 * The provenance stamp is applied HERE and not in the guest, because a guest
 * cannot attest to its own admission. The values written into
 * `provenance` come only from managed-sandbox runtime receipts observed by
 * this process: the sandbox ref it created, the generation the control plane
 * reported, the guest image digest the control plane admitted, and the receipt
 * refs the control plane emitted. If any of those disagree with the profile
 * this script requested, it refuses rather than writing a capture.
 */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const CAPTURE_SCHEMA = "openagents.artifact_witness_capture.v2";
const GUEST_IO_SCHEMA = "openagents.managed_sandbox_guest_io.v1";
const GUEST_DRIVER = "/opt/openagents-managed-sandbox/coldcard-build-driver.mjs";

const PINS = {
  vulnerable: {
    commitSha: "bcc2c382a324690a2fcf972c0bac3b79bf923f7b",
    gitTreeSha: "7abc9a4c680b5623fc8a64f70555dd2d3802e488",
  },
  fixed: {
    commitSha: "ca72463709f4e3f8964952039d5caf955f566a87",
    gitTreeSha: "51f735fb2dd888865c81ea2ed1fc690a0ed5b396",
  },
  fault_build: {
    commitSha: "ca72463709f4e3f8964952039d5caf955f566a87",
    gitTreeSha: "51f735fb2dd888865c81ea2ed1fc690a0ed5b396",
  },
} as const;

type Variant = keyof typeof PINS;

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
  readonly action: "create" | "probe" | "stop" | "resume" | "delete" | "reconcile";
  readonly sandboxRef: string;
  readonly generation: number;
  readonly phase: Phase;
  readonly profileDigest: string;
  readonly imageDigest: string;
  readonly isolationClass: string;
  readonly providerKind: string;
  readonly readinessObserved: boolean;
  readonly cleanupObserved: boolean;
  readonly measuredCostMicrousd: number;
  readonly sandboxBudgetMicrousd: number;
}

const sha256 = (value: string | Buffer): string => createHash("sha256").update(value).digest("hex");

function required(name: string): string {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`missing required environment variable ${name}`);
  return value;
}

function parseArgs(): { apply: boolean; variant: Variant; out: string } {
  const args = process.argv.slice(2);
  const option = (name: string): string | undefined => {
    const index = args.indexOf(`--${name}`);
    return index >= 0 ? args[index + 1] : undefined;
  };
  const variant = option("variant");
  if (variant !== "vulnerable" && variant !== "fixed" && variant !== "fault_build") {
    throw new Error("--variant must be vulnerable, fixed or fault_build");
  }
  return {
    apply: args.includes("--apply"),
    variant,
    out: resolve(option("out") ?? `artifacts/coldcard-artifact-witness-${variant}.json`),
  };
}

const { apply, variant, out } = parseArgs();
if (!apply || process.env.OA_MANAGED_SANDBOX_OWNER_GATE !== "I_ACCEPT_LIVE_GCP_COST") {
  throw new Error(
    "live artifact-witness runs are default-off; pass --apply and set OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST",
  );
}

const baseUrl = required("OA_MANAGED_SANDBOX_BASE_URL").replace(/\/$/, "");
const token = required("OA_CODEX_CONTROL_TOKEN");
const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const imageDigest = required("OA_MANAGED_SANDBOX_IMAGE_DIGEST");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const buildTimeoutMillis = Number(process.env.OA_COLDCARD_BUILD_TIMEOUT_MS ?? 3_000_000);

const stamp = `${Date.now()}-${process.pid}-${variant}`;
const suffix = sha256(stamp).slice(0, 20);
// The guest I/O route validates public refs more strictly than the runtime
// operations route: it rejects `/`. These dot-separated refs are the form that
// both routes accept, so one sandbox identity can carry the whole run.
const sandboxRef = `sandbox.ofr014.${variant.replace(/_/g, "-")}.${suffix}`;
const capabilityRef = `capability-ref://run/${sha256(`${stamp}|run`).slice(0, 32)}`;
const scope = {
  actorRef: "principal.owner.ofr014-artifact-witness",
  ownerRef: "owner.openagents.primary",
  tenantRef: "tenant.openagents.primary",
  programRef: "program.managed-agent-sandboxes.ofr014",
  workUnitRef: `work.ofr014.${suffix}`,
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
  imageRef: `gce-image-ref://sha256/${imageDigest.replace("sha256:", "")}`,
  imageDigest,
  networkPolicyRef: required("OA_MANAGED_SANDBOX_NETWORK_POLICY_REF"),
  controlIdentityRef: required("OA_MANAGED_SANDBOX_CONTROL_IDENTITY_REF"),
  guestIdentityRef: "identity-ref://openagents/managed-sandbox/guest-none",
  // The build itself takes minutes, so the sandbox must outlive it. The budget
  // below is the cost ceiling that makes that TTL affordable, not a target.
  ttlMs: 60 * 60 * 1_000,
  capacity: { minCapacity: 0, maxCapacity: 2, prewarmCapacity: 0, concurrentCapacityCap: 2 },
  budget: {
    sandboxBudgetMicrousd: 40_000,
    programBudgetMicrousd: 160_000,
    maxHourlyCostMicrousd: 40_000,
  },
  capabilityRefs: [capabilityRef],
};

let generation = 0;
let phase: Phase | undefined;
let counter = 0;
const receipts: RuntimeReceipt[] = [];

async function operate(
  action: RuntimeReceipt["action"],
  expectedGeneration: number,
  includeProfile = false,
): Promise<RuntimeReceipt> {
  counter += 1;
  const operationId = sha256(`${stamp}|${counter}|${action}`).slice(0, 32);
  const response = await fetch(`${baseUrl}/v1/managed-sandbox/runtime/operations`, {
    method: "POST",
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: JSON.stringify({
      ...scope,
      operationRef: `operation.ofr014-live.${operationId}`,
      idempotencyRef: `idempotency.ofr014-live.${operationId}`,
      expectedGeneration,
      action,
      ...(includeProfile ? { profile } : {}),
    }),
  });
  const body = (await response.json()) as RuntimeReceipt & { error?: string };
  if (!response.ok) {
    throw new Error(`managed-sandbox ${action} refused with ${response.status}: ${body.error}`);
  }
  if (body.schemaVersion !== "openagents.managed_sandbox_runtime.v1") {
    throw new Error(`unexpected runtime receipt schema ${body.schemaVersion}`);
  }
  if (body.sandboxRef !== sandboxRef || body.providerKind !== "live_gce") {
    throw new Error("runtime receipt scope or provider identity mismatch");
  }
  const serialized = JSON.stringify(body).toLowerCase();
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
  receipts.push(body);
  generation = body.generation;
  phase = body.phase;
  return body;
}

let ioCounter = 0;

async function guestIo(request: Record<string, unknown>): Promise<Record<string, unknown>> {
  ioCounter += 1;
  const operationId = sha256(`${stamp}|io|${ioCounter}`).slice(0, 32);
  const now = new Date();
  process.stderr.write(`[io] ${String(request.action)} start ${new Date().toISOString()}\n`);
  const response = await fetch(`${baseUrl}/v1/managed-sandbox/runtime/io`, {
    method: "POST",
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    // A guest build runs for minutes behind an IAP tunnel, so the client must
    // not impose a shorter deadline than the operation it is waiting for.
    signal: AbortSignal.timeout(buildTimeoutMillis + 600_000),
    body: JSON.stringify({
      schemaVersion: GUEST_IO_SCHEMA,
      ...scope,
      operationRef: `operation.ofr014-io.${operationId}`,
      idempotencyRef: `idempotency.ofr014-io.${operationId}`,
      resourceGeneration: generation,
      capabilityRef,
      capabilityState: "active",
      capabilityExpiresAt: new Date(now.getTime() + 3_600_000).toISOString(),
      requestedAt: now.toISOString(),
      limits: {
        workspaceRootRef: "workspace.managed-sandbox",
        maxFileBytes: 1_048_576,
        maxArtifactBytes: 16_777_216,
        maxOutputBytes: 262_144,
        maxDurationMillis: 3_600_000,
        maxCpuMillis: 3_600_000,
        maxProcesses: 64,
        maxNetworkBytes: 0,
        networkPolicyRef: "network-policy.managed-sandbox.deny-all",
      },
      ...request,
    }),
  }).catch((error: unknown) => {
    throw new Error(
      `guest io ${String(request.action)} transport failed: ${error instanceof Error ? error.message : String(error)}`,
    );
  });
  const body = (await response.json()) as Record<string, unknown>;
  if (!response.ok) {
    throw new Error(
      `guest io ${String(request.action)} refused with ${response.status}: ${JSON.stringify(body)}`,
    );
  }
  process.stderr.write(`[io] ${String(request.action)} done ${new Date().toISOString()}\n`);
  return body;
}

/**
 * The bundle exists to declare, inside the guest, exactly which pinned revision
 * this run claims to build, and to make the guest create the writable scratch
 * the build needs. It is not the source tree: the pinned trees are baked into
 * the admitted guest image, because the guest has no network.
 */
function sourceBundle(): { base64: string; digest: string } {
  const manifest = [
    `# Coldcard artifact-witness run`,
    `variant: ${variant}`,
    `commit: ${PINS[variant].commitSha}`,
    `tree: ${PINS[variant].gitTreeSha}`,
    `driver: ${GUEST_DRIVER}`,
    `guestImageDigest: ${imageDigest}`,
    ``,
  ].join("\n");
  // The guest re-derives this digest from the installed tree using its own
  // canonical JSON form (sorted keys, no separators padding). Serializing any
  // other way makes the post-copy verification fail even though the bytes on
  // disk are correct.
  const canonical = (value: unknown): string =>
    Array.isArray(value)
      ? `[${value.map(canonical).join(",")}]`
      : value !== null && typeof value === "object"
        ? `{${Object.keys(value as Record<string, unknown>)
            .sort()
            .map(
              (key) =>
                `${JSON.stringify(key)}:${canonical((value as Record<string, unknown>)[key])}`,
            )
            .join(",")}}`
        : JSON.stringify(value);
  const payload = canonical({
    schema: "openagents.forensic_source_bundle_payload.v2",
    repositoryRef: "repository.github.Coldcard.firmware",
    commitSha: PINS[variant].commitSha,
    gitTreeSha: PINS[variant].gitTreeSha,
    entries: [
      {
        path: "RUN.md",
        contentDigest: `sha256:${sha256(manifest)}`,
        contentBase64: Buffer.from(manifest, "utf8").toString("base64"),
      },
    ],
  });
  return {
    base64: Buffer.from(payload, "utf8").toString("base64"),
    digest: `sha256:${sha256(payload)}`,
  };
}

function count(collection: "instances" | "firewall-rules" | "disks", filter: string): number {
  const args = ["compute", collection, "list", "--project", projectId];
  if (collection !== "firewall-rules") args.push("--zones", zone);
  args.push("--filter", filter, "--format", "value(name)");
  const output = execFileSync(gcloud, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  return output.split("\n").filter((line) => line.trim().length > 0).length;
}

async function guaranteedCleanup(): Promise<void> {
  try {
    if (!phase) await operate("reconcile", 1);
    if (phase === "ready") await operate("stop", generation);
    if (phase && phase !== "deleted") {
      if (!["stopped", "failed", "recovery_required", "deleting"].includes(phase)) {
        await operate("reconcile", generation);
      }
      if (String(phase) !== "deleted") await operate("delete", generation);
    }
  } catch {
    try {
      await operate("reconcile", Math.max(generation, 1));
    } catch {
      // The independent residue check below still fails the run.
    }
  }
}

let passed = false;
let failure: string | undefined;
let capture: Record<string, unknown> | undefined;
let driverSummary: unknown;

try {
  const create = await operate("create", 0, true);
  if (create.phase !== "ready" || !create.readinessObserved) {
    throw new Error("create did not produce observed ready state");
  }
  if (create.imageDigest !== imageDigest) {
    throw new Error("control plane admitted a different guest image than requested");
  }
  if (create.isolationClass !== "gce_vm") {
    throw new Error(`unexpected isolation class ${create.isolationClass}`);
  }

  const bundle = sourceBundle();
  await guestIo({
    action: "install_forensic_source",
    artifactRef: `artifact.ofr014.${sha256(bundle.base64).slice(0, 32)}`,
    artifactContentBase64: bundle.base64,
    artifactContentDigest: bundle.digest,
    sourcePath: "workspace/source",
    scratchPath: "workspace/scratch",
  });

  // The workspace scratch directory is the only writable path with real disk
  // behind it: the per-command scratch is a tmpfs and cannot hold a firmware
  // build tree, and it is destroyed when the command returns.
  const command = [
    `OA_COLDCARD_WORKER_PROFILE_DIGEST=${create.profileDigest}`,
    `OA_COLDCARD_PLACEMENT_REF=placement.coldcard.${sha256(sandboxRef).slice(0, 20)}`,
    "node",
    GUEST_DRIVER,
    "--variant",
    variant,
    "--workdir",
    "/workspace/scratch/build",
    "--out",
    "/workspace/scratch/capture.json",
  ].join(" ");
  const executed = await guestIo({
    action: "execute_command",
    command,
    commandDigest: `sha256:${sha256(command)}`,
    cwd: "workspace",
    timeoutMillis: buildTimeoutMillis,
  });
  if (executed.timedOut === true) throw new Error("guest build timed out");
  if (executed.exitCode !== 0) {
    throw new Error(
      `coldcard build driver exited ${String(executed.exitCode)}: ${String(executed.stderr).slice(0, 400)}`,
    );
  }
  driverSummary = JSON.parse(String(executed.stdout).trim().split("\n").at(-1) ?? "{}");

  const artifact = await guestIo({
    action: "read_artifact",
    path: "workspace/scratch/capture.json",
    retentionUntil: new Date(Date.now() + 86_400_000).toISOString(),
  });
  const bytes = Buffer.from(String(artifact.contentBase64), "base64");
  capture = JSON.parse(bytes.toString("utf8")) as Record<string, unknown>;
  if (capture.schema !== CAPTURE_SCHEMA) {
    throw new Error(`guest produced an unexpected capture schema ${String(capture.schema)}`);
  }
  if (capture.variant !== variant) throw new Error("guest capture variant mismatch");
  if (capture.targetCommit !== PINS[variant].commitSha) {
    throw new Error("guest capture did not build the pinned commit");
  }

  await guestIo({
    action: "remove_forensic_source",
    expectedSourceDigest: bundle.digest,
    sourcePath: "workspace/source",
    scratchPath: "workspace/scratch",
  });

  await operate("stop", generation);
  const deleted = await operate("delete", generation);
  if (deleted.phase !== "deleted" || !deleted.cleanupObserved) {
    throw new Error("delete did not produce observed cleanup");
  }
  if (deleted.measuredCostMicrousd > deleted.sandboxBudgetMicrousd) {
    throw new Error("measured incremental cost exceeded the sandbox budget");
  }

  // Every provenance field below is read from receipts this process observed.
  // Nothing here is supplied by the guest.
  const imageDigests = new Set(receipts.map((receipt) => receipt.imageDigest));
  if (imageDigests.size !== 1 || !imageDigests.has(imageDigest)) {
    throw new Error("guest image digest was not stable across this run");
  }
  const runGeneration = create.generation;
  if (!Number.isInteger(runGeneration) || runGeneration < 1) {
    throw new Error("control plane did not report a positive resource generation");
  }
  capture.provenance = {
    guestImageDigest: imageDigest,
    isolationClass: "gce_vm",
    kind: "admitted_worker_run",
    providerKind: "live_gce",
    receiptRefs: receipts.map((receipt) => receipt.receiptRef),
    resourceGeneration: runGeneration,
    sandboxRef,
  };
  capture.workerProfileDigest = create.profileDigest;

  passed = true;
} catch (error) {
  failure = error instanceof Error ? error.message : String(error);
} finally {
  await guaranteedCleanup();
}

const residueSuffix = sha256(sandboxRef).slice(0, 20);
const residue = {
  compute: count("instances", `name=oa-msb-${residueSuffix}`),
  firewall: ["egress", "broker", "metadata", "ssh", "ingress"].reduce(
    (total, kind) => total + count("firewall-rules", `name=oa-msb-${kind}-${residueSuffix}`),
    0,
  ),
  disks: count("disks", `name=oa-msb-${residueSuffix}`),
};
const residual = residue.compute + residue.firewall + residue.disks;

if (passed && residual === 0 && capture !== undefined) {
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, JSON.stringify(capture), { mode: 0o600 });
}

process.stdout.write(
  `${JSON.stringify({
    passed: passed && residual === 0,
    variant,
    sandboxRef,
    resourceGeneration: receipts.at(0)?.generation,
    guestImageDigest: imageDigest,
    receiptRefs: receipts.map((receipt) => receipt.receiptRef),
    measuredCostMicrousd: receipts.at(-1)?.measuredCostMicrousd,
    capturePath: passed && residual === 0 ? out : null,
    driverSummary,
    residue,
    failure,
  })}\n`,
);
if (!passed || residual !== 0) process.exit(1);
