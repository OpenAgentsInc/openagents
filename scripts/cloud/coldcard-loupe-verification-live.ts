#!/usr/bin/env -S pnpm exec tsx

/**
 * Owner-gated live Loupe verification run (openagents #9294, OFR-007).
 *
 * This produces the evidence the OFR-007 acceptance audit said did not exist:
 * a vulnerable-fail / fixed-pass control pair that actually executed, after an
 * initial verdict was locked, on admitted OpenAgents Cloud managed sandboxes.
 *
 * The order below is the whole point and is not rearrangeable:
 *
 *   1. MECHANICAL. Build the pinned vulnerable Coldcard MK4 tree on an admitted
 *      sandbox and read the source-ref, macro-value and symbol-provider
 *      observations out of the capture the guest measured.
 *   2. LOCK. Write the initial verdict to the durable first-verdict ledger. The
 *      ledger is a file in the repository, so the lock is committed before any
 *      control runs and its ordering is externally checkable. A verdict that
 *      already exists for this verification is never overwritten.
 *   3. CONTROLS. Only then build the vulnerable and the fixed tree on two more
 *      admitted sandboxes and run the provider-provenance detector inside each
 *      guest. The detector's exit status is observed through the managed
 *      sandbox I/O route; this process does not decide it.
 *
 * The detector is deliberately not a statistical test of generator output. It
 * asks which object the linker resolved `rng_get` to, and whether MicroPython's
 * deterministic fallback contributes any symbol at all. That is the finding's
 * actual claim, and it is the only kind of claim `openagents.artifact_witness`
 * accepts as entropy-provenance evidence.
 *
 * Nothing in the emitted run file is asserted by a guest. Sandbox refs,
 * generations, image digests, receipt refs and lease windows come only from
 * managed-sandbox runtime receipts this process observed.
 */

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

const CAPTURE_SCHEMA = "openagents.artifact_witness_capture.v2";
const GUEST_IO_SCHEMA = "openagents.managed_sandbox_guest_io.v1";
const GUEST_DRIVER = "/opt/openagents-managed-sandbox/coldcard-build-driver.mjs";
const RUN_SCHEMA = "openagents.loupe_verification_live_run.v1";
const LEDGER_SCHEMA = "openagents.loupe_first_verdict_ledger.v1";

const VERIFICATION_REF = "verification.coldcard.rng-fallback.live.v1";
const APPROVED_PROVIDER_REF = "provider.object.boards/COLDCARD_MK4/rng.o";
const FALLBACK_PROVIDER_REF = "provider.object.rng.o";
const LINKED_RNG_SYMBOL = "symbol.linked.rng_get";

const PINS = {
  vulnerable: {
    commitSha: "bcc2c382a324690a2fcf972c0bac3b79bf923f7b",
    gitTreeSha: "7abc9a4c680b5623fc8a64f70555dd2d3802e488",
  },
  fixed: {
    commitSha: "ca72463709f4e3f8964952039d5caf955f566a87",
    gitTreeSha: "51f735fb2dd888865c81ea2ed1fc690a0ed5b396",
  },
} as const;

type Variant = keyof typeof PINS;
type Role = "mechanical" | "control_vulnerable" | "control_fixed";

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
const digestOf = (value: unknown): string => `sha256:${sha256(canonical(value))}`;

const canonical = (value: unknown): string =>
  Array.isArray(value)
    ? `[${value.map(canonical).join(",")}]`
    : value !== null && typeof value === "object"
      ? `{${Object.keys(value as Record<string, unknown>)
          .sort()
          .map((key) => `${JSON.stringify(key)}:${canonical((value as Record<string, unknown>)[key])}`)
          .join(",")}}`
      : JSON.stringify(value);

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
if (!args.includes("--apply") || process.env.OA_MANAGED_SANDBOX_OWNER_GATE !== "I_ACCEPT_LIVE_GCP_COST") {
  throw new Error(
    "live Loupe verification runs are default-off; pass --apply and set OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST",
  );
}
const ledgerPath = resolve(option("ledger") ?? "fixtures/forensics/coldcard/loupe-first-verdict-ledger.v1.json");
const out = resolve(option("out") ?? "fixtures/forensics/coldcard/loupe-verification-live-run.v1.json");

const baseUrl = required("OA_MANAGED_SANDBOX_BASE_URL").replace(/\/$/, "");
const token = required("OA_CODEX_CONTROL_TOKEN");
const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const imageDigest = required("OA_MANAGED_SANDBOX_IMAGE_DIGEST");
const profileDigest = required("OA_MANAGED_SANDBOX_PROFILE_DIGEST");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";
const buildTimeoutMillis = Number(process.env.OA_COLDCARD_BUILD_TIMEOUT_MS ?? 3_000_000);
const leaseMillis = 60 * 60 * 1_000;

/**
 * The provider-provenance detector, executed inside the guest.
 *
 * Exit 0 means the approved hardware-TRNG object provides `rng_get` and
 * MicroPython's deterministic fallback object contributes nothing to the link.
 * Exit 1 means it does not. Any other exit means the detector could not decide,
 * which the verifier reads as a failure, never as a pass.
 */
const DETECTOR_SOURCE = `
const { readFileSync, writeFileSync } = require("node:fs");
const { createHash } = require("node:crypto");
const [, , capturePath, reportPath] = process.argv;
const capture = JSON.parse(readFileSync(capturePath, "utf8"));
const providers = capture.symbolProviders ?? [];
const inventory = capture.symbolInventory ?? [];
const linked = providers.filter((entry) => entry.symbolName === ${JSON.stringify(LINKED_RNG_SYMBOL)});
const providerRefs = [...new Set(linked.map((entry) => entry.providerRef))].sort();
const fallbackSymbols = inventory
  .filter((entry) => entry.providerRef === ${JSON.stringify(FALLBACK_PROVIDER_REF)})
  .map((entry) => entry.symbolName)
  .sort();
const approvedProviderLinked =
  providerRefs.length === 1 && providerRefs[0] === ${JSON.stringify(APPROVED_PROVIDER_REF)};
const fallbackAbsent = fallbackSymbols.length === 0;
const satisfied = approvedProviderLinked && fallbackAbsent;
const report = {
  schema: "openagents.loupe_provider_provenance_report.v1",
  captureRef: capture.captureRef,
  targetCommit: capture.targetCommit,
  sourceBundleDigest: capture.sourceBundleDigest,
  approvedProviderRef: ${JSON.stringify(APPROVED_PROVIDER_REF)},
  observedProviderRefs: providerRefs,
  forbiddenProviderRef: ${JSON.stringify(FALLBACK_PROVIDER_REF)},
  observedForbiddenSymbolCount: fallbackSymbols.length,
  observedForbiddenSymbolSample: fallbackSymbols.slice(0, 8),
  enumeratedSymbolCount: inventory.length,
  approvedProviderLinked,
  fallbackAbsent,
  satisfied,
};
if (satisfied) {
  writeFileSync(reportPath, JSON.stringify(report));
}
process.stdout.write(JSON.stringify(report) + "\\n");
process.exit(satisfied ? 0 : 1);
`;
const detectorDigest = `sha256:${sha256(DETECTOR_SOURCE)}`;

interface SandboxRun {
  readonly role: Role;
  readonly variant: Variant;
  readonly workerRef: string;
  readonly sandboxRef: string;
  readonly resourceGeneration: number;
  readonly placementRef: string;
  readonly createReceiptRef: string;
  readonly receiptRefs: ReadonlyArray<string>;
  readonly leaseObservedAt: string;
  readonly leaseExpiresAt: string;
  readonly measuredCostMicrousd: number;
  readonly capture: Record<string, unknown>;
  readonly captureDigest: string;
  readonly buildExitStatus: number;
  readonly detector?: {
    readonly exitStatus: number;
    readonly observedAt: string;
    readonly reportDigest?: string;
    readonly report: Record<string, unknown>;
  };
}

const timestamp = (): string => new Date().toISOString().replace(/\.(\d{3})\d*Z$/, ".$1Z");

async function runSandbox(role: Role, variant: Variant, runDetector: boolean): Promise<SandboxRun> {
  const stamp = `${Date.now()}-${process.pid}-${role}`;
  const suffix = sha256(stamp).slice(0, 20);
  const sandboxRef = `sandbox.ofr007.${role.replace(/_/g, "-")}.${suffix}`;
  const capabilityRef = `capability-ref://run/${sha256(`${stamp}|run`).slice(0, 32)}`;
  const placementRef = `placement.coldcard.${sha256(sandboxRef).slice(0, 20)}`;
  const scope = {
    actorRef: "principal.owner.ofr007-loupe-verification",
    ownerRef: "owner.openagents.primary",
    tenantRef: "tenant.openagents.primary",
    programRef: "program.managed-agent-sandboxes.ofr007",
    workUnitRef: `work.ofr007.${suffix}`,
    sandboxRef,
  };
  const profile = {
    profileRef: required("OA_MANAGED_SANDBOX_PROFILE_REF"),
    profileDigest,
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
    ttlMs: leaseMillis,
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

  const operate = async (
    action: RuntimeReceipt["action"],
    expectedGeneration: number,
    includeProfile = false,
  ): Promise<RuntimeReceipt> => {
    counter += 1;
    const operationId = sha256(`${stamp}|${counter}|${action}`).slice(0, 32);
    const response = await fetch(`${baseUrl}/v1/managed-sandbox/runtime/operations`, {
      method: "POST",
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
      body: JSON.stringify({
        ...scope,
        operationRef: `operation.ofr007-live.${operationId}`,
        idempotencyRef: `idempotency.ofr007-live.${operationId}`,
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
  };

  let ioCounter = 0;
  const guestIo = async (request: Record<string, unknown>): Promise<Record<string, unknown>> => {
    ioCounter += 1;
    const operationId = sha256(`${stamp}|io|${ioCounter}`).slice(0, 32);
    const now = new Date();
    process.stderr.write(`[${role}] io ${String(request.action)} start ${now.toISOString()}\n`);
    const response = await fetch(`${baseUrl}/v1/managed-sandbox/runtime/io`, {
      method: "POST",
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
      signal: AbortSignal.timeout(buildTimeoutMillis + 600_000),
      body: JSON.stringify({
        schemaVersion: GUEST_IO_SCHEMA,
        ...scope,
        operationRef: `operation.ofr007-io.${operationId}`,
        idempotencyRef: `idempotency.ofr007-io.${operationId}`,
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
        `guest io ${String(request.action)} refused with ${response.status}: ${JSON.stringify(body).slice(0, 600)}`,
      );
    }
    process.stderr.write(`[${role}] io ${String(request.action)} done ${new Date().toISOString()}\n`);
    return body;
  };

  const bundle = (() => {
    const manifest = [
      `# Coldcard Loupe verification run`,
      `role: ${role}`,
      `variant: ${variant}`,
      `commit: ${PINS[variant].commitSha}`,
      `tree: ${PINS[variant].gitTreeSha}`,
      `detectorDigest: ${detectorDigest}`,
      ``,
    ].join("\n");
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
  })();

  const guaranteedCleanup = async (): Promise<void> => {
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
        // The residue check below still fails the run.
      }
    }
  };

  let result: SandboxRun | undefined;
  let failure: string | undefined;
  try {
    const create = await operate("create", 0, true);
    const leaseObservedAt = timestamp();
    if (create.phase !== "ready" || !create.readinessObserved) {
      throw new Error("create did not produce observed ready state");
    }
    if (create.imageDigest !== imageDigest) {
      throw new Error("control plane admitted a different guest image than requested");
    }
    if (create.profileDigest !== profileDigest) {
      throw new Error("control plane admitted a different worker profile than requested");
    }
    if (create.isolationClass !== "gce_vm") {
      throw new Error(`unexpected isolation class ${create.isolationClass}`);
    }

    await guestIo({
      action: "install_forensic_source",
      artifactRef: `artifact.ofr007.${sha256(bundle.base64).slice(0, 32)}`,
      artifactContentBase64: bundle.base64,
      artifactContentDigest: bundle.digest,
      sourcePath: "workspace/source",
      scratchPath: "workspace/scratch",
    });

    const buildCommand = [
      `OA_COLDCARD_WORKER_PROFILE_DIGEST=${create.profileDigest}`,
      `OA_COLDCARD_PLACEMENT_REF=${placementRef}`,
      "node",
      GUEST_DRIVER,
      "--variant",
      variant,
      "--workdir",
      "/workspace/scratch/build",
      "--out",
      "/workspace/scratch/capture.json",
    ].join(" ");
    const built = await guestIo({
      action: "execute_command",
      command: buildCommand,
      commandDigest: `sha256:${sha256(buildCommand)}`,
      cwd: "workspace",
      timeoutMillis: buildTimeoutMillis,
    });
    if (built.timedOut === true) throw new Error("guest build timed out");
    const buildExitStatus = Number(built.exitCode ?? -1);
    if (buildExitStatus !== 0) {
      throw new Error(
        `coldcard build driver exited ${buildExitStatus}: ${String(built.stderr).slice(0, 400)}`,
      );
    }

    const artifact = await guestIo({
      action: "read_artifact",
      path: "workspace/scratch/capture.json",
      retentionUntil: new Date(Date.now() + 86_400_000).toISOString(),
    });
    const capture = JSON.parse(
      Buffer.from(String(artifact.contentBase64), "base64").toString("utf8"),
    ) as Record<string, unknown>;
    if (capture.schema !== CAPTURE_SCHEMA) {
      throw new Error(`guest produced an unexpected capture schema ${String(capture.schema)}`);
    }
    if (capture.variant !== variant) throw new Error("guest capture variant mismatch");
    if (capture.targetCommit !== PINS[variant].commitSha) {
      throw new Error("guest capture did not build the pinned commit");
    }

    let detector: SandboxRun["detector"];
    if (runDetector) {
      // The detector is written into the guest as bytes and run there. Its exit
      // status is what the managed-sandbox I/O route observed, not a verdict
      // this process reached about a build it never ran.
      const install = [
        "printf",
        "%s",
        `'${Buffer.from(DETECTOR_SOURCE, "utf8").toString("base64")}'`,
        "|",
        "base64",
        "-d",
        ">",
        "/workspace/scratch/detector.cjs",
      ].join(" ");
      const installed = await guestIo({
        action: "execute_command",
        command: install,
        commandDigest: `sha256:${sha256(install)}`,
        cwd: "workspace",
        timeoutMillis: 120_000,
      });
      if (Number(installed.exitCode ?? -1) !== 0) {
        throw new Error(`detector install failed: ${String(installed.stderr).slice(0, 400)}`);
      }
      const detectCommand =
        "node /workspace/scratch/detector.cjs /workspace/scratch/capture.json /workspace/scratch/report.json";
      const detected = await guestIo({
        action: "execute_command",
        command: detectCommand,
        commandDigest: `sha256:${sha256(detectCommand)}`,
        cwd: "workspace",
        timeoutMillis: 300_000,
      });
      if (detected.timedOut === true) throw new Error("guest detector timed out");
      const detectorObservedAt = timestamp();
      const exitStatus = Number(detected.exitCode ?? -1);
      const report = JSON.parse(String(detected.stdout).trim().split("\n").at(-1) ?? "{}") as Record<
        string,
        unknown
      >;
      let reportDigest: string | undefined;
      if (exitStatus === 0) {
        const reportArtifact = await guestIo({
          action: "read_artifact",
          path: "workspace/scratch/report.json",
          retentionUntil: new Date(Date.now() + 86_400_000).toISOString(),
        });
        reportDigest = `sha256:${sha256(Buffer.from(String(reportArtifact.contentBase64), "base64"))}`;
      }
      detector = {
        exitStatus,
        observedAt: detectorObservedAt,
        ...(reportDigest === undefined ? {} : { reportDigest }),
        report,
      };
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

    const imageDigests = new Set(receipts.map((receipt) => receipt.imageDigest));
    if (imageDigests.size !== 1 || !imageDigests.has(imageDigest)) {
      throw new Error("guest image digest was not stable across this run");
    }

    result = {
      role,
      variant,
      workerRef: `worker.coldcard.${role.replace(/_/g, "-")}.${suffix}`,
      sandboxRef,
      resourceGeneration: create.generation,
      placementRef,
      createReceiptRef: create.receiptRef,
      receiptRefs: receipts.map((receipt) => receipt.receiptRef),
      leaseObservedAt,
      leaseExpiresAt: timestamp2(leaseObservedAt, leaseMillis),
      measuredCostMicrousd: deleted.measuredCostMicrousd,
      capture,
      captureDigest: digestOf(capture),
      buildExitStatus,
      ...(detector === undefined ? {} : { detector }),
    };
  } catch (error) {
    failure = error instanceof Error ? error.message : String(error);
  } finally {
    await guaranteedCleanup();
  }

  const residueSuffix = sha256(sandboxRef).slice(0, 20);
  const residue =
    count("instances", `name=oa-msb-${residueSuffix}`) +
    ["egress", "broker", "metadata", "ssh", "ingress"].reduce(
      (total, kind) => total + count("firewall-rules", `name=oa-msb-${kind}-${residueSuffix}`),
      0,
    ) +
    count("disks", `name=oa-msb-${residueSuffix}`);
  if (residue !== 0) throw new Error(`${role} left ${residue} live resources behind`);
  if (result === undefined) throw new Error(`${role} run failed: ${failure ?? "unknown"}`);
  return result;
}

function timestamp2(from: string, addMillis: number): string {
  return new Date(Date.parse(from) + addMillis).toISOString().replace(/\.(\d{3})\d*Z$/, ".$1Z");
}

function count(collection: "instances" | "firewall-rules" | "disks", filter: string): number {
  const commandArgs = ["compute", collection, "list", "--project", projectId];
  if (collection !== "firewall-rules") commandArgs.push("--zones", zone);
  commandArgs.push("--filter", filter, "--format", "value(name)");
  const output = execFileSync(gcloud, commandArgs, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  return output.split("\n").filter((line) => line.trim().length > 0).length;
}

const observation = <Value>(capture: Record<string, unknown>, key: string): Array<Value> =>
  (capture[key] as Array<Value> | undefined) ?? [];

// ---------------------------------------------------------------------------
// 1. MECHANICAL
// ---------------------------------------------------------------------------
const mechanical = await runSandbox("mechanical", "vulnerable", false);
const macro = observation<{ macroName: string; value: string; preprocessedArtifactRef: string }>(
  mechanical.capture,
  "macroObservations",
).find((entry) => entry.macroName === "macro.MICROPY_HW_ENABLE_RNG");
const provider = observation<{ symbolName: string; providerRef: string; linkMapArtifactRef: string }>(
  mechanical.capture,
  "symbolProviders",
).find((entry) => entry.symbolName === LINKED_RNG_SYMBOL);
if (macro === undefined) throw new Error("mechanical capture observed no RNG guard macro");
if (provider === undefined) throw new Error("mechanical capture resolved no rng_get provider");
if (provider.providerRef !== FALLBACK_PROVIDER_REF) {
  // The finding under verification is that the vulnerable tree links the
  // deterministic fallback. If the measurement disagrees, the honest initial
  // verdict is not `confirmed`, and this run must not manufacture one.
  throw new Error(
    `vulnerable build resolved rng_get to ${provider.providerRef}, not the deterministic fallback`,
  );
}

const environmentDigest = digestOf({
  workerImageDigest: imageDigest,
  workerProfileDigest: profileDigest,
});
const mechanicalObservedAt = String(mechanical.capture.capturedAt);
const vulnerableTargetDigest = String(mechanical.capture.sourceBundleDigest);

const mechanicalReceipt = (
  sequence: number,
  operation: string,
  subjectRef: string,
  commandInput: string,
  resultRecord: unknown,
) => ({
  schema: "openagents.loupe_verification_evidence.v1",
  receiptRef: `receipt.ofr007.mechanical.${sequence}.${sha256(`${mechanical.sandboxRef}|${sequence}`).slice(0, 16)}`,
  verificationRef: VERIFICATION_REF,
  sequence,
  operation,
  evidenceTier: "source_observed",
  subjectRef,
  commandDigest: `sha256:${sha256(commandInput)}`,
  inputDigests: [vulnerableTargetDigest, String(mechanical.capture.buildConfigurationDigest)],
  outcome: "succeeded",
  resultDigest: digestOf(resultRecord),
  environmentDigest,
  workerRef: mechanical.workerRef,
  workerReceiptRef: mechanical.createReceiptRef,
  observedAt: mechanicalObservedAt,
});

const mechanicalEvidence = [
  mechanicalReceipt(
    1,
    "source_ref_resolved",
    String(mechanical.capture.targetSnapshotRef),
    `git-tree ${PINS.vulnerable.gitTreeSha}`,
    {
      targetCommit: mechanical.capture.targetCommit,
      sourceBundleDigest: mechanical.capture.sourceBundleDigest,
      targetSnapshotRef: mechanical.capture.targetSnapshotRef,
    },
  ),
  mechanicalReceipt(2, "macro_value_observed", macro.macroName, "arm-none-eabi-gcc -E", macro),
  mechanicalReceipt(3, "symbol_provider_resolved", provider.symbolName, "arm-none-eabi-nm", provider),
];

// ---------------------------------------------------------------------------
// 2. LOCK — durable first verdict, written before any control runs
// ---------------------------------------------------------------------------
const lockedAt = timestamp();
const candidateVerdict = {
  schema: "openagents.loupe_initial_verdict.v1",
  verdictRef: `verdict.ofr007.${sha256(`${VERIFICATION_REF}|${lockedAt}`).slice(0, 24)}`,
  verificationRef: VERIFICATION_REF,
  findingDigest: digestOf({
    claim: "the pinned vulnerable Coldcard MK4 build links MicroPython's deterministic fallback rng_get",
    approvedProviderRef: APPROVED_PROVIDER_REF,
    fallbackProviderRef: FALLBACK_PROVIDER_REF,
    symbolName: LINKED_RNG_SYMBOL,
    vulnerableCommit: PINS.vulnerable.commitSha,
    fixedCommit: PINS.fixed.commitSha,
  }),
  verifierActorRef: "actor.verifier.ofr007.loupe.v1",
  outcome: "confirmed",
  evidenceReceiptRefs: mechanicalEvidence.map((receipt) => receipt.receiptRef),
  rationaleDigest: digestOf({ macro, provider }),
  lockedAt,
};

const ledger: Record<string, unknown> = existsSync(ledgerPath)
  ? (JSON.parse(readFileSync(ledgerPath, "utf8")) as Record<string, unknown>)
  : { schema: LEDGER_SCHEMA, verdicts: {} };
const stored = (ledger.verdicts as Record<string, unknown>)[VERIFICATION_REF];
if (stored !== undefined && digestOf(stored) !== digestOf(candidateVerdict)) {
  throw new Error(
    "a different initial verdict is already durably locked for this verification; refusing to relock",
  );
}
(ledger.verdicts as Record<string, unknown>)[VERIFICATION_REF] = candidateVerdict;
mkdirSync(dirname(ledgerPath), { recursive: true });
writeFileSync(ledgerPath, `${JSON.stringify(ledger, null, 2)}\n`);
process.stderr.write(`[lock] initial verdict durably locked at ${lockedAt}\n`);

// ---------------------------------------------------------------------------
// 3. CONTROLS — only now
// ---------------------------------------------------------------------------
const vulnerableControl = await runSandbox("control_vulnerable", "vulnerable", true);
const fixedControl = await runSandbox("control_fixed", "fixed", true);
const fixedTargetDigest = String(fixedControl.capture.sourceBundleDigest);
if (fixedTargetDigest === vulnerableTargetDigest) {
  throw new Error("the vulnerable and fixed controls resolved to the same immutable target");
}

const pocObservedAt = vulnerableControl.detector?.observedAt ?? timestamp();
const controlEvidence = [
  {
    schema: "openagents.loupe_verification_evidence.v1",
    receiptRef: `receipt.ofr007.poc.${sha256(`${vulnerableControl.sandboxRef}|poc`).slice(0, 16)}`,
    verificationRef: VERIFICATION_REF,
    sequence: 4,
    operation: "poc_applied",
    evidenceTier: "artifact_observed",
    subjectRef: "detector.coldcard.provider-provenance.v1",
    commandDigest: detectorDigest,
    inputDigests: [vulnerableTargetDigest, String(vulnerableControl.capture.buildConfigurationDigest)],
    outcome: "succeeded",
    resultDigest: detectorDigest,
    environmentDigest,
    workerRef: vulnerableControl.workerRef,
    workerReceiptRef: vulnerableControl.createReceiptRef,
    observedAt: pocObservedAt,
  },
  {
    schema: "openagents.loupe_verification_evidence.v1",
    receiptRef: `receipt.ofr007.control.vulnerable.${sha256(vulnerableControl.sandboxRef).slice(0, 16)}`,
    verificationRef: VERIFICATION_REF,
    sequence: 5,
    operation: "control_test_observed",
    evidenceTier: "executed",
    subjectRef: String(vulnerableControl.capture.targetSnapshotRef),
    commandDigest: detectorDigest,
    inputDigests: [vulnerableTargetDigest],
    outcome: "succeeded",
    resultDigest: digestOf(vulnerableControl.detector?.report ?? {}),
    environmentDigest,
    workerRef: vulnerableControl.workerRef,
    workerReceiptRef: vulnerableControl.createReceiptRef,
    controlRevision: "vulnerable",
    expectedTestOutcome: "failure",
    observedTermination: {
      status: "observed",
      exitStatus: vulnerableControl.detector?.exitStatus ?? -1,
      ...(vulnerableControl.detector?.reportDigest === undefined
        ? {}
        : { resultArtifactDigest: vulnerableControl.detector.reportDigest }),
    },
    observedAt: vulnerableControl.detector?.observedAt ?? timestamp(),
  },
  {
    schema: "openagents.loupe_verification_evidence.v1",
    receiptRef: `receipt.ofr007.control.fixed.${sha256(fixedControl.sandboxRef).slice(0, 16)}`,
    verificationRef: VERIFICATION_REF,
    sequence: 6,
    operation: "control_test_observed",
    evidenceTier: "executed",
    subjectRef: String(fixedControl.capture.targetSnapshotRef),
    commandDigest: detectorDigest,
    inputDigests: [fixedTargetDigest],
    outcome: "succeeded",
    resultDigest: digestOf(fixedControl.detector?.report ?? {}),
    environmentDigest,
    workerRef: fixedControl.workerRef,
    workerReceiptRef: fixedControl.createReceiptRef,
    controlRevision: "fixed",
    expectedTestOutcome: "success",
    observedTermination: {
      status: "observed",
      exitStatus: fixedControl.detector?.exitStatus ?? -1,
      ...(fixedControl.detector?.reportDigest === undefined
        ? {}
        : { resultArtifactDigest: fixedControl.detector.reportDigest }),
    },
    observedAt: fixedControl.detector?.observedAt ?? timestamp(),
  },
];

const runs = [mechanical, vulnerableControl, fixedControl];
const runFile = {
  schema: RUN_SCHEMA,
  runRef: `run.coldcard.loupe-verification.${sha256(lockedAt).slice(0, 16)}`,
  note:
    "Live OFR-007 acceptance evidence. The mechanical tier was measured on an admitted sandbox, the initial verdict was then written to the durable first-verdict ledger, and only afterwards did the vulnerable and fixed control tests execute inside two further admitted sandboxes. Every sandbox ref, generation, receipt ref, image digest and lease window below was observed by the orchestrating process from managed-sandbox runtime receipts, never reported by a guest.",
  detector: {
    detectorRef: "detector.coldcard.provider-provenance.v1",
    detectorDigest,
    description:
      "Exits 0 when the linker resolved rng_get to the board's hardware-TRNG object and MicroPython's deterministic fallback object contributes no symbol to the enumerated inventory; exits 1 otherwise. This is a provenance check over measured link and symbol evidence, never a statistical test of generator output.",
  },
  plan: {
    schema: "openagents.loupe_verification_plan.v1",
    verificationRef: VERIFICATION_REF,
    runRef: `run.coldcard.loupe-verification.${sha256(lockedAt).slice(0, 16)}`,
    findingRef: "finding.coldcard.rng-fallback.v1",
    findingDigest: candidateVerdict.findingDigest,
    discoveryActorRef: "actor.discovery.ofr014.artifact-witness.v1",
    verifierActorRef: candidateVerdict.verifierActorRef,
    evidenceProvenance: "admitted_worker_run",
    sourceBundleRef: String(mechanical.capture.targetSnapshotRef),
    sourceBundleDigest: vulnerableTargetDigest,
    dependencyManifestDigest: String(mechanical.capture.buildConfigurationDigest),
    vulnerableTargetDigest,
    fixedTargetDigest,
    workerImageDigest: imageDigest,
    workerProfileDigest: profileDigest,
    admittedWorkers: runs.map((run) => ({
      workerRef: run.workerRef,
      sandboxRef: run.sandboxRef,
      resourceGeneration: run.resourceGeneration,
      placementRef: run.placementRef,
    })),
    createdAt: mechanicalObservedAt,
  },
  initialVerdict: candidateVerdict,
  mechanicalEvidence,
  controlEvidence,
  admittedWorkerReceipts: runs.map((run) => ({
    schema: "openagents.loupe_admitted_worker_receipt.v1",
    receiptRef: run.createReceiptRef,
    sandboxRef: run.sandboxRef,
    resourceGeneration: run.resourceGeneration,
    placementRef: run.placementRef,
    imageDigest,
    profileDigest,
    lifecycleState: "admitted",
    exact: true,
    observedAt: run.leaseObservedAt,
    expiresAt: run.leaseExpiresAt,
  })),
  runs: runs.map((run) => ({
    role: run.role,
    variant: run.variant,
    sandboxRef: run.sandboxRef,
    resourceGeneration: run.resourceGeneration,
    receiptRefs: run.receiptRefs,
    capturedAt: run.capture.capturedAt,
    captureDigest: run.captureDigest,
    targetCommit: run.capture.targetCommit,
    sourceBundleDigest: run.capture.sourceBundleDigest,
    buildExitStatus: run.buildExitStatus,
    measuredCostMicrousd: run.measuredCostMicrousd,
    ...(run.detector === undefined
      ? {}
      : {
          detectorExitStatus: run.detector.exitStatus,
          detectorObservedAt: run.detector.observedAt,
          detectorReport: run.detector.report,
          ...(run.detector.reportDigest === undefined
            ? {}
            : { detectorReportDigest: run.detector.reportDigest }),
        }),
  })),
  completedAt: timestamp(),
  totalMeasuredCostMicrousd: runs.reduce((total, run) => total + run.measuredCostMicrousd, 0),
};

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify(runFile, null, 2)}\n`);

process.stdout.write(
  `${JSON.stringify({
    passed: true,
    out,
    ledgerPath,
    lockedAt,
    vulnerableDetectorExit: vulnerableControl.detector?.exitStatus,
    fixedDetectorExit: fixedControl.detector?.exitStatus,
    totalMeasuredCostMicrousd: runFile.totalMeasuredCostMicrousd,
  })}\n`,
);
