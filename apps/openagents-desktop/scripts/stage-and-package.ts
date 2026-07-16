/**
 * Descriptor-first packaging entrypoint (DIST-03, #8916).
 *
 * The only supported way to run Electron Forge for this application:
 *
 *   1. Build the explicit typed target descriptor (target key from the
 *      command line; version/channel/source revision/lockfile identity
 *      derived from the EXACT `HEAD` revision — never host inference).
 *   2. Run the full isolated staging flow (`scripts/stage-target.ts`):
 *      clean temporary workspace, exported source, locked target-only
 *      production install, explicit-triple native builds, staged-tree oracle,
 *      §9 ledger.
 *   3. Invoke `electron-forge package|make` with
 *      `OA_DESKTOP_STAGING_WORKSPACE` pointing at that workspace, so
 *      forge.config.ts packages the STAGED tree and runs the live post-
 *      package ASAR gate.
 *
 * `--staging-workspace <path>` reuses an existing staged workspace (e.g. a
 * separate `pnpm run stage:target` run) instead of staging again.
 */
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  decodeDesktopTargetBuildDescriptor,
  desktopTargets,
  type DesktopTargetKey,
  TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
} from "../src/release-staging-contract.ts";
import { hostStageTargetIo, readDesktopManifestPins, stageTarget } from "./stage-target.ts";
import { createHash } from "node:crypto";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const checkoutRoot = path.resolve(appRoot, "../..");

const argv = process.argv.slice(2);
const flag = (name: string): string | undefined => {
  const index = argv.indexOf(`--${name}`);
  return index >= 0 ? argv[index + 1] : undefined;
};

const targetKey = flag("target");
if (targetKey === undefined || !(targetKey in desktopTargets)) {
  throw new Error(
    "stage-and-package REQUIRES an explicit target: --target <darwin-arm64|darwin-x64|win32-arm64|win32-x64|linux-arm64|linux-x64> --mode <package|make> [--unsigned-dev] [--staging-workspace <path>]",
  );
}
const mode = flag("mode") ?? "package";
if (mode !== "package" && mode !== "make") {
  throw new Error(`unsupported mode ${mode}: use package or make`);
}

let workspace = flag("staging-workspace");
if (workspace === undefined) {
  const sourceRevision = execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: checkoutRoot,
    encoding: "utf8",
  }).trim();
  const version = (
    JSON.parse(
      execFileSync("git", ["show", `${sourceRevision}:apps/openagents-desktop/package.json`], {
        cwd: checkoutRoot,
        encoding: "utf8",
      }),
    ) as { version: string }
  ).version;
  const lockfileSha256 = createHash("sha256")
    .update(
      execFileSync("git", ["show", `${sourceRevision}:pnpm-lock.yaml`], {
        cwd: checkoutRoot,
        maxBuffer: 512 * 1024 * 1024,
      }),
    )
    .digest("hex");
  const descriptor = decodeDesktopTargetBuildDescriptor({
    schema: TARGET_BUILD_DESCRIPTOR_SCHEMA_ID,
    product: "OpenAgents",
    targetKey,
    channel: version.includes("-rc.") ? "rc" : "stable",
    version,
    sourceRevision,
    lockfileSha256,
    formats: [...desktopTargets[targetKey as DesktopTargetKey].requiredFormats],
    signingPolicy: argv.includes("--unsigned-dev") ? "unsigned-dev" : "production",
  });
  const pins = readDesktopManifestPins(readFileSync(path.join(appRoot, "package.json"), "utf8"));
  const result = await stageTarget(descriptor, pins, hostStageTargetIo("local-stage-and-package"));
  if (!result.ok) {
    process.stderr.write(`${JSON.stringify(result, null, 2)}\n`);
    process.exit(1);
  }
  await writeFile(
    path.join(result.workspace, "descriptor.json"),
    `${JSON.stringify(descriptor, null, 2)}\n`,
    "utf8",
  );
  await writeFile(
    path.join(result.workspace, "ledger.json"),
    `${JSON.stringify(result.ledger, null, 2)}\n`,
    "utf8",
  );
  process.stderr.write(
    `[stage-and-package] staged ${descriptor.targetKey} at ${result.workspace} (ledgerRef ${result.ledgerRef})\n`,
  );
  workspace = result.workspace;
}

const { platform, arch } = desktopTargets[targetKey as DesktopTargetKey];
execFileSync(
  "pnpm",
  ["exec", "electron-forge", mode, `--platform=${platform}`, `--arch=${arch}`],
  {
    cwd: appRoot,
    stdio: "inherit",
    env: { ...process.env, OA_DESKTOP_STAGING_WORKSPACE: workspace },
  },
);
