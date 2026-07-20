import { readFileSync } from "node:fs";
import path from "node:path";

/**
 * AFS-02 Apple FM version-drift check (#9080).
 *
 * AFS-00 recorded an intentional finding: the Swift `foundation-bridge` source
 * declared `0.1.3` while the Desktop staging pin declared `0.1.1`. AFS-02 owns
 * closing that drift. The neutral `@openagentsinc/apple-fm-runtime` package is
 * now the SINGLE wire-version source (`APPLE_FM_CANONICAL_HELPER_VERSION`); the
 * Swift bridge and the package's native copy carry that same version, and the
 * Desktop staging pin is DERIVED from the package constant rather than a second
 * literal.
 *
 * This check reads the live sources and passes only when every copy agrees with
 * the single source AND the Desktop staging pin derives from it (no divergent
 * literal). A future wire-version bump changes ONE constant; this check proves
 * every generated copy followed.
 */
const repositoryRoot = path.resolve(import.meta.dirname, "..");

const PACKAGE_IDENTITY = path.join(repositoryRoot, "packages/apple-fm-runtime/src/identity.ts");
const SWIFT_SOURCE = path.join(
  repositoryRoot,
  "apps/pylon/swift/foundation-bridge/Sources/foundation-bridge/main.swift",
);
const PACKAGE_SWIFT_SOURCE = path.join(
  repositoryRoot,
  "packages/apple-fm-runtime/native/foundation-bridge/Sources/foundation-bridge/main.swift",
);
const STAGING_SOURCE = path.join(repositoryRoot, "apps/openagents-desktop/scripts/stage-target.ts");

export type AppleFmVersionDrift = Readonly<{
  canonicalVersion: string | null;
  helperVersion: string | null;
  packageNativeHelperVersion: string | null;
  stagingVersion: string | null;
  stagingDerivesFromSource: boolean;
  agree: boolean;
  finding: string | null;
}>;

const firstMatch = (file: string, pattern: RegExp): string | null => {
  const match = readFileSync(file, "utf8").match(pattern);
  return match?.[1] ?? null;
};

export const inspectAppleFmVersionDrift = (): AppleFmVersionDrift => {
  const canonicalVersion = firstMatch(
    PACKAGE_IDENTITY,
    /APPLE_FM_CANONICAL_HELPER_VERSION\s*=\s*"([^"]+)"/u,
  );
  const helperVersion = firstMatch(SWIFT_SOURCE, /private let bridgeVersion\s*=\s*"([^"]+)"/u);
  const packageNativeHelperVersion = firstMatch(
    PACKAGE_SWIFT_SOURCE,
    /private let bridgeVersion\s*=\s*"([^"]+)"/u,
  );

  const stagingSource = readFileSync(STAGING_SOURCE, "utf8");
  const divergentStagingLiteral = stagingSource.match(/const APPLE_FM_BRIDGE_VERSION\s*=\s*"([^"]+)"/u);
  const stagingDerivesFromSource =
    divergentStagingLiteral === null &&
    /const APPLE_FM_BRIDGE_VERSION\s*=\s*APPLE_FM_CANONICAL_HELPER_VERSION/u.test(stagingSource);
  // When the pin is derived from the single source, its effective value IS the
  // canonical version; a divergent literal keeps its own (drifting) value.
  const stagingVersion = stagingDerivesFromSource ? canonicalVersion : (divergentStagingLiteral?.[1] ?? null);

  const agree =
    canonicalVersion !== null &&
    helperVersion === canonicalVersion &&
    packageNativeHelperVersion === canonicalVersion &&
    stagingDerivesFromSource &&
    stagingVersion === canonicalVersion;

  const finding = agree
    ? null
    : `Apple FM bridge version drift: single source ${String(canonicalVersion)}, Swift helper ${String(helperVersion)}, package native helper ${String(packageNativeHelperVersion)}, Desktop staging ${stagingDerivesFromSource ? "derived" : String(stagingVersion)}. AFS-02 requires every copy to derive from the single wire-version source.`;

  return {
    canonicalVersion,
    helperVersion,
    packageNativeHelperVersion,
    stagingVersion,
    stagingDerivesFromSource,
    agree,
    finding,
  };
};

const main = (): void => {
  const drift = inspectAppleFmVersionDrift();
  if (drift.canonicalVersion === null || drift.helperVersion === null) {
    console.error("[afs-apple-fm-version-drift] FAIL — could not read one of the version sources");
    process.exitCode = 1;
    return;
  }
  if (drift.agree) {
    console.log(`[afs-apple-fm-version-drift] OK — every version source agrees at ${drift.canonicalVersion}`);
    return;
  }
  console.error(`[afs-apple-fm-version-drift] FAIL — ${drift.finding}`);
  process.exitCode = 1;
};

if (import.meta.url === `file://${process.argv[1]}`) main();
