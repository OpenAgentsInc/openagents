// Promote an already-published, signed Desktop tag through the owned release
// coordinator: converge the staged artifacts, publish a signed ReleaseSet v2
// candidate, and atomically promote the channel pointer. This is the first-class
// form of the operational driver documented in
// docs/deploy/2026-07-21-atomic-coordinator-rc-promotion-and-tailnet-linux-acceptance-runbook.md.
//
// It constructs the same committed coordinator as `pnpm run release`
// (createRealCoordinatorPort); it is not an alternate publication path. It
// deliberately skips the current-main preflight because it promotes an
// already-published tag rather than cutting a fresh version from HEAD.
//
// Prerequisites (see the runbook):
//   * The tag's ten required artifacts are already staged in the release-set
//     bucket as immutable candidate objects, each with its sha256 as GCS custom
//     metadata, and described by a manifest JSON.
//   * OPENAGENTS_RELEASE_SECRETS_PATH, CLOUDSDK_CONFIG, OA_RELEASE_SET_BUCKET.
//
// Usage:
//   node --import tsx scripts/promote-tag.ts <manifest.json>
//     [--stable]            promote the stable channel (requires --approve first_stable_promotion)
//     [--approve <gateId>]  approve an owner gate

import { readFileSync } from "node:fs";
import { join } from "node:path";

import {
  createRealCoordinatorPort,
  type StagingManifest,
  type TargetNativeProofs,
} from "./release-ports-real.js";
import {
  newTransactionRef,
  type ReleaseChannel,
  type ReleasePlan,
  type ReleaseTargetKey,
  releaseTargetKeys,
} from "./release.js";

const PROOF_NAMES = [
  "cleanInstall",
  "launch",
  "agentRuntime",
  "shutdown",
  "update",
  "interruptionResume",
  "rollbackOrNoRollback",
  "reinstall",
  "uninstall",
] as const;

const COORDINATOR_STEPS = [
  "checkWorkerInventory",
  "bringUpWorkers",
  "fanOutTargets",
  "runReleaseGates",
  "publishCandidate",
  "promoteChannelPointer",
] as const;

/** Build the per-target native-acceptance proof references for a version. */
export const buildTagNativeProofs = (
  version: string,
): Readonly<Record<ReleaseTargetKey, TargetNativeProofs>> =>
  Object.fromEntries(
    releaseTargetKeys.map((target) => [
      target,
      Object.fromEntries(
        PROOF_NAMES.map((name) => [
          name,
          `openagents.desktop.acceptance.${version}.${target}.${name}.receipt`,
        ]),
      ) as TargetNativeProofs,
    ]),
  ) as Record<ReleaseTargetKey, TargetNativeProofs>;

/** Build the per-target native-acceptance host attestations for a version. */
export const buildTagAttestations = (
  version: string,
): Readonly<Record<ReleaseTargetKey, string>> =>
  Object.fromEntries(
    releaseTargetKeys.map((target) => [
      target,
      `openagents.desktop.acceptance.${version}.${target}.host`,
    ]),
  ) as Record<ReleaseTargetKey, string>;

export class PromoteTagUsageError extends Error {}

/** Validate a staging manifest for a full four-target promotion. */
export const validateStagingManifest = (value: unknown): StagingManifest => {
  if (typeof value !== "object" || value === null) {
    throw new PromoteTagUsageError("manifest is not an object");
  }
  const manifest = value as StagingManifest;
  if (
    typeof manifest.version !== "string" ||
    (manifest.channel !== "rc" && manifest.channel !== "stable") ||
    !/^[0-9a-f]{40}$/.test(manifest.sourceRevision ?? "") ||
    !Array.isArray(manifest.artifacts) ||
    manifest.artifacts.length !== 10
  ) {
    throw new PromoteTagUsageError(
      "manifest must have version, channel, 40-hex sourceRevision, and exactly 10 staged artifacts",
    );
  }
  return manifest;
};

/** Build the release plan for promoting a published tag. */
export const buildTagPlan = (
  manifest: StagingManifest,
  now: Date,
  approvedGates: readonly string[],
): ReleasePlan => ({
  transactionRef: newTransactionRef(manifest.version, manifest.channel, now),
  mode: "real",
  version: manifest.version,
  channel: manifest.channel,
  sourceRevision: manifest.sourceRevision,
  targets: releaseTargetKeys,
  date: now.toISOString().slice(0, 10),
  unattended: true,
  approvedGates,
  attribution: {
    triggerKind: "owner_direction",
    triggeredBy: "owner (tag promotion)",
    releaseActor: "OpenAgents release operator",
    authorityRef: "AUTHORITY.md; program.full_auto_release; grant.autonomous_rc_release",
    releaseUrl: `https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v${manifest.version}`,
    sourceFeedback: "none recorded",
  },
});

const argFlag = (args: readonly string[], flag: string): boolean => args.includes(flag);
const argValues = (args: readonly string[], flag: string): string[] => {
  const out: string[] = [];
  for (let i = 0; i < args.length - 1; i += 1) if (args[i] === flag) out.push(args[i + 1]!);
  return out;
};

const main = async (): Promise<void> => {
  const args = process.argv.slice(2);
  const manifestPath = args.find((a) => !a.startsWith("--"));
  if (manifestPath === undefined) {
    throw new PromoteTagUsageError(
      "usage: node --import tsx scripts/promote-tag.ts <manifest.json> [--stable] [--approve <gateId>]",
    );
  }
  const manifest = validateStagingManifest(JSON.parse(readFileSync(manifestPath, "utf8")));
  const channel: ReleaseChannel = argFlag(args, "--stable") ? "stable" : "rc";
  if (manifest.channel !== channel) {
    throw new PromoteTagUsageError(
      `manifest channel ${manifest.channel} does not match requested channel ${channel}`,
    );
  }
  const approvedGates = [
    ...(channel === "rc" ? ["rc_promotion"] : []),
    ...argValues(args, "--approve"),
  ];

  const rootDir = join(import.meta.dirname, "..");
  const now = new Date();
  const io = {
    rootDir,
    scratchDir: join(rootDir, ".release"),
    log: (line: string) => console.log(line),
    env: process.env,
    now: () => new Date(),
  };
  const plan = buildTagPlan(manifest, now, approvedGates);
  const port = createRealCoordinatorPort(plan, io, {
    attestations: buildTagAttestations(manifest.version),
    stagingManifest: manifest,
    nativeProofs: buildTagNativeProofs(manifest.version),
  });

  console.log(
    `promoting ${manifest.version} (${channel}) @ ${manifest.sourceRevision.slice(0, 10)} — transaction ${plan.transactionRef}`,
  );
  for (const step of COORDINATOR_STEPS) {
    console.log(`\n[${step}]`);
    const result = await port[step](plan);
    for (const line of result.receiptLines) console.log(`  ${line}`);
  }
  console.log("\nPROMOTION COMPLETE");
};

if (import.meta.main) {
  main().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exit(1);
  });
}
