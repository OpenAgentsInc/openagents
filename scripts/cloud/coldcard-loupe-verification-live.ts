#!/usr/bin/env -S pnpm exec tsx

/**
 * Owner-gated live Loupe verification run (openagents #9294, OFR-007).
 *
 * This script no longer orchestrates the verification. It supplies a spec, an
 * authenticated managed-sandbox control-plane transport, and a ledger
 * directory, and `@openagentsinc/forensic-loupe-adapter/verifier` does the rest:
 * it drives the sandboxes itself, derives every evidence receipt and every
 * admitted-worker receipt from what the control plane returned to its own
 * calls, and locks its first verdict in an `O_EXCL` ledger before any control
 * exists.
 *
 * That is the point of the change. Previously this script measured everything
 * and handed the verifier a finished story plus the authority that validated
 * it. It cannot do that now: there is nowhere to put it.
 *
 * What this script still owns is what belongs outside the verifier: the owner
 * gate, the credential, and an out-of-band residue audit that asks Google Cloud
 * directly whether any instance, disk or firewall rule survived the run rather
 * than trusting a delete receipt.
 */

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { gzipSync, gunzipSync } from "node:zlib";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

import {
  httpLoupeControlPlane,
  recordedLoupeControlPlane,
  recordingLoupeControlPlane,
  runLoupeVerification,
  type LoupeControlPlaneTranscript,
  type LoupeVerificationSpec,
} from "../../packages/forensic-loupe-adapter/src/verifier.ts";

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex");

const required = (name: string): string => {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`missing required environment variable ${name}`);
  return value;
};

const args = process.argv.slice(2);
const option = (name: string): string | undefined => {
  const index = args.indexOf(`--${name}`);
  return index >= 0 ? args[index + 1] : undefined;
};
/**
 * Replays an already-recorded transcript instead of provisioning anything, so
 * the derived summary beside it can be regenerated without spending. The
 * verifier does the same derivation either way; only the transport differs.
 */
const replayOnly = args.includes("--replay");
if (
  !replayOnly &&
  (!args.includes("--apply") ||
    process.env.OA_MANAGED_SANDBOX_OWNER_GATE !== "I_ACCEPT_LIVE_GCP_COST")
) {
  throw new Error(
    "live Loupe verification runs are default-off; pass --apply and set OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST",
  );
}

const ledgerDirectory = resolve(
  option("ledger") ?? "fixtures/forensics/coldcard/loupe-first-verdict-ledger",
);
const transcriptPath = resolve(
  option("transcript") ?? "fixtures/forensics/coldcard/loupe-control-plane-transcript.v1.json.gz",
);
const summaryPath = resolve(
  option("out") ?? "fixtures/forensics/coldcard/loupe-verification-live-run.v2.json",
);

const projectId = required("OA_MANAGED_SANDBOX_PROJECT_ID");
const zone = required("OA_MANAGED_SANDBOX_ZONE");
const gcloud = process.env.OA_MANAGED_SANDBOX_GCLOUD_BIN?.trim() || "gcloud";

const spec: LoupeVerificationSpec = {
  verificationRef: option("verification-ref") ?? "verification.coldcard.rng-fallback.live.v2",
  runRef: option("run-ref") ?? "run.coldcard.loupe-verification.v2",
  findingRef: "finding.coldcard.rng-fallback.v1",
  discoveryActorRef: "actor.discovery.ofr014.artifact-witness.v1",
  verifierActorRef: "actor.verifier.ofr007.loupe.v2",
  runSeed: option("run-seed") ?? "seed.ofr007.live.v2",
  finding: {
    claim:
      "the pinned vulnerable Coldcard MK4 build links MicroPython's deterministic fallback rng_get",
    symbolName: "symbol.linked.rng_get",
    guardMacroName: "macro.MICROPY_HW_ENABLE_RNG",
    approvedProviderRef: "provider.object.boards/COLDCARD_MK4/rng.o",
    forbiddenProviderRef: "provider.object.rng.o",
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
    imageDigest: required("OA_MANAGED_SANDBOX_IMAGE_DIGEST"),
    profileDigest: required("OA_MANAGED_SANDBOX_PROFILE_DIGEST"),
    profileRef: required("OA_MANAGED_SANDBOX_PROFILE_REF"),
    provisionerRef: required("OA_MANAGED_SANDBOX_PROVISIONER_REF"),
    region: required("OA_MANAGED_SANDBOX_REGION"),
    machineClass: required("OA_MANAGED_SANDBOX_MACHINE_CLASS"),
    networkPolicyRef: required("OA_MANAGED_SANDBOX_NETWORK_POLICY_REF"),
    controlIdentityRef: required("OA_MANAGED_SANDBOX_CONTROL_IDENTITY_REF"),
    actorRef: "principal.owner.ofr007-loupe-verification",
    ownerRef: "owner.openagents.primary",
    tenantRef: "tenant.openagents.primary",
    programRef: "program.managed-agent-sandboxes.ofr007",
  },
  limits: {
    buildTimeoutMillis: Number(process.env.OA_COLDCARD_BUILD_TIMEOUT_MS ?? 3_000_000),
    leaseMillis: 60 * 60 * 1_000,
    sandboxBudgetMicrousd: 40_000,
    programBudgetMicrousd: 160_000,
  },
  guestDriverPath: "/opt/openagents-managed-sandbox/coldcard-build-driver.mjs",
};

const recording = replayOnly
  ? undefined
  : recordingLoupeControlPlane(
      httpLoupeControlPlane({
        baseUrl: required("OA_MANAGED_SANDBOX_BASE_URL"),
        token: required("OA_CODEX_CONTROL_TOKEN"),
        timeoutMillis: spec.limits.buildTimeoutMillis + 600_000,
      }),
    );
const recorded = replayOnly
  ? (JSON.parse(
      gunzipSync(readFileSync(transcriptPath)).toString("utf8"),
    ) as LoupeControlPlaneTranscript)
  : undefined;

const count = (collection: "instances" | "firewall-rules" | "disks", filter: string): number => {
  const commandArgs = ["compute", collection, "list", "--project", projectId];
  if (collection !== "firewall-rules") commandArgs.push("--zones", zone);
  commandArgs.push("--filter", filter, "--format", "value(name)");
  const output = execFileSync(gcloud, commandArgs, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  return output.split("\n").filter((line) => line.trim().length > 0).length;
};

/**
 * Asks Google Cloud what survived, rather than believing the delete receipt.
 * The verifier already refuses a delete that did not report observed cleanup;
 * this is the independent second opinion, and it runs even when the run failed.
 */
const residueFor = (sandboxRefs: ReadonlyArray<string>): number => {
  let residue = 0;
  for (const sandboxRef of sandboxRefs) {
    const suffix = sha256(sandboxRef).slice(0, 20);
    residue += count("instances", `name=oa-msb-${suffix}`);
    residue += count("disks", `name=oa-msb-${suffix}`);
    for (const kind of ["egress", "broker", "metadata", "ssh", "ingress"]) {
      residue += count("firewall-rules", `name=oa-msb-${kind}-${suffix}`);
    }
  }
  return residue;
};

// The driver derives its sandbox refs from the run seed, so the residue audit
// can name the exact resources before the run starts and check them after it
// ends, whether it succeeded or threw.
const expectedSandboxRefs = (["mechanical", "control_vulnerable", "control_fixed"] as const).map(
  (role) => `sandbox.${role.replace(/_/g, "-")}.${sha256(`${spec.runSeed}|${role}`).slice(0, 20)}`,
);

let failure: string | undefined;
let run: Awaited<ReturnType<typeof runLoupeVerification>> | undefined;
try {
  run = await runLoupeVerification({
    spec,
    controlPlane:
      recorded === undefined ? recording!.transport : recordedLoupeControlPlane(recorded),
    ledgerDirectory,
    onPhase: (phase) => process.stderr.write(`[phase] ${phase} ${new Date().toISOString()}\n`),
  });
} catch (error) {
  failure = error instanceof Error ? error.message : String(error);
}

const transcript = recorded ?? recording!.transcript();
if (recorded === undefined) {
  mkdirSync(dirname(transcriptPath), { recursive: true });
  writeFileSync(transcriptPath, gzipSync(Buffer.from(`${JSON.stringify(transcript)}\n`, "utf8")));
}

const residue = residueFor(expectedSandboxRefs);
if (run !== undefined) {
  mkdirSync(dirname(summaryPath), { recursive: true });
  writeFileSync(
    summaryPath,
    `${JSON.stringify(
      {
        schema: "openagents.loupe_verification_live_run.v2",
        note: "Derived summary of a live OFR-007 verification. The verifier drove three admitted OpenAgents Cloud managed sandboxes itself over one authenticated control-plane transport. Nothing here was supplied to the verifier: the plan, evidence, worker receipts and result were all derived from the control-plane responses recorded in the transcript beside this file.",
        transcriptFile: transcriptPath.split("/").at(-1),
        transcriptDigest: `sha256:${sha256(JSON.stringify(transcript))}`,
        recordedOriginRef: transcript.recordedOriginRef,
        exchangeCount: transcript.exchanges.length,
        detectorRef: run.detectorRef,
        detectorDigest: run.detectorDigest,
        // The intent the verifier was given. Replaying the transcript with a
        // different spec fails, because the driver would issue different
        // requests and the recorded control plane matches on request identity.
        spec,
        plan: run.plan,
        initialVerdict: run.initialVerdict,
        result: run.result,
        runs: run.runs,
        completedAt: run.completedAt,
        totalMeasuredCostMicrousd: run.totalMeasuredCostMicrousd,
        cloudResidue: residue,
      },
      null,
      2,
    )}\n`,
  );
}

process.stdout.write(
  `${JSON.stringify({
    passed: failure === undefined && residue === 0,
    failure,
    residue,
    transcriptPath,
    summaryPath: run === undefined ? undefined : summaryPath,
    ledgerDirectory,
    outcome: run?.result.outcome,
    evidenceTier: run?.result.evidenceTier,
    totalMeasuredCostMicrousd: run?.totalMeasuredCostMicrousd,
  })}\n`,
);
if (failure !== undefined) throw new Error(failure);
if (residue !== 0) throw new Error(`the run left ${residue} live Google Cloud resources behind`);
