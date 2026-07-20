import { readFileSync } from "node:fs";
import path from "node:path";

/**
 * AFS-00 Apple FM version-drift finding.
 *
 * The Swift `foundation-bridge` source declares one bridge version. The Desktop
 * staging pin declares another. AFS-00 records this drift as an intentional
 * finding; it does not fix it. AFS-02 removes the second copy and generates the
 * staging pin from the single wire-version source in
 * `@openagentsinc/apple-fm-runtime`. This check reads the two live sources and
 * reports the finding so the drift stays visible until AFS-02 closes it.
 */
const repositoryRoot = path.resolve(import.meta.dirname, "..");

const SWIFT_SOURCE = path.join(
  repositoryRoot,
  "apps/pylon/swift/foundation-bridge/Sources/foundation-bridge/main.swift",
);
const STAGING_SOURCE = path.join(repositoryRoot, "apps/openagents-desktop/scripts/stage-target.ts");

export type AppleFmVersionDrift = Readonly<{
  helperVersion: string | null;
  stagingVersion: string | null;
  agree: boolean;
  finding: string | null;
}>;

const firstMatch = (file: string, pattern: RegExp): string | null => {
  const match = readFileSync(file, "utf8").match(pattern);
  return match?.[1] ?? null;
};

export const inspectAppleFmVersionDrift = (): AppleFmVersionDrift => {
  const helperVersion = firstMatch(SWIFT_SOURCE, /private let bridgeVersion\s*=\s*"([^"]+)"/u);
  const stagingVersion = firstMatch(STAGING_SOURCE, /const APPLE_FM_BRIDGE_VERSION\s*=\s*"([^"]+)"/u);
  const agree = helperVersion !== null && helperVersion === stagingVersion;
  const finding = agree
    ? null
    : `Apple FM bridge version drift: Swift helper declares ${String(helperVersion)} but Desktop staging pins ${String(stagingVersion)}. AFS-02 must generate the staging pin from the single wire-version source.`;
  return { helperVersion, stagingVersion, agree, finding };
};

const main = (): void => {
  const drift = inspectAppleFmVersionDrift();
  if (drift.helperVersion === null || drift.stagingVersion === null) {
    console.error("[afs-apple-fm-version-drift] FAIL — could not read one of the version sources");
    process.exitCode = 1;
    return;
  }
  if (drift.agree) {
    console.log(
      `[afs-apple-fm-version-drift] OK — both sources agree at ${drift.helperVersion}`,
    );
    return;
  }
  // AFS-00 records the drift as a finding without failing the build.
  console.warn(`[afs-apple-fm-version-drift] FINDING — ${drift.finding}`);
};

if (import.meta.url === `file://${process.argv[1]}`) main();
