import { readFileSync } from "node:fs";
import path from "node:path";

/**
 * AFS-02 Apple FM version-drift check (#9080).
 *
 * AFS-00 recorded an intentional finding: copies of the Swift
 * `foundation-bridge` wire version drifted apart. AFS-02 owns closing that
 * drift. The neutral `@openagentsinc/apple-fm-runtime` package is the SINGLE
 * wire-version source (`APPLE_FM_CANONICAL_HELPER_VERSION`); the Pylon Swift
 * bridge and the package's own native copy carry that same version.
 *
 * (The former Desktop staging pin was a third derived copy. The Electron
 * Desktop app is gone, so that leg is retired; the remaining live copies are
 * the package identity constant and the two Swift sources.)
 *
 * This check reads the live sources and passes only when every copy agrees with
 * the single source. A future wire-version bump changes ONE constant; this
 * check proves every generated copy followed.
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

export type AppleFmVersionDrift = Readonly<{
  canonicalVersion: string | null;
  helperVersion: string | null;
  packageNativeHelperVersion: string | null;
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

  const agree =
    canonicalVersion !== null &&
    helperVersion === canonicalVersion &&
    packageNativeHelperVersion === canonicalVersion;

  const finding = agree
    ? null
    : `Apple FM bridge version drift: single source ${String(canonicalVersion)}, Swift helper ${String(helperVersion)}, package native helper ${String(packageNativeHelperVersion)}. AFS-02 requires every copy to derive from the single wire-version source.`;

  return {
    canonicalVersion,
    helperVersion,
    packageNativeHelperVersion,
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
