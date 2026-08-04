import { createHash } from "node:crypto";

import { forensicCanonicalJson, forensicSha256Digest } from "@openagentsinc/forensic-contract";

import type { LoupeControlPlaneTransport } from "./control-plane.ts";
import { durableFirstVerdictLedger } from "./verdict-ledger.ts";
import {
  LOUPE_ADMITTED_WORKER_RECEIPT_VERSION,
  LOUPE_INITIAL_VERDICT_VERSION,
  LOUPE_VERIFICATION_EVIDENCE_VERSION,
  LOUPE_VERIFICATION_PLAN_VERSION,
  LOUPE_VERIFICATION_SESSION_BRAND,
  evaluateLoupeVerificationSession,
  type LoupeEvidenceProvenance,
  type LoupeInitialVerdictDraft,
  type LoupeVerificationResult,
  type LoupeVerificationSession,
} from "./verification-contract.ts";

const GUEST_IO_SCHEMA = "openagents.managed_sandbox_guest_io.v1";
const RUNTIME_RECEIPT_SCHEMA = "openagents.managed_sandbox_runtime.v1";
const CAPTURE_SCHEMA = "openagents.artifact_witness_capture.v2";
const SOURCE_BUNDLE_SCHEMA = "openagents.forensic_source_bundle_payload.v2";

const sha256Hex = (value: string | Uint8Array): string =>
  createHash("sha256").update(value).digest("hex");
const contentDigest = (value: string | Uint8Array): string => `sha256:${sha256Hex(value)}`;

export type LoupeControlVariant = "vulnerable" | "fixed";
export type LoupeVerificationRole = "mechanical" | "control_vulnerable" | "control_fixed";

export interface LoupeTargetPin {
  readonly commitSha: string;
  readonly gitTreeSha: string;
}

/**
 * What a caller of the Loupe verifier is allowed to supply.
 *
 * It is an intent: which finding, which two immutable revisions, which admitted
 * worker profile, and which budget. It contains no evidence, no verdict, no
 * worker receipt and no authority. Everything the verifier concludes is
 * measured by the verifier through the control-plane transport.
 */
export interface LoupeVerificationSpec {
  readonly verificationRef: string;
  readonly runRef: string;
  readonly findingRef: string;
  readonly discoveryActorRef: string;
  readonly verifierActorRef: string;
  /** Seeds every sandbox, worker, placement, operation and receipt ref. */
  readonly runSeed: string;
  readonly finding: {
    readonly claim: string;
    readonly symbolName: string;
    readonly guardMacroName: string;
    readonly approvedProviderRef: string;
    readonly forbiddenProviderRef: string;
  };
  readonly targets: {
    readonly repositoryRef: string;
    readonly vulnerable: LoupeTargetPin;
    readonly fixed: LoupeTargetPin;
  };
  readonly worker: {
    readonly imageDigest: string;
    readonly profileDigest: string;
    readonly profileRef: string;
    readonly provisionerRef: string;
    readonly region: string;
    readonly machineClass: string;
    readonly networkPolicyRef: string;
    readonly controlIdentityRef: string;
    readonly actorRef: string;
    readonly ownerRef: string;
    readonly tenantRef: string;
    readonly programRef: string;
  };
  readonly limits: {
    readonly buildTimeoutMillis: number;
    readonly leaseMillis: number;
    readonly sandboxBudgetMicrousd: number;
    readonly programBudgetMicrousd: number;
  };
  readonly guestDriverPath: string;
}

interface RuntimeReceipt {
  readonly schemaVersion: string;
  readonly receiptRef: string;
  readonly action: string;
  readonly sandboxRef: string;
  readonly generation: number;
  readonly phase: string;
  readonly profileDigest: string;
  readonly imageDigest: string;
  readonly isolationClass: string;
  readonly providerKind: string;
  readonly readinessObserved: boolean;
  readonly cleanupObserved: boolean;
  readonly measuredCostMicrousd: number;
  readonly sandboxBudgetMicrousd: number;
}

export interface LoupeSandboxRunSummary {
  readonly role: LoupeVerificationRole;
  readonly variant: LoupeControlVariant;
  readonly sandboxRef: string;
  readonly workerRef: string;
  readonly resourceGeneration: number;
  readonly receiptRefs: ReadonlyArray<string>;
  readonly capturedAt: string;
  readonly captureDigest: string;
  readonly targetCommit: string;
  readonly sourceBundleDigest: string;
  readonly buildExitStatus: number;
  readonly measuredCostMicrousd: number;
  readonly detectorExitStatus?: number;
  readonly detectorObservedAt?: string;
  readonly detectorReport?: Record<string, unknown>;
}

export interface LoupeVerificationRun {
  readonly plan: Record<string, unknown>;
  readonly result: LoupeVerificationResult;
  readonly initialVerdict: Record<string, unknown>;
  readonly mechanicalEvidence: ReadonlyArray<Record<string, unknown>>;
  readonly controlEvidence: ReadonlyArray<Record<string, unknown>>;
  readonly admittedWorkerReceipts: ReadonlyArray<Record<string, unknown>>;
  readonly runs: ReadonlyArray<LoupeSandboxRunSummary>;
  readonly detectorRef: string;
  readonly detectorDigest: string;
  readonly completedAt: string;
  readonly totalMeasuredCostMicrousd: number;
}

/**
 * A conformance transport is a simulation of a control plane, so nothing
 * derived through it may claim admitted-worker provenance. This is where that
 * cap is applied, and it is derived from the transport rather than declared by
 * a caller, which is the point.
 */
const provenanceOf = (transport: LoupeControlPlaneTransport): LoupeEvidenceProvenance =>
  transport.kind === "conformance" ? "conformance_vector" : "admitted_worker_run";

/**
 * The provider-provenance detector, executed inside the guest.
 *
 * Exit 0 means the linker resolved the finding's symbol to the approved object
 * and the forbidden object contributes no symbol at all to the enumerated
 * inventory. Exit 1 means it does not. It is a provenance question over
 * measured link and symbol evidence, never a statistical test of generator
 * output. It lives here, in the verifier, because a verifier that is handed its
 * own instrument by the party under verification has not verified anything.
 */
const detectorSource = (spec: LoupeVerificationSpec): string => `
const { readFileSync, writeFileSync } = require("node:fs");
const [, , capturePath, reportPath] = process.argv;
const capture = JSON.parse(readFileSync(capturePath, "utf8"));
const providers = capture.symbolProviders ?? [];
const inventory = capture.symbolInventory ?? [];
const linked = providers.filter((entry) => entry.symbolName === ${JSON.stringify(spec.finding.symbolName)});
const providerRefs = [...new Set(linked.map((entry) => entry.providerRef))].sort();
const fallbackSymbols = inventory
  .filter((entry) => entry.providerRef === ${JSON.stringify(spec.finding.forbiddenProviderRef)})
  .map((entry) => entry.symbolName)
  .sort();
const approvedProviderLinked =
  providerRefs.length === 1 && providerRefs[0] === ${JSON.stringify(spec.finding.approvedProviderRef)};
const fallbackAbsent = fallbackSymbols.length === 0;
const satisfied = approvedProviderLinked && fallbackAbsent;
const report = {
  schema: "openagents.loupe_provider_provenance_report.v1",
  captureRef: capture.captureRef,
  targetCommit: capture.targetCommit,
  sourceBundleDigest: capture.sourceBundleDigest,
  approvedProviderRef: ${JSON.stringify(spec.finding.approvedProviderRef)},
  observedProviderRefs: providerRefs,
  forbiddenProviderRef: ${JSON.stringify(spec.finding.forbiddenProviderRef)},
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

interface SandboxRun {
  readonly role: LoupeVerificationRole;
  readonly variant: LoupeControlVariant;
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

const isoPlus = (from: string, addMillis: number): string =>
  new Date(Date.parse(from) + addMillis).toISOString().replace(/\.(\d{3})\d*Z$/, ".$1Z");

const observation = <Value>(capture: Record<string, unknown>, key: string): Array<Value> =>
  (capture[key] as Array<Value> | undefined) ?? [];

/**
 * Drives one admitted managed sandbox from create to observed delete, and
 * returns only what the control plane and the guest were observed to do.
 */
const runSandbox = async (
  spec: LoupeVerificationSpec,
  transport: LoupeControlPlaneTransport,
  role: LoupeVerificationRole,
  variant: LoupeControlVariant,
  runDetector: boolean,
): Promise<SandboxRun> => {
  const seed = sha256Hex(`${spec.runSeed}|${role}`);
  const suffix = seed.slice(0, 20);
  const sandboxRef = `sandbox.${role.replace(/_/g, "-")}.${suffix}`;
  const capabilityRef = `capability-ref://run/${sha256Hex(`${seed}|capability`).slice(0, 32)}`;
  const placementRef = `placement.loupe.${sha256Hex(`${seed}|placement`).slice(0, 20)}`;
  const pin = spec.targets[variant];
  const scope = {
    actorRef: spec.worker.actorRef,
    ownerRef: spec.worker.ownerRef,
    tenantRef: spec.worker.tenantRef,
    programRef: spec.worker.programRef,
    workUnitRef: `work.loupe.${suffix}`,
    sandboxRef,
  };
  const profile = {
    profileRef: spec.worker.profileRef,
    profileDigest: spec.worker.profileDigest,
    targetRef: "target://openagents/google-cloud/managed-sandbox",
    provisionerRef: spec.worker.provisionerRef,
    region: spec.worker.region,
    machineClass: spec.worker.machineClass,
    isolationClass: "gce_vm",
    imageRef: `gce-image-ref://sha256/${spec.worker.imageDigest.replace("sha256:", "")}`,
    imageDigest: spec.worker.imageDigest,
    networkPolicyRef: spec.worker.networkPolicyRef,
    controlIdentityRef: spec.worker.controlIdentityRef,
    guestIdentityRef: "identity-ref://openagents/managed-sandbox/guest-none",
    ttlMs: spec.limits.leaseMillis,
    capacity: { minCapacity: 0, maxCapacity: 2, prewarmCapacity: 0, concurrentCapacityCap: 2 },
    budget: {
      sandboxBudgetMicrousd: spec.limits.sandboxBudgetMicrousd,
      programBudgetMicrousd: spec.limits.programBudgetMicrousd,
      maxHourlyCostMicrousd: spec.limits.sandboxBudgetMicrousd,
    },
    capabilityRefs: [capabilityRef],
  };

  let generation = 0;
  let phase: string | undefined;
  let counter = 0;
  const receipts: Array<RuntimeReceipt> = [];

  const operate = async (
    action: string,
    expectedGeneration: number,
    includeProfile = false,
  ): Promise<RuntimeReceipt> => {
    counter += 1;
    const operationId = sha256Hex(`${seed}|${counter}|${action}`).slice(0, 32);
    const response = await transport.post("runtime_operation", {
      ...scope,
      operationRef: `operation.loupe.${operationId}`,
      idempotencyRef: `idempotency.loupe.${operationId}`,
      expectedGeneration,
      action,
      ...(includeProfile ? { profile } : {}),
    });
    const body = response.body as RuntimeReceipt & { readonly error?: string };
    if (response.status < 200 || response.status >= 300) {
      throw new Error(`managed-sandbox ${action} refused with ${response.status}: ${body?.error}`);
    }
    if (body.schemaVersion !== RUNTIME_RECEIPT_SCHEMA) {
      throw new Error(`unexpected runtime receipt schema ${String(body.schemaVersion)}`);
    }
    if (body.sandboxRef !== sandboxRef) {
      throw new Error("the control plane answered about a sandbox this verification did not create");
    }
    if (transport.kind !== "conformance" && body.providerKind !== "live_gce") {
      throw new Error(`a live verification requires live_gce workers, observed ${body.providerKind}`);
    }
    receipts.push(body);
    generation = body.generation;
    phase = body.phase;
    return body;
  };

  let ioCounter = 0;
  const guestIo = async (request: Record<string, unknown>): Promise<Record<string, unknown>> => {
    ioCounter += 1;
    const operationId = sha256Hex(`${seed}|io|${ioCounter}`).slice(0, 32);
    const requestedAt = transport.now();
    const response = await transport.post("guest_io", {
      schemaVersion: GUEST_IO_SCHEMA,
      ...scope,
      operationRef: `operation.loupe-io.${operationId}`,
      idempotencyRef: `idempotency.loupe-io.${operationId}`,
      resourceGeneration: generation,
      capabilityRef,
      capabilityState: "active",
      capabilityExpiresAt: isoPlus(requestedAt, 3_600_000),
      requestedAt,
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
    });
    const body = (response.body ?? {}) as Record<string, unknown>;
    if (response.status < 200 || response.status >= 300) {
      throw new Error(
        `guest io ${String(request.action)} refused with ${response.status}: ${JSON.stringify(body).slice(0, 600)}`,
      );
    }
    return body;
  };

  const manifest = [
    "# Coldcard Loupe verification run",
    `role: ${role}`,
    `variant: ${variant}`,
    `commit: ${pin.commitSha}`,
    `tree: ${pin.gitTreeSha}`,
    "",
  ].join("\n");
  const bundlePayload = forensicCanonicalJson({
    schema: SOURCE_BUNDLE_SCHEMA,
    repositoryRef: spec.targets.repositoryRef,
    commitSha: pin.commitSha,
    gitTreeSha: pin.gitTreeSha,
    entries: [
      {
        path: "RUN.md",
        contentDigest: contentDigest(manifest),
        contentBase64: Buffer.from(manifest, "utf8").toString("base64"),
      },
    ],
  });
  const bundle = {
    base64: Buffer.from(bundlePayload, "utf8").toString("base64"),
    digest: contentDigest(bundlePayload),
  };

  const guaranteedCleanup = async (): Promise<void> => {
    try {
      if (phase === undefined) await operate("reconcile", 1);
      if (phase === "ready") await operate("stop", generation);
      if (phase !== undefined && phase !== "deleted") {
        if (!["stopped", "failed", "recovery_required", "deleting"].includes(phase)) {
          await operate("reconcile", generation);
        }
        if (String(phase) !== "deleted") await operate("delete", generation);
      }
    } catch {
      try {
        await operate("reconcile", Math.max(generation, 1));
      } catch {
        // The caller's out-of-band residue audit is what fails the run.
      }
    }
  };

  let result: SandboxRun | undefined;
  let failure: string | undefined;
  try {
    const create = await operate("create", 0, true);
    const leaseObservedAt = transport.now();
    if (create.phase !== "ready" || !create.readinessObserved) {
      throw new Error("create did not produce observed ready state");
    }
    if (create.imageDigest !== spec.worker.imageDigest) {
      throw new Error("control plane admitted a different guest image than requested");
    }
    if (create.profileDigest !== spec.worker.profileDigest) {
      throw new Error("control plane admitted a different worker profile than requested");
    }
    if (create.isolationClass !== "gce_vm") {
      throw new Error(`unexpected isolation class ${create.isolationClass}`);
    }

    await guestIo({
      action: "install_forensic_source",
      artifactRef: `artifact.loupe.${sha256Hex(bundle.base64).slice(0, 32)}`,
      artifactContentBase64: bundle.base64,
      artifactContentDigest: bundle.digest,
      sourcePath: "workspace/source",
      scratchPath: "workspace/scratch",
    });

    const buildCommand = [
      `OA_COLDCARD_WORKER_PROFILE_DIGEST=${create.profileDigest}`,
      `OA_COLDCARD_PLACEMENT_REF=${placementRef}`,
      "node",
      spec.guestDriverPath,
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
      commandDigest: contentDigest(buildCommand),
      cwd: "workspace",
      timeoutMillis: spec.limits.buildTimeoutMillis,
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
      retentionUntil: isoPlus(transport.now(), 86_400_000),
    });
    const capture = JSON.parse(
      Buffer.from(String(artifact.contentBase64), "base64").toString("utf8"),
    ) as Record<string, unknown>;
    if (capture.schema !== CAPTURE_SCHEMA) {
      throw new Error(`guest produced an unexpected capture schema ${String(capture.schema)}`);
    }
    if (capture.variant !== variant) throw new Error("guest capture variant mismatch");
    if (capture.targetCommit !== pin.commitSha) {
      throw new Error("guest capture did not build the pinned commit");
    }

    let detector: SandboxRun["detector"];
    if (runDetector) {
      const source = detectorSource(spec);
      const install = [
        "printf",
        "%s",
        `'${Buffer.from(source, "utf8").toString("base64")}'`,
        "|",
        "base64",
        "-d",
        ">",
        "/workspace/scratch/detector.cjs",
      ].join(" ");
      const installed = await guestIo({
        action: "execute_command",
        command: install,
        commandDigest: contentDigest(install),
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
        commandDigest: contentDigest(detectCommand),
        cwd: "workspace",
        timeoutMillis: 300_000,
      });
      if (detected.timedOut === true) throw new Error("guest detector timed out");
      const detectorObservedAt = transport.now();
      const exitStatus = Number(detected.exitCode ?? -1);
      const report = JSON.parse(
        String(detected.stdout).trim().split("\n").at(-1) ?? "{}",
      ) as Record<string, unknown>;
      let reportDigest: string | undefined;
      if (exitStatus === 0) {
        const reportArtifact = await guestIo({
          action: "read_artifact",
          path: "workspace/scratch/report.json",
          retentionUntil: isoPlus(transport.now(), 86_400_000),
        });
        const reportContent = reportArtifact.contentBase64;
        // A guest that exited zero and kept nothing has produced no result to
        // digest. `deriveControlTestOutcome` reads that as `not_observed`,
        // never as a pass.
        if (typeof reportContent === "string" && reportContent.length > 0) {
          reportDigest = contentDigest(Buffer.from(reportContent, "base64"));
        }
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
    if (imageDigests.size !== 1 || !imageDigests.has(spec.worker.imageDigest)) {
      throw new Error("guest image digest was not stable across this run");
    }

    result = {
      role,
      variant,
      workerRef: `worker.loupe.${role.replace(/_/g, "-")}.${suffix}`,
      sandboxRef,
      resourceGeneration: create.generation,
      placementRef,
      createReceiptRef: create.receiptRef,
      receiptRefs: receipts.map((receipt) => receipt.receiptRef),
      leaseObservedAt,
      leaseExpiresAt: isoPlus(leaseObservedAt, spec.limits.leaseMillis),
      measuredCostMicrousd: deleted.measuredCostMicrousd,
      capture,
      captureDigest: forensicSha256Digest(capture),
      buildExitStatus,
      ...(detector === undefined ? {} : { detector }),
    };
  } catch (error) {
    failure = error instanceof Error ? error.message : String(error);
  } finally {
    await guaranteedCleanup();
  }
  if (result === undefined) throw new Error(`${role} run failed: ${failure ?? "unknown"}`);
  return result;
};

export interface RunLoupeVerificationOptions {
  readonly spec: LoupeVerificationSpec;
  readonly controlPlane: LoupeControlPlaneTransport;
  /** Directory of the adapter-owned durable first-verdict ledger. */
  readonly ledgerDirectory: string;
  readonly onPhase?: (phase: string) => void;
}

/**
 * Runs one Loupe verification end to end against a managed-sandbox control
 * plane, in the only order that means anything:
 *
 *   1. MECHANICAL — build the pinned vulnerable tree on an admitted sandbox and
 *      read the source-ref, macro-value and symbol-provider observations out of
 *      what the guest measured.
 *   2. LOCK — commit the initial verdict to the adapter-owned durable ledger.
 *      `O_EXCL` makes it exactly-once, and it happens before any control
 *      exists.
 *   3. CONTROLS — only then build the vulnerable and fixed trees on two further
 *      admitted sandboxes and run the detector inside each guest.
 *
 * The caller supplies a spec, a transport and a ledger directory. It supplies
 * no evidence, no worker receipt, no verdict and no lifecycle authority.
 */
export const runLoupeVerification = async (
  options: RunLoupeVerificationOptions,
): Promise<LoupeVerificationRun> => {
  const { spec, controlPlane: transport } = options;
  const ledger = durableFirstVerdictLedger(options.ledgerDirectory);
  const evidenceProvenance = provenanceOf(transport);
  const detectorDigest = contentDigest(detectorSource(spec));
  const environmentDigest = forensicSha256Digest({
    workerImageDigest: spec.worker.imageDigest,
    workerProfileDigest: spec.worker.profileDigest,
  });
  const findingDigest = forensicSha256Digest({
    claim: spec.finding.claim,
    approvedProviderRef: spec.finding.approvedProviderRef,
    fallbackProviderRef: spec.finding.forbiddenProviderRef,
    symbolName: spec.finding.symbolName,
    vulnerableCommit: spec.targets.vulnerable.commitSha,
    fixedCommit: spec.targets.fixed.commitSha,
  });

  // 1. MECHANICAL
  options.onPhase?.("mechanical");
  const mechanical = await runSandbox(spec, transport, "mechanical", "vulnerable", false);
  const macro = observation<{
    readonly macroName: string;
    readonly value: string;
  }>(mechanical.capture, "macroObservations").find(
    (entry) => entry.macroName === spec.finding.guardMacroName,
  );
  const provider = observation<{
    readonly symbolName: string;
    readonly providerRef: string;
  }>(mechanical.capture, "symbolProviders").find(
    (entry) => entry.symbolName === spec.finding.symbolName,
  );
  if (macro === undefined) throw new Error("mechanical capture observed no guard macro");
  if (provider === undefined) throw new Error("mechanical capture resolved no symbol provider");

  const vulnerableTargetDigest = String(mechanical.capture.sourceBundleDigest);
  const dependencyManifestDigest = String(mechanical.capture.buildConfigurationDigest);
  const mechanicalObservedAt = String(mechanical.capture.capturedAt);
  const mechanicalReceipt = (
    sequence: number,
    operation: string,
    subjectRef: string,
    commandInput: string,
    resultRecord: unknown,
  ) => ({
    schema: LOUPE_VERIFICATION_EVIDENCE_VERSION,
    receiptRef: `receipt.loupe.mechanical.${sequence}.${sha256Hex(`${mechanical.sandboxRef}|${sequence}`).slice(0, 16)}`,
    verificationRef: spec.verificationRef,
    sequence,
    operation,
    evidenceTier: "source_observed",
    subjectRef,
    commandDigest: contentDigest(commandInput),
    inputDigests: [vulnerableTargetDigest, dependencyManifestDigest],
    outcome: "succeeded",
    resultDigest: forensicSha256Digest(resultRecord),
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
      `git-tree ${spec.targets.vulnerable.gitTreeSha}`,
      {
        targetCommit: mechanical.capture.targetCommit,
        sourceBundleDigest: mechanical.capture.sourceBundleDigest,
        targetSnapshotRef: mechanical.capture.targetSnapshotRef,
      },
    ),
    mechanicalReceipt(2, "macro_value_observed", macro.macroName, "arm-none-eabi-gcc -E", macro),
    mechanicalReceipt(
      3,
      "symbol_provider_resolved",
      provider.symbolName,
      "arm-none-eabi-nm",
      provider,
    ),
  ];

  // The finding under verification says the vulnerable tree links the forbidden
  // object. If the measurement disagrees, the honest first verdict is not
  // `confirmed`, and this run must not manufacture one.
  const initialOutcome =
    provider.providerRef === spec.finding.forbiddenProviderRef ? "confirmed" : "dismissed";

  // 2. LOCK
  options.onPhase?.("lock");
  const lockedAt = transport.now();
  const draft: LoupeInitialVerdictDraft = {
    verdictRef: `verdict.loupe.${sha256Hex(`${spec.verificationRef}|${lockedAt}`).slice(0, 24)}`,
    outcome: initialOutcome,
    rationaleDigest: forensicSha256Digest({ macro, provider }),
    lockedAt,
  };
  const candidateVerdict = {
    schema: LOUPE_INITIAL_VERDICT_VERSION,
    verdictRef: draft.verdictRef,
    verificationRef: spec.verificationRef,
    findingDigest,
    verifierActorRef: spec.verifierActorRef,
    outcome: draft.outcome,
    evidenceReceiptRefs: mechanicalEvidence.map((receipt) => receipt.receiptRef),
    rationaleDigest: draft.rationaleDigest,
    lockedAt: draft.lockedAt,
  };
  const storedVerdict = await ledger.commit(spec.verificationRef, candidateVerdict);
  if (forensicSha256Digest(storedVerdict) !== forensicSha256Digest(candidateVerdict)) {
    throw new Error(
      "an initial verdict is already durably locked for this verification and cannot be relocked",
    );
  }

  // 3. CONTROLS
  const controlEvidence: Array<Record<string, unknown>> = [];
  const runs: Array<SandboxRun> = [mechanical];
  let fixedTargetDigest: string | undefined;
  if (initialOutcome === "confirmed") {
    options.onPhase?.("control_vulnerable");
    const vulnerableControl = await runSandbox(
      spec,
      transport,
      "control_vulnerable",
      "vulnerable",
      true,
    );
    options.onPhase?.("control_fixed");
    const fixedControl = await runSandbox(spec, transport, "control_fixed", "fixed", true);
    runs.push(vulnerableControl, fixedControl);
    fixedTargetDigest = String(fixedControl.capture.sourceBundleDigest);
    if (fixedTargetDigest === vulnerableTargetDigest) {
      throw new Error("the vulnerable and fixed controls resolved to the same immutable target");
    }
    const controlReceipt = (
      run: SandboxRun,
      revision: LoupeControlVariant,
      sequence: number,
      targetDigest: string,
    ) => ({
      schema: LOUPE_VERIFICATION_EVIDENCE_VERSION,
      receiptRef: `receipt.loupe.control.${revision}.${sha256Hex(run.sandboxRef).slice(0, 16)}`,
      verificationRef: spec.verificationRef,
      sequence,
      operation: "control_test_observed",
      evidenceTier: "executed",
      subjectRef: String(run.capture.targetSnapshotRef),
      commandDigest: detectorDigest,
      inputDigests: [targetDigest],
      outcome: "succeeded",
      resultDigest: forensicSha256Digest(run.detector?.report ?? {}),
      environmentDigest,
      workerRef: run.workerRef,
      workerReceiptRef: run.createReceiptRef,
      controlRevision: revision,
      expectedTestOutcome: revision === "vulnerable" ? "failure" : "success",
      observedTermination: {
        status: "observed",
        exitStatus: run.detector?.exitStatus ?? -1,
        ...(run.detector?.reportDigest === undefined
          ? {}
          : { resultArtifactDigest: run.detector.reportDigest }),
      },
      observedAt: run.detector?.observedAt ?? transport.now(),
    });
    controlEvidence.push(
      {
        schema: LOUPE_VERIFICATION_EVIDENCE_VERSION,
        receiptRef: `receipt.loupe.poc.${sha256Hex(`${vulnerableControl.sandboxRef}|poc`).slice(0, 16)}`,
        verificationRef: spec.verificationRef,
        sequence: 4,
        operation: "poc_applied",
        evidenceTier: "artifact_observed",
        subjectRef: "detector.coldcard.provider-provenance.v1",
        commandDigest: detectorDigest,
        inputDigests: [
          vulnerableTargetDigest,
          String(vulnerableControl.capture.buildConfigurationDigest),
        ],
        outcome: "succeeded",
        resultDigest: detectorDigest,
        environmentDigest,
        workerRef: vulnerableControl.workerRef,
        workerReceiptRef: vulnerableControl.createReceiptRef,
        observedAt: vulnerableControl.detector?.observedAt ?? transport.now(),
      },
      controlReceipt(vulnerableControl, "vulnerable", 5, vulnerableTargetDigest),
      controlReceipt(fixedControl, "fixed", 6, fixedTargetDigest),
    );
  }

  const admittedWorkerReceipts = runs.map((run) => ({
    schema: LOUPE_ADMITTED_WORKER_RECEIPT_VERSION,
    receiptRef: run.createReceiptRef,
    sandboxRef: run.sandboxRef,
    resourceGeneration: run.resourceGeneration,
    placementRef: run.placementRef,
    imageDigest: spec.worker.imageDigest,
    profileDigest: spec.worker.profileDigest,
    lifecycleState: "admitted",
    exact: true,
    observedAt: run.leaseObservedAt,
    expiresAt: run.leaseExpiresAt,
  }));

  const plan = {
    schema: LOUPE_VERIFICATION_PLAN_VERSION,
    verificationRef: spec.verificationRef,
    runRef: spec.runRef,
    findingRef: spec.findingRef,
    findingDigest,
    discoveryActorRef: spec.discoveryActorRef,
    verifierActorRef: spec.verifierActorRef,
    evidenceProvenance,
    sourceBundleRef: String(mechanical.capture.targetSnapshotRef),
    sourceBundleDigest: vulnerableTargetDigest,
    dependencyManifestDigest,
    vulnerableTargetDigest,
    ...(fixedTargetDigest === undefined ? {} : { fixedTargetDigest }),
    workerImageDigest: spec.worker.imageDigest,
    workerProfileDigest: spec.worker.profileDigest,
    admittedWorkers: runs.map((run) => ({
      workerRef: run.workerRef,
      sandboxRef: run.sandboxRef,
      resourceGeneration: run.resourceGeneration,
      placementRef: run.placementRef,
    })),
    createdAt: mechanicalObservedAt,
  };

  const session: LoupeVerificationSession = {
    [LOUPE_VERIFICATION_SESSION_BRAND]: true,
    evidenceProvenance,
    originRef: transport.originRef,
    plan,
    collectMechanicalEvidence: async () => mechanicalEvidence,
    submitInitialVerdict: async () => draft,
    applyPocAndRunControls: async () => controlEvidence,
    resolveAdmittedWorkerReceipt: async (workerReceiptRef) =>
      admittedWorkerReceipts.find((receipt) => receipt.receiptRef === workerReceiptRef),
    commitInitialVerdict: async (candidate) => await ledger.commit(spec.verificationRef, candidate),
  };

  const completedAt = transport.now();
  const result = await evaluateLoupeVerificationSession(session, completedAt);

  return {
    plan,
    result,
    initialVerdict: candidateVerdict,
    mechanicalEvidence,
    controlEvidence,
    admittedWorkerReceipts,
    detectorRef: "detector.coldcard.provider-provenance.v1",
    detectorDigest,
    completedAt,
    totalMeasuredCostMicrousd: runs.reduce((total, run) => total + run.measuredCostMicrousd, 0),
    runs: runs.map((run) => ({
      role: run.role,
      variant: run.variant,
      sandboxRef: run.sandboxRef,
      workerRef: run.workerRef,
      resourceGeneration: run.resourceGeneration,
      receiptRefs: run.receiptRefs,
      capturedAt: String(run.capture.capturedAt),
      captureDigest: run.captureDigest,
      targetCommit: String(run.capture.targetCommit),
      sourceBundleDigest: String(run.capture.sourceBundleDigest),
      buildExitStatus: run.buildExitStatus,
      measuredCostMicrousd: run.measuredCostMicrousd,
      ...(run.detector === undefined
        ? {}
        : {
            detectorExitStatus: run.detector.exitStatus,
            detectorObservedAt: run.detector.observedAt,
            detectorReport: run.detector.report,
          }),
    })),
  };
};
