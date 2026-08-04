import { createHash } from "node:crypto";

import { forensicSha256Digest } from "@openagentsinc/forensic-contract";

import {
  conformanceLoupeControlPlane,
  type LoupeControlPlaneResponse,
  type LoupeControlPlaneRoute,
  type LoupeControlPlaneTransport,
  type LoupeVerificationSpec,
} from "../src/verifier.ts";

/**
 * An in-memory managed sandbox.
 *
 * It answers the same wire shapes the real control plane answers, so the
 * verifier's driver runs unmodified against it: create/stop/delete runtime
 * receipts, guest source installation, a guest build that emits an artifact
 * witness capture, and the detector the verifier installs into the guest.
 *
 * It is a simulation, and `conformanceLoupeControlPlane` labels it as one, so
 * nothing derived through it can leave the verifier confirmed or independently
 * verified. It exists to falsify the driver's ordering, derivation and refusal
 * behaviour without spending money.
 */

const hex = (value: string) => createHash("sha256").update(value).digest("hex");

export const SPEC_IMAGE_DIGEST = `sha256:${hex("simulated-image")}`;
export const SPEC_PROFILE_DIGEST = `sha256:${hex("simulated-profile")}`;

export const APPROVED_PROVIDER_REF = "provider.object.boards/COLDCARD_MK4/rng.o";
export const FORBIDDEN_PROVIDER_REF = "provider.object.rng.o";

export const simulationSpec = (
  overrides: Partial<LoupeVerificationSpec> = {},
): LoupeVerificationSpec => ({
  verificationRef: "verification.simulated.rng.v1",
  runRef: "run.simulated.rng.v1",
  findingRef: "finding.coldcard.rng-fallback.v1",
  discoveryActorRef: "actor.discovery.simulated.v1",
  verifierActorRef: "actor.verifier.simulated.v1",
  runSeed: "seed.simulated.v1",
  finding: {
    claim: "the pinned vulnerable tree links the deterministic fallback rng_get",
    symbolName: "symbol.linked.rng_get",
    guardMacroName: "macro.MICROPY_HW_ENABLE_RNG",
    approvedProviderRef: APPROVED_PROVIDER_REF,
    forbiddenProviderRef: FORBIDDEN_PROVIDER_REF,
  },
  targets: {
    repositoryRef: "repository.github.Coldcard.firmware",
    vulnerable: {
      commitSha: "bcc2c382a324690a2fcf972c0bac3b79bf923f7b",
      gitTreeSha: "7abc9a4c680b5623fc8a64f70555dd2d3802e488",
    },
    fixed: {
      commitSha: "ca72463709f4e3f8964952039d5caf955f566a87",
      gitTreeSha: "51f735fb2dd888865c81ea2ed1fc690a0ed5b396",
    },
  },
  worker: {
    imageDigest: SPEC_IMAGE_DIGEST,
    profileDigest: SPEC_PROFILE_DIGEST,
    profileRef: "profile.managed-sandbox.simulated.v1",
    provisionerRef: "provisioner.simulated.v1",
    region: "us-central1",
    machineClass: "n2-standard-8",
    networkPolicyRef: "network-policy.managed-sandbox.deny-all",
    controlIdentityRef: "identity-ref://openagents/managed-sandbox/control",
    actorRef: "principal.owner.simulated",
    ownerRef: "owner.openagents.primary",
    tenantRef: "tenant.openagents.primary",
    programRef: "program.managed-agent-sandboxes.simulated",
  },
  limits: {
    buildTimeoutMillis: 600_000,
    leaseMillis: 3_600_000,
    sandboxBudgetMicrousd: 40_000,
    programBudgetMicrousd: 160_000,
  },
  guestDriverPath: "/opt/openagents-managed-sandbox/coldcard-build-driver.mjs",
  ...overrides,
});

export interface SimulationOptions {
  readonly spec?: LoupeVerificationSpec;
  readonly startedAt?: string;
  /** Which object the simulated linker resolved the finding's symbol to. */
  readonly providerRefFor?: (variant: "vulnerable" | "fixed") => string;
  /** How many forbidden-provider symbols the simulated inventory contains. */
  readonly forbiddenSymbolsFor?: (variant: "vulnerable" | "fixed") => number;
  /** Last-word hook for falsifiers: rewrite any response before the driver sees it. */
  readonly perturb?: (
    context: {
      readonly route: LoupeControlPlaneRoute;
      readonly action: string;
      readonly sandboxRef: string;
      readonly command?: string;
      readonly path?: string;
    },
    response: LoupeControlPlaneResponse,
  ) => LoupeControlPlaneResponse;
}

const variantOfSandbox = (sandboxRef: string): "vulnerable" | "fixed" =>
  sandboxRef.includes("control-fixed") ? "fixed" : "vulnerable";

export const simulatedControlPlane = (
  options: SimulationOptions = {},
): LoupeControlPlaneTransport => {
  const spec = options.spec ?? simulationSpec();
  const providerRefFor =
    options.providerRefFor ??
    ((variant) => (variant === "fixed" ? APPROVED_PROVIDER_REF : FORBIDDEN_PROVIDER_REF));
  const forbiddenSymbolsFor =
    options.forbiddenSymbolsFor ?? ((variant) => (variant === "fixed" ? 0 : 6));
  const generations = new Map<string, number>();
  const reports = new Map<string, string>();

  let transport: LoupeControlPlaneTransport;

  const capture = (sandboxRef: string, variant: "vulnerable" | "fixed") => {
    const pin = spec.targets[variant];
    const inventory = [
      { symbolName: "symbol.linked.main", providerRef: "provider.object.main.o" },
      ...Array.from({ length: forbiddenSymbolsFor(variant) }, (_, index) => ({
        symbolName: `symbol.linked.fallback_${index}`,
        providerRef: FORBIDDEN_PROVIDER_REF,
      })),
    ];
    return {
      schema: "openagents.artifact_witness_capture.v2",
      captureRef: `capture.simulated.${hex(sandboxRef).slice(0, 20)}`,
      variant,
      targetCommit: pin.commitSha,
      capturedAt: transport.now(),
      targetSnapshotRef: `snapshot.simulated.${variant}.v1`,
      sourceBundleDigest: forensicSha256Digest({ bundle: variant, commit: pin.commitSha }),
      buildConfigurationDigest: forensicSha256Digest({ buildConfiguration: "simulated" }),
      macroObservations: [
        {
          macroName: spec.finding.guardMacroName,
          value: variant === "fixed" ? "1" : "0",
          preprocessedArtifactRef: `artifact.simulated.preprocessed.${variant}`,
        },
      ],
      symbolProviders: [
        {
          symbolName: spec.finding.symbolName,
          providerRef: providerRefFor(variant),
          linkMapArtifactRef: `artifact.simulated.linkmap.${variant}`,
        },
      ],
      symbolInventory: inventory,
    };
  };

  const runtimeOperation = (body: Record<string, unknown>): LoupeControlPlaneResponse => {
    const sandboxRef = String(body.sandboxRef);
    const action = String(body.action);
    const generation = action === "create" ? 1 : (generations.get(sandboxRef) ?? 1);
    generations.set(sandboxRef, generation);
    const phase =
      action === "create" || action === "resume"
        ? "ready"
        : action === "stop"
          ? "stopped"
          : action === "delete"
            ? "deleted"
            : "ready";
    return {
      status: 200,
      body: {
        schemaVersion: "openagents.managed_sandbox_runtime.v1",
        receiptRef: `receipt.managed-sandbox.${hex(`${sandboxRef}|${String(body.operationRef)}`).slice(0, 32)}`,
        action,
        sandboxRef,
        generation,
        phase,
        profileDigest: spec.worker.profileDigest,
        imageDigest: spec.worker.imageDigest,
        isolationClass: "gce_vm",
        providerKind: "conformance_sim",
        readinessObserved: phase === "ready",
        cleanupObserved: action === "delete",
        measuredCostMicrousd: action === "delete" ? 5_000 : 0,
        sandboxBudgetMicrousd: spec.limits.sandboxBudgetMicrousd,
      },
    };
  };

  const guestIo = (body: Record<string, unknown>): LoupeControlPlaneResponse => {
    const sandboxRef = String(body.sandboxRef);
    const variant = variantOfSandbox(sandboxRef);
    const action = String(body.action);
    if (action === "install_forensic_source" || action === "remove_forensic_source") {
      return { status: 200, body: { accepted: true } };
    }
    if (action === "read_artifact") {
      const path = String(body.path);
      if (path.endsWith("report.json")) {
        const stored = reports.get(sandboxRef) ?? "{}";
        return {
          status: 200,
          body: { contentBase64: Buffer.from(stored, "utf8").toString("base64") },
        };
      }
      return {
        status: 200,
        body: {
          contentBase64: Buffer.from(
            JSON.stringify(capture(sandboxRef, variant)),
            "utf8",
          ).toString("base64"),
        },
      };
    }
    if (action === "execute_command") {
      const command = String(body.command);
      if (command.includes("detector.cjs") && command.startsWith("node ")) {
        // The simulated guest runs the same decision the detector runs: the
        // approved object must be the sole provider and the forbidden object
        // must contribute no symbol at all.
        const approvedLinked = providerRefFor(variant) === spec.finding.approvedProviderRef;
        const fallbackAbsent = forbiddenSymbolsFor(variant) === 0;
        const satisfied = approvedLinked && fallbackAbsent;
        const report = {
          schema: "openagents.loupe_provider_provenance_report.v1",
          observedProviderRefs: [providerRefFor(variant)],
          observedForbiddenSymbolCount: forbiddenSymbolsFor(variant),
          enumeratedSymbolCount: forbiddenSymbolsFor(variant) + 1,
          approvedProviderLinked: approvedLinked,
          fallbackAbsent,
          satisfied,
        };
        if (satisfied) reports.set(sandboxRef, JSON.stringify(report));
        return {
          status: 200,
          body: { exitCode: satisfied ? 0 : 1, stdout: `${JSON.stringify(report)}\n`, stderr: "" },
        };
      }
      return { status: 200, body: { exitCode: 0, stdout: "", stderr: "" } };
    }
    return { status: 400, body: { error: `unsupported guest io action ${action}` } };
  };

  transport = conformanceLoupeControlPlane({
    startedAt: options.startedAt ?? "2026-08-01T16:00:00.000Z",
    handle: (route, body) => {
      const response = route === "runtime_operation" ? runtimeOperation(body) : guestIo(body);
      if (options.perturb === undefined) return response;
      return options.perturb(
        {
          route,
          action: String(body.action ?? ""),
          sandboxRef: String(body.sandboxRef ?? ""),
          ...(typeof body.command === "string" ? { command: body.command } : {}),
          ...(typeof body.path === "string" ? { path: body.path } : {}),
        },
        response,
      );
    },
  });
  return transport;
};

/** Rewrites one field of a runtime receipt, for falsifiers. */
export const overrideRuntimeReceipt =
  (
    match: (context: { readonly action: string; readonly sandboxRef: string }) => boolean,
    overrides: Record<string, unknown>,
  ): NonNullable<SimulationOptions["perturb"]> =>
  (context, response) =>
    context.route === "runtime_operation" && match(context)
      ? { ...response, body: { ...(response.body as Record<string, unknown>), ...overrides } }
      : response;
