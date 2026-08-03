#!/usr/bin/env -S pnpm exec tsx

/**
 * Owner-gated live Coldcard generator run (openagents #9297, OFR-015).
 *
 * Provisions one live `live_gce` managed sandbox on the admitted profile,
 * installs the generator harness into it, compiles the pinned libngu
 * `ngu/random.c` from the read-only Coldcard tree baked into the guest image,
 * captures the expected values for all eight frozen vectors from that compiled
 * target source, measures a candidate-search rate through the same code, and
 * deletes the sandbox.
 *
 * Provenance is stamped HERE, not in the guest, because a guest cannot attest
 * to its own admission. Every provenance field comes from a managed-sandbox
 * runtime receipt this process observed. If any of them disagrees with what
 * this process requested, it refuses rather than writing a capture.
 */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const CAPTURE_SCHEMA = "openagents.coldcard_generator_capture.v1";
const GUEST_IO_SCHEMA = "openagents.managed_sandbox_guest_io.v1";
const LIBNGU_RANDOM_C = "/opt/coldcard/vulnerable/external/libngu/ngu/random.c";

/** The libngu commit both pinned Coldcard firmware trees carry. */
const LIBNGU_COMMIT = "537519a829259622ea6b0334fbafd6cae852852f";
/** sha256 of `ngu/random.c` at that commit, re-derived by the guest. */
const LIBNGU_RANDOM_C_DIGEST =
  "sha256:812585e47b2f9251693280c95b5e58558cbd564d62e4398b17388f9cb5198abb";

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

const args = process.argv.slice(2);
const option = (name: string): string | undefined => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 ? args[index + 1] : undefined;
};
const apply = args.includes("--apply");
const out = resolve(option("out") ?? "artifacts/coldcard-generator-capture.json");
const minNanos = option("min-nanos") ?? "20000000000";
const batch = option("batch") ?? "8192";

if (!apply || process.env.OA_MANAGED_SANDBOX_OWNER_GATE !== "I_ACCEPT_LIVE_GCP_COST") {
  throw new Error(
    "live generator runs are default-off; pass --apply and set OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST",
  );
}

const baseUrl = required("OA_MANAGED_SANDBOX_BASE_URL").replace(/\/$/, "");
const token = required("OA_CODEX_CONTROL_TOKEN");
const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const imageDigest = required("OA_MANAGED_SANDBOX_IMAGE_DIGEST");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const runTimeoutMillis = Number(process.env.OA_COLDCARD_GENERATOR_TIMEOUT_MS ?? 1_800_000);

const stamp = `${Date.now()}-${process.pid}-generator`;
const suffix = sha256(stamp).slice(0, 20);
// The guest I/O route rejects `/` in public refs; dot-separated refs are the
// form both it and the runtime operations route accept.
const sandboxRef = `sandbox.ofr015.generator.${suffix}`;
const capabilityRef = `capability-ref://run/${sha256(`${stamp}|run`).slice(0, 32)}`;
const scope = {
  actorRef: "principal.owner.ofr015-generator",
  ownerRef: "owner.openagents.primary",
  tenantRef: "tenant.openagents.primary",
  programRef: "program.managed-agent-sandboxes.ofr015",
  workUnitRef: `work.ofr015.${suffix}`,
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
      operationRef: `operation.ofr015-live.${operationId}`,
      idempotencyRef: `idempotency.ofr015-live.${operationId}`,
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
    signal: AbortSignal.timeout(runTimeoutMillis + 600_000),
    body: JSON.stringify({
      schemaVersion: GUEST_IO_SCHEMA,
      ...scope,
      operationRef: `operation.ofr015-io.${operationId}`,
      idempotencyRef: `idempotency.ofr015-io.${operationId}`,
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
 * The harness the guest compiles and runs.
 *
 * The pinned libngu source is NOT shipped: it is already inside the admitted
 * guest image, which has no network, and the guest re-derives its digest.
 * What ships is the driver, the two verbatim-include translation units, the
 * MicroPython interface shim, and the stepping harness.
 */
const HARNESS_ROOT = join(dirname(fileURLToPath(import.meta.url)), "coldcard-generator-guest");
const HARNESS_ENTRIES: ReadonlyArray<{ readonly path: string; readonly source: string }> = [
  {
    path: "coldcard-generator-driver.mjs",
    source: join(HARNESS_ROOT, "..", "coldcard-generator-driver.mjs"),
  },
  { path: "oa_harness.h", source: join(HARNESS_ROOT, "oa_harness.h") },
  { path: "oa_libngu.c", source: join(HARNESS_ROOT, "oa_libngu.c") },
  { path: "oa_main.c", source: join(HARNESS_ROOT, "oa_main.c") },
  { path: "oa_provider.c", source: join(HARNESS_ROOT, "oa_provider.c") },
  { path: "oa_shim.c", source: join(HARNESS_ROOT, "oa_shim.c") },
  { path: "shim/py/mperrno.h", source: join(HARNESS_ROOT, "shim", "py", "mperrno.h") },
  { path: "shim/py/runtime.h", source: join(HARNESS_ROOT, "shim", "py", "runtime.h") },
];

function sourceBundle(): { base64: string; digest: string } {
  // The guest re-derives this digest from the installed tree using its own
  // canonical JSON form (sorted keys, compact separators). Serializing any
  // other way makes post-copy verification fail even though the bytes on disk
  // are correct.
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
    repositoryRef: "repository.github.switck.libngu",
    commitSha: LIBNGU_COMMIT,
    gitTreeSha: LIBNGU_COMMIT,
    entries: HARNESS_ENTRIES.map((entry) => {
      const content = readFileSync(entry.source);
      return {
        path: entry.path,
        contentDigest: `sha256:${sha256(content)}`,
        contentBase64: content.toString("base64"),
      };
    }),
  });
  return {
    base64: Buffer.from(payload, "utf8").toString("base64"),
    digest: `sha256:${sha256(payload)}`,
  };
}

function count(collection: "instances" | "firewall-rules" | "disks", filter: string): number {
  const argv = ["compute", collection, "list", "--project", projectId];
  if (collection !== "firewall-rules") argv.push("--zones", zone);
  argv.push("--filter", filter, "--format", "value(name)");
  const output = execFileSync(gcloud, argv, {
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
    // The independent residue check below still fails the run.
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
    artifactRef: `artifact.ofr015.${sha256(bundle.base64).slice(0, 32)}`,
    artifactContentBase64: bundle.base64,
    artifactContentDigest: bundle.digest,
    sourcePath: "workspace/source",
    scratchPath: "workspace/scratch",
  });

  const command = [
    `OA_COLDCARD_WORKER_PROFILE_DIGEST=${create.profileDigest}`,
    "node",
    "/workspace/source/coldcard-generator-driver.mjs",
    "--source-dir",
    "/workspace/source",
    "--random-c",
    LIBNGU_RANDOM_C,
    "--workdir",
    "/workspace/scratch/generator",
    "--out",
    "/workspace/scratch/generator-capture.json",
    "--min-nanos",
    minNanos,
    "--batch",
    batch,
  ].join(" ");
  const executed = await guestIo({
    action: "execute_command",
    command,
    commandDigest: `sha256:${sha256(command)}`,
    cwd: "workspace",
    timeoutMillis: runTimeoutMillis,
  });
  if (executed.timedOut === true) throw new Error("guest generator run timed out");
  if (executed.exitCode !== 0) {
    throw new Error(
      `generator driver exited ${String(executed.exitCode)}: ${String(executed.stderr).slice(0, 600)}`,
    );
  }
  driverSummary = JSON.parse(String(executed.stdout).trim().split("\n").at(-1) ?? "{}");

  const artifact = await guestIo({
    action: "read_artifact",
    path: "workspace/scratch/generator-capture.json",
    retentionUntil: new Date(Date.now() + 86_400_000).toISOString(),
  });
  capture = JSON.parse(
    Buffer.from(String(artifact.contentBase64), "base64").toString("utf8"),
  ) as Record<string, unknown>;
  if (capture.schema !== CAPTURE_SCHEMA) {
    throw new Error(`guest produced an unexpected capture schema ${String(capture.schema)}`);
  }
  const targetSource = capture.targetSource as Record<string, unknown>;
  if (targetSource?.path !== LIBNGU_RANDOM_C) {
    throw new Error("guest compiled a source other than the pinned libngu generator");
  }
  if (targetSource?.sourceDigest !== LIBNGU_RANDOM_C_DIGEST) {
    throw new Error(
      `guest libngu source digest ${String(targetSource?.sourceDigest)} is not the pinned generator`,
    );
  }
  if ((capture.vectors as ReadonlyArray<unknown>).length !== 8) {
    throw new Error("guest did not produce all eight vector classes");
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
