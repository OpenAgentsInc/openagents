import type { ForgeConfig } from "@electron-forge/shared-types";
import { MakerDMG } from "@electron-forge/maker-dmg";
import { MakerDeb } from "@electron-forge/maker-deb";
import { MakerRpm } from "@electron-forge/maker-rpm";
import { MakerZIP } from "@electron-forge/maker-zip";
import { FusesPlugin } from "@electron-forge/plugin-fuses";
import { FuseV1Options, FuseVersion } from "@electron/fuses";
import { signAsync, type SignOptions } from "@electron/osx-sign";
import { cp, mkdir, readdir, rm } from "node:fs/promises";
import { existsSync, readFileSync, renameSync } from "node:fs";
import path from "node:path";
import {
  assertGatekeeperGreen,
  gatekeeperAppChecks,
  gatekeeperImageChecks,
  notarizeAndStapleDmg,
  unsignedDevArtifactName,
} from "./scripts/macos-gatekeeper.ts";
import { desktopReleaseArtifactName } from "./scripts/release-artifact-name.ts";
import { MakerAppImage } from "./scripts/maker-appimage.ts";
import { verifyPackagedCodexRuntime } from "./scripts/codex-runtime-artifact-smoke.ts";
import {
  assertPackagedAsarAdmissible,
  stagedTreePath,
  verifyStagedTreeAgainstLedger,
} from "./scripts/stage-target.ts";
import {
  type DesktopTargetBuildDescriptor,
  decodeDesktopTargetBuildDescriptor,
  desktopTargets,
} from "./src/release-staging-contract.ts";

/**
 * DIST-03 (#8916, repaired): packaging entrypoints require an EXPLICIT typed
 * target build descriptor AND a staged workspace produced by
 * `scripts/stage-target.ts`. `OA_DESKTOP_STAGING_WORKSPACE` points at that
 * workspace; the descriptor is decoded ONCE from its `descriptor.json` and
 * threaded through every hook. The developer checkout and shared
 * node_modules are NEVER the packaged source: Forge's copy of the checkout is
 * discarded wholesale and the staged tree becomes the sole packaged content.
 * Host architecture inference never chooses a release target.
 */
interface StagedBuildInputs {
  readonly descriptor: DesktopTargetBuildDescriptor;
  readonly workspace: string;
  readonly stagedTree: string;
  /** Raw ledger.json — re-verified against the CURRENT staged bytes at every gate. */
  readonly ledgerJson: unknown;
  /** Independently supplied by the staging/coordinator invocation. */
  readonly expectedLedgerRef: string;
}

let cachedStagedBuildInputs: StagedBuildInputs | null = null;
const requireStagedBuildInputs = (): StagedBuildInputs => {
  if (cachedStagedBuildInputs !== null) return cachedStagedBuildInputs;
  const workspace = process.env.OA_DESKTOP_STAGING_WORKSPACE;
  if (workspace === undefined || workspace === "") {
    throw new Error(
      "packaging REFUSED: OA_DESKTOP_STAGING_WORKSPACE must point at a stage:target staging " +
        "workspace (scripts/stage-target.ts). Packaging never consumes the developer checkout " +
        "or shared node_modules, and host architecture is never inferred (DIST-03 #8916).",
    );
  }
  const expectedLedgerRef = process.env.OA_DESKTOP_EXPECTED_LEDGER_REF;
  if (expectedLedgerRef === undefined || !/^sha256:[0-9a-f]{64}$/u.test(expectedLedgerRef)) {
    throw new Error(
      "packaging REFUSED: OA_DESKTOP_EXPECTED_LEDGER_REF must independently pin the exact ledger selected by staging/coordinator",
    );
  }
  // Decode the typed descriptor exactly once and thread it through.
  const descriptor = decodeDesktopTargetBuildDescriptor(
    JSON.parse(readFileSync(path.join(workspace, "descriptor.json"), "utf8")),
  );
  const stagedTree = stagedTreePath(workspace);
  for (const required of ["package.json", "dist", "node_modules"]) {
    if (!existsSync(path.resolve(stagedTree, required))) {
      throw new Error(
        `packaging REFUSED: staging workspace is incomplete (missing ${required}); ` +
          "re-run pnpm run stage:target (DIST-03 #8916)",
      );
    }
  }
  const ledgerJson = JSON.parse(
    readFileSync(path.join(workspace, "ledger.json"), "utf8"),
  ) as unknown;
  cachedStagedBuildInputs = { descriptor, workspace, stagedTree, ledgerJson, expectedLedgerRef };
  return cachedStagedBuildInputs;
};

/**
 * Binds the ACTUAL tool versions this packaging invocation resolves (the
 * installed Electron and Electron Forge) to the staged ledger's locked
 * toolchain identity — a packaging run can never consume a staged workspace
 * built against different tool pins (DIST-03 re-review blocker 1).
 */
const assertActualToolIdentity = (toolchain: {
  readonly electron: string;
  readonly forge: string;
}): void => {
  const installedVersion = (packagePath: string): string =>
    (
      JSON.parse(
        readFileSync(
          path.join(process.cwd(), "node_modules", ...packagePath.split("/"), "package.json"),
          "utf8",
        ),
      ) as { version: string }
    ).version;
  const actual = {
    electron: installedVersion("electron"),
    forge: installedVersion("@electron-forge/cli"),
  };
  for (const tool of ["electron", "forge"] as const) {
    if (actual[tool] !== toolchain[tool]) {
      throw new Error(
        `packaging REFUSED: installed ${tool} ${actual[tool]} does not match the staged ledger's locked ${tool} ${toolchain[tool]} (DIST-03 #8916)`,
      );
    }
  }
};

const stagingWorkspaceEnv = process.env.OA_DESKTOP_STAGING_WORKSPACE;
const stagedPathOr = (fallback: string, ...stagedSegments: ReadonlyArray<string>): string =>
  stagingWorkspaceEnv === undefined || stagingWorkspaceEnv === ""
    ? fallback
    : path.join(stagedTreePath(stagingWorkspaceEnv), ...stagedSegments);

const activeDescriptor =
  stagingWorkspaceEnv === undefined || stagingWorkspaceEnv === ""
    ? undefined
    : requireStagedBuildInputs().descriptor;
const activeChannel = activeDescriptor?.channel ?? "stable";
const activeProductIdentity = {
  appId: activeChannel === "rc" ? "com.openagents.desktop.rc" : "com.openagents.desktop",
  displayName: activeChannel === "rc" ? "OpenAgents RC" : "OpenAgents",
  executableName:
    activeDescriptor?.targetKey.startsWith("linux-") === true
      ? activeChannel === "rc"
        ? "openagents-rc"
        : "openagents"
      : activeChannel === "rc"
        ? "OpenAgents RC"
        : "OpenAgents",
  linuxPackageName: activeChannel === "rc" ? "openagents-desktop-rc" : "openagents-desktop",
  protocol: activeChannel === "rc" ? "openagents-rc" : "openagents",
  startupWmClass: activeChannel === "rc" ? "OpenAgents-RC" : "OpenAgents",
} as const;

const releaseSetArtifactName = (
  descriptor: DesktopTargetBuildDescriptor,
  extension: string,
): string => {
  const normalizedExtension = extension.toLowerCase() === ".appimage" ? ".AppImage" : extension;
  return `OpenAgents-${descriptor.version}-${descriptor.channel}-${descriptor.targetKey}${normalizedExtension}`;
};

const assertDescriptorMatchesMakerTarget = (
  descriptor: DesktopTargetBuildDescriptor,
  platform: string,
  arch: string,
): void => {
  if (`${platform}-${arch}` !== descriptor.targetKey) {
    throw new Error(
      `packaging REFUSED: maker invocation targets ${platform}-${arch} but the staged descriptor declares ${descriptor.targetKey}`,
    );
  }
};

export const OPENAGENTS_DESKTOP_BUNDLE_ID = "com.openagents.desktop";
export const OPENAGENTS_DESKTOP_PROTOCOL = "openagents";
export const canonicalArtifactPath = (
  artifact: string,
  platform: string,
  arch: string,
  stagedVersion: string,
): string =>
  path.join(
    path.dirname(artifact),
    desktopReleaseArtifactName({
      product: "OpenAgents",
      version: stagedVersion,
      platform,
      arch,
      extension: path.extname(artifact),
    }),
  );

const renameArtifact = (artifact: string, destination: string): string => {
  if (destination !== artifact) renameSync(artifact, destination);
  return destination;
};

const developerIdApplication = process.env.OA_DEVELOPER_ID_APPLICATION;
const notarizeCredentials =
  process.env.ASC_API_PRIVATE_KEY_PATH !== undefined &&
  process.env.ASC_API_KEY_ID !== undefined &&
  process.env.ASC_API_ISSUER_ID !== undefined
    ? {
        appleApiKey: process.env.ASC_API_PRIVATE_KEY_PATH,
        appleApiKeyId: process.env.ASC_API_KEY_ID,
        appleApiIssuer: process.env.ASC_API_ISSUER_ID,
      }
    : undefined;
/**
 * The ONLY escape valve for a make without signing/notary credentials
 * (#8786). It exists for local dev iteration; the artifact is renamed
 * `-UNSIGNED-DEV` so it can never be mistaken for — or published as — a
 * release (release preflight and publish-release both refuse the marker).
 */
const allowUnsignedDev = process.env.OA_ALLOW_UNSIGNED_DEV === "1";

const macCodeSignableBasenames = new Set([
  "OpenAgents",
  "OpenAgents Helper",
  "Electron Framework",
  "chrome_crashpad_handler",
  "codex",
  "claude",
  "codex-code-mode-host",
  "rg",
  "ShipIt",
  "zsh",
  "oa-desktop-audio",
]);

const isMacCodeSignablePath = (file: string): boolean =>
  /\.(?:app|framework|dylib|node)$/u.test(file) ||
  macCodeSignableBasenames.has(path.basename(file));

const macSignOptions = (appPath: string): SignOptions => {
  if (developerIdApplication === undefined) {
    throw new Error("macOS signing identity is absent");
  }
  return {
    app: appPath,
    identity: developerIdApplication,
    platform: "darwin",
    ignore: (file) =>
      file.includes("/Electron Framework.framework/Versions/Current/") ||
      !isMacCodeSignablePath(file),
    optionsForFile: () => ({
      entitlements: stagedPathOr(
        "build/entitlements.mac.plist",
        "build",
        "entitlements.mac.plist",
      ),
      hardenedRuntime: true,
    }),
  };
};

const config: ForgeConfig = {
  packagerConfig: {
    name: activeProductIdentity.displayName,
    executableName: activeProductIdentity.executableName,
    appBundleId: activeProductIdentity.appId,
    appCategoryType: "public.app-category.developer-tools",
    extendInfo: {
      NSMicrophoneUsageDescription:
        "OpenAgents uses the microphone only while you explicitly run a voice session.",
    },
    asar: {
      // Both provider packages resolve and spawn native executables relative
      // to their installed package. Executables cannot run inside app.asar.
      // Keep the renderer on a real, bounded filesystem path. With
      // GrantFileProtocolExtraPrivileges disabled, Chromium does not admit the
      // top-level file URL through ASAR on the installed artifact even though
      // Electron's Node-side ASAR APIs can list it.
      unpack: "**/node_modules/{@anthropic-ai/claude-agent-sdk*,@openai/codex*}/**/*",
      // `unpack` glob matching is rooted differently by Electron Packager;
      // the prior brace expression left these files inside app.asar. The
      // dedicated directory option is the authoritative real-file boundary.
      // Node worker_threads must execute from a real file. Electron 43 can
      // address a worker entry inside ASAR but traps in V8 thread isolation
      // when that worker starts. Keep only the two bounded worker entrypoints
      // beside the renderer on the signed, unpacked filesystem.
      unpackDir: "dist/{renderer,workers}",
    },
    // The developer checkout is NEVER the packaged source (DIST-03 #8916):
    // ignore every checkout path so Electron Packager's initial copy is
    // empty, then packageAfterCopy materializes the STAGED tree produced by
    // scripts/stage-target.ts as the sole packaged content. The release
    // preflight/ASAR oracle plus the postPackage live gate enforce the
    // resulting allowlist.
    prune: false,
    derefSymlinks: true,
    // Electron Packager does not convert a PNG into a macOS application icon.
    // Point it at the product-owned ICNS bundle so Finder/Dock never inherit
    // Electron's atom icon. Packaging inputs come from the staged tree
    // (exported at the descriptor's source revision), with the checkout path
    // as the config-import fallback for tests that never package.
    icon: stagedPathOr("resources/openagents-icon.icns", "resources", "openagents-icon.icns"),
    extraResource: [
      stagedPathOr("dist/native", "native"),
      stagedPathOr("dist/builtin-skills", "dist", "builtin-skills"),
    ],
    ignore: () => true,
    protocols: [{ name: activeProductIdentity.displayName, schemes: [activeProductIdentity.protocol] }],
  },
  hooks: {
    /**
     * Packaging performs NO building (DIST-03 #8916, repaired): the staged
     * workspace produced by `pnpm run stage:target` — clean source export,
     * locked target-only production install, explicit-Rust-triple native
     * helper — is the only admissible input. This hook merely proves the
     * staged inputs exist and match the maker invocation before any copy.
     */
    generateAssets: async (_forgeConfig, platform, arch) => {
      const { descriptor, stagedTree, ledgerJson, expectedLedgerRef } = requireStagedBuildInputs();
      assertDescriptorMatchesMakerTarget(descriptor, platform, arch);
      // Explicit-triple native builds happen in staging (stage-target.ts),
      // never here: the maker lane consumes the descriptor's closed target
      // definition and the already-built staged closure.
      void desktopTargets[descriptor.targetKey].rustTargetTriple;
      // BEFORE-COPY binding (re-review blocker 3): decode/validate the
      // ledger, bind every descriptor identity field, recompute the ledger
      // ref, and re-hash the CURRENT staged bytes against the component
      // digests — a mutated or stale staging workspace never reaches a copy.
      const verified = await verifyStagedTreeAgainstLedger(
        stagedTree,
        descriptor,
        ledgerJson,
        expectedLedgerRef,
      );
      // Bind the ACTUAL tools this invocation resolves to the locked
      // identities the ledger recorded (re-review blocker 1).
      assertActualToolIdentity(verified.ledger.toolchain);
      process.stderr.write(
        `[forge] staged ledger verified before copy: ${verified.ledgerRef} (${verified.ledger.components.length} components) for ${descriptor.targetKey}\n`,
      );
    },
    /**
     * The staged tree IS the packaged source. Electron Packager's initial
     * copy of the checkout is fully discarded (packagerConfig.ignore already
     * excludes everything) and replaced with the staged application content:
     * dist/, package.json, and the target-only runtime node_modules
     * materialized by the locked production install. `process.cwd()` and the
     * shared workspace node_modules never reach the package.
     */
    packageAfterCopy: async (_forgeConfig, buildPath, _electronVersion, platform, arch) => {
      const { descriptor, stagedTree } = requireStagedBuildInputs();
      assertDescriptorMatchesMakerTarget(descriptor, platform, arch);
      // packagerConfig.ignore excludes the entire checkout, so the packager
      // may not even create the app directory; materialize it and discard any
      // stray copied content so the staged tree is the SOLE packaged source.
      await mkdir(buildPath, { recursive: true });
      for (const leftover of await readdir(buildPath)) {
        await rm(path.join(buildPath, leftover), { recursive: true, force: true });
      }
      for (const entry of ["dist", "package.json", "node_modules"]) {
        await cp(path.join(stagedTree, entry), path.join(buildPath, entry), {
          recursive: true,
          dereference: true,
        });
      }
      const stagedManifest = JSON.parse(
        readFileSync(path.join(stagedTree, "package.json"), "utf8"),
      ) as { version: string };
      if (stagedManifest.version !== descriptor.version) {
        throw new Error(
          `packaging REFUSED: staged tree version ${stagedManifest.version} does not match descriptor ${descriptor.version}`,
        );
      }
    },
    /**
     * LIVE post-package ASAR gate (DIST-03 #8916 review blocker 3): list the
     * REAL entries of the just-assembled app.asar and re-run the staged-tree
     * oracle with them. One unexpected ASAR entry fails the build here —
     * before any maker or signing work.
     */
    postPackage: async (_forgeConfig, packageResult) => {
      const { descriptor, stagedTree, ledgerJson, expectedLedgerRef } = requireStagedBuildInputs();
      // POST-PACKAGE binding (re-review blocker 3): the staged tree must
      // STILL match the ledger after the package step — reuse of a mutated
      // workspace can never yield a receipt referencing stale proof.
      const verified = await verifyStagedTreeAgainstLedger(
        stagedTree,
        descriptor,
        ledgerJson,
        expectedLedgerRef,
      );
      for (const outputPath of packageResult.outputPaths) {
        const resourcesPath =
          packageResult.platform === "darwin"
            ? path.join(
                outputPath,
                `${activeProductIdentity.displayName}.app`,
                "Contents",
                "Resources",
              )
            : path.join(outputPath, "resources");
        const gate = await assertPackagedAsarAdmissible({
          descriptor,
          ledger: verified.ledger,
          stagedTree,
          asarPath: path.join(resourcesPath, "app.asar"),
          resourcesPath,
          repoRoot: path.resolve(process.cwd(), "../.."),
        });
        process.stderr.write(
          `[forge] postPackage asar gate: ${gate.result} (${gate.asarEntryCount} entries, ` +
            `${gate.unpackedEntryCount} unpacked, ${gate.verifiedComponents} closure components byte-verified) ` +
            `against ledger ${verified.ledgerRef} for ${descriptor.targetKey}\n`,
        );
      }
    },
    /**
     * Electron Packager normally signs before Forge's postPackage hook, which
     * changes Mach-O bytes before the staged-ledger oracle can compare them.
     * Keep packaging unsigned through postPackage, then sign the verified app
     * here before any DMG/ZIP maker consumes it.
     */
    preMake: async () => {
      const { descriptor } = requireStagedBuildInputs();
      if (!descriptor.targetKey.startsWith("darwin-")) return;
      const signingReady =
        developerIdApplication !== undefined && notarizeCredentials !== undefined;
      if (!signingReady) {
        if (allowUnsignedDev) return;
        throw new Error(
          "make REFUSED before makers: Developer ID identity and/or notary credentials are absent",
        );
      }
      const appPath = path.join(
        process.cwd(),
        "out",
        `${activeProductIdentity.displayName}-${descriptor.targetKey}`,
        `${activeProductIdentity.displayName}.app`,
      );
      await signAsync(macSignOptions(appPath));
      process.stderr.write(`[forge] signed verified app before makers: ${appPath}\n`);
    },
    /**
     * Gatekeeper release gate (#8786) — mechanized from two 2026-07-13
     * incidents: T3 Code's Gatekeeper-dead unsigned DMG around a notarized
     * app (docs/teardowns/2026-07-13-t3-code-teardown.md, night addendum)
     * and ChatGPT's updater installing an app the machine refused to exec
     * (docs/fable/2026-07-13-chatgpt-codex-launch-failure-analysis.md).
     *
     * With credentials: notarize the DMG ITSELF (the ticket covers the
     * nested app), staple both the `.app` and the `.dmg`, then fail the make
     * closed unless every Gatekeeper oracle is green. Without credentials:
     * REFUSE — no unsigned release fallback — unless OA_ALLOW_UNSIGNED_DEV=1,
     * which renames every artifact `-UNSIGNED-DEV`.
     */
    postMake: async (_forgeConfig, makeResults) => {
      const { descriptor } = requireStagedBuildInputs();
      for (const result of makeResults) {
        assertDescriptorMatchesMakerTarget(descriptor, result.platform, result.arch);
      }
      if (process.platform !== "darwin") {
        return makeResults.map((result) => ({
          ...result,
          artifacts: result.artifacts.map((artifact) =>
            renameArtifact(
              artifact,
              path.join(
                path.dirname(artifact),
                releaseSetArtifactName(descriptor, path.extname(artifact)),
              ),
            ),
          ),
        }));
      }
      const signingReady =
        developerIdApplication !== undefined && notarizeCredentials !== undefined;
      if (!signingReady) {
        if (!allowUnsignedDev) {
          throw new Error(
            "make REFUSED: Developer ID identity and/or notary credentials are absent " +
              "(OA_DEVELOPER_ID_APPLICATION + ASC_API_PRIVATE_KEY_PATH/ASC_API_KEY_ID/ASC_API_ISSUER_ID). " +
              "There is no unsigned release fallback — an unsigned outer artifact is Gatekeeper-dead on arrival " +
              "(docs/teardowns/2026-07-13-t3-code-teardown.md). " +
              "For a local dev artifact only, set OA_ALLOW_UNSIGNED_DEV=1; the output is renamed -UNSIGNED-DEV " +
              "and can never pass release preflight or publish.",
          );
        }
        return makeResults.map((result) => ({
          ...result,
          artifacts: result.artifacts.map((artifact) => {
            const canonical = canonicalArtifactPath(
              artifact,
              result.platform,
              result.arch,
              descriptor.version,
            );
            const unsigned = path.join(
              path.dirname(canonical),
              unsignedDevArtifactName(path.basename(canonical)),
            );
            return renameArtifact(artifact, unsigned);
          }),
        }));
      }
      for (const result of makeResults) {
        const appPath = path.join(
          process.cwd(),
          "out",
          `${activeProductIdentity.displayName}-${result.platform}-${result.arch}`,
          `${activeProductIdentity.displayName}.app`,
        );
        for (const artifact of result.artifacts.filter((file) => file.endsWith(".dmg"))) {
          notarizeAndStapleDmg(artifact, appPath, notarizeCredentials);
          assertGatekeeperGreen([
            ...gatekeeperImageChecks(artifact),
            ...gatekeeperAppChecks(appPath),
          ]);
          // A signed shell is insufficient: prove the exact unpacked native
          // runtime is signed, executable, target-correct, and version-pinned
          // with no global Codex/NVM resolution available.
          verifyPackagedCodexRuntime({
            appPath,
            platform: result.platform,
            arch: result.arch,
            requireSignature: true,
          });
        }
      }
      return makeResults.map((result) => ({
        ...result,
        artifacts: result.artifacts.map((artifact) =>
          renameArtifact(
            artifact,
            path.join(
              path.dirname(artifact),
              releaseSetArtifactName(descriptor, path.extname(artifact)),
            ),
          ),
        ),
      }));
    },
  },
  makers: [
    new MakerDMG(
      {
        format: "ULFO",
        overwrite: true,
        ...(developerIdApplication === undefined
          ? {}
          : {
              additionalDMGOptions: {
                "code-sign": {
                  "signing-identity": developerIdApplication,
                  identifier: activeProductIdentity.appId,
                },
              },
            }),
      },
      ["darwin"],
    ),
    new MakerZIP({}, ["darwin"]),
    new MakerAppImage(
      () => {
        const { descriptor } = requireStagedBuildInputs();
        return {
          artifactName: releaseSetArtifactName(descriptor, ".AppImage"),
          appId: activeProductIdentity.appId,
          executableName: activeProductIdentity.executableName,
          productName: activeProductIdentity.displayName,
          startupWmClass: activeProductIdentity.startupWmClass,
        };
      },
      ["linux"],
    ),
    new MakerDeb(
      () => ({
        options: {
          name: activeProductIdentity.linuxPackageName,
          productName: activeProductIdentity.displayName,
          genericName: "AI development environment",
          description: "OpenAgents Effect Native desktop application",
          productDescription: "OpenAgents Effect Native desktop application and local agent runtime.",
          section: "devel",
          priority: "optional",
          maintainer: "OpenAgents, Inc.",
          homepage: "https://openagents.com",
          bin: activeProductIdentity.executableName,
          icon: stagedPathOr("resources/openagents-icon.png", "resources", "openagents-icon.png"),
          categories: ["Development"],
        },
      }),
      ["linux"],
    ),
    new MakerRpm(
      () => ({
        options: {
          name: activeProductIdentity.linuxPackageName,
          productName: activeProductIdentity.displayName,
          genericName: "AI development environment",
          description: "OpenAgents Effect Native desktop application",
          productDescription: "OpenAgents Effect Native desktop application and local agent runtime.",
          license: "MIT",
          group: "Development/Tools",
          homepage: "https://openagents.com",
          bin: activeProductIdentity.executableName,
          icon: stagedPathOr("resources/openagents-icon.png", "resources", "openagents-icon.png"),
          categories: ["Development"],
        },
      }),
      ["linux"],
    ),
  ],
  plugins: [
    new FusesPlugin({
      version: FuseVersion.V1,
      strictlyRequireAllFuses: true,
      [FuseV1Options.RunAsNode]: false,
      [FuseV1Options.EnableCookieEncryption]: true,
      [FuseV1Options.EnableNodeOptionsEnvironmentVariable]: false,
      [FuseV1Options.EnableNodeCliInspectArguments]: false,
      [FuseV1Options.EnableEmbeddedAsarIntegrityValidation]: true,
      [FuseV1Options.OnlyLoadAppFromAsar]: true,
      // Electron ships the standard architecture-specific snapshot. Enabling
      // the browser-specific fuse without also supplying
      // browser_v8_context_snapshot.bin makes the signed app fail before boot.
      [FuseV1Options.LoadBrowserProcessSpecificV8Snapshot]: false,
      [FuseV1Options.GrantFileProtocolExtraPrivileges]: false,
      [FuseV1Options.WasmTrapHandlers]: true,
    }),
  ],
};

export default config;
