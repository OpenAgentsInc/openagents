import type { ForgeConfig } from "@electron-forge/shared-types"
import { MakerDMG } from "@electron-forge/maker-dmg"
import { MakerZIP } from "@electron-forge/maker-zip"
import { FusesPlugin } from "@electron-forge/plugin-fuses"
import { FuseV1Options, FuseVersion } from "@electron/fuses"
import { execFileSync } from "node:child_process"
import { cp, mkdir, rm } from "node:fs/promises"
import { chmodSync, copyFileSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import { createHash } from "node:crypto"
import path from "node:path"
import { createRequire } from "node:module"
import {
  assertGatekeeperGreen,
  gatekeeperAppChecks,
  gatekeeperImageChecks,
  notarizeAndStapleDmg,
  unsignedDevArtifactName,
} from "./scripts/macos-gatekeeper.ts"


export const OPENAGENTS_DESKTOP_BUNDLE_ID = "com.openagents.desktop"
export const OPENAGENTS_DESKTOP_PROTOCOL = "openagents"

const ignoredCheckoutPath = /^\/(src|scripts|tests|docs|receipts|node_modules)(\/|$)|^\/(README\.md|UPSTREAM\.md|GUARANTEES\.md|tsconfig\.json|forge\.config\.ts)$/
const resolveFromApp = createRequire(path.join(process.cwd(), "package.json"))
const resolveFromClaudeSdk = (): NodeRequire =>
  createRequire(resolveFromApp.resolve("@anthropic-ai/claude-agent-sdk"))
const developerIdApplication = process.env.OA_DEVELOPER_ID_APPLICATION
const notarizeCredentials = process.env.ASC_API_PRIVATE_KEY_PATH !== undefined &&
  process.env.ASC_API_KEY_ID !== undefined && process.env.ASC_API_ISSUER_ID !== undefined
  ? {
      appleApiKey: process.env.ASC_API_PRIVATE_KEY_PATH,
      appleApiKeyId: process.env.ASC_API_KEY_ID,
      appleApiIssuer: process.env.ASC_API_ISSUER_ID,
    }
  : undefined
/**
 * The ONLY escape valve for a make without signing/notary credentials
 * (#8786). It exists for local dev iteration; the artifact is renamed
 * `-UNSIGNED-DEV` so it can never be mistaken for — or published as — a
 * release (release preflight and publish-release both refuse the marker).
 */
const allowUnsignedDev = process.env.OA_ALLOW_UNSIGNED_DEV === "1"

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
])

const isMacCodeSignablePath = (file: string): boolean =>
  /\.(?:app|framework|dylib|node)$/u.test(file) ||
  macCodeSignableBasenames.has(path.basename(file))

const copyRuntimePackage = async (
  buildPath: string,
  packageName: string,
  resolveSpecifier = packageName,
  ascend = 0,
  resolver: NodeRequire = resolveFromApp,
): Promise<void> => {
  let source = path.dirname(resolver.resolve(resolveSpecifier))
  for (let index = 0; index < ascend; index += 1) source = path.dirname(source)
  const destination = path.join(buildPath, "node_modules", ...packageName.split("/"))
  await mkdir(path.dirname(destination), { recursive: true })
  await cp(source, destination, { recursive: true, dereference: true })
}

const config: ForgeConfig = {
  packagerConfig: {
    name: "OpenAgents",
    executableName: "OpenAgents",
    appBundleId: OPENAGENTS_DESKTOP_BUNDLE_ID,
    appCategoryType: "public.app-category.developer-tools",
    extendInfo: { NSMicrophoneUsageDescription: "OpenAgents uses the microphone only while you explicitly run a voice session." },
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
    // Forge's npm-oriented dependency walker does not own pnpm workspace
    // links. Ignore the workspace node_modules tree entirely: the application
    // bundle already contains ordinary dependencies, and packageAfterCopy
    // materializes only the provider packages/native executables that must
    // remain external. This avoids a large, racy copy that is deleted anyway.
    // The release preflight/ASAR oracle enforces the resulting allowlist.
    prune: false,
    derefSymlinks: true,
    // Electron Packager does not convert a PNG into a macOS application icon.
    // Point it at the product-owned ICNS bundle so Finder/Dock never inherit
    // Electron's atom icon. The renderer still uses the shared PNG below.
    icon: "resources/openagents-icon.icns",
    extraResource: ["dist/native", "dist/builtin-skills"],
    ignore: path => ignoredCheckoutPath.test(path),
    protocols: [{ name: "OpenAgents", schemes: [OPENAGENTS_DESKTOP_PROTOCOL] }],
    osxSign: developerIdApplication === undefined ? undefined : {
      identity: developerIdApplication,
      // Electron Framework/Versions/Current is a symlink to A. Walking and
      // signing both views races the same resource tree and eventually asks
      // codesign to reopen a path already rewritten through the other view.
      ignore: file =>
        file.includes("/Electron Framework.framework/Versions/Current/") ||
        !isMacCodeSignablePath(file),
      optionsForFile: () => ({
        entitlements: "build/entitlements.mac.plist",
        hardenedRuntime: true,
      }),
    },
    osxNotarize: notarizeCredentials,
  },
  hooks: {
    generateAssets: async () => {
      execFileSync("node", ["--import", "tsx", "scripts/build.ts"], {
        cwd: process.cwd(),
        stdio: "inherit",
      })
      execFileSync("cargo", ["build", "--release", "-p", "oa-desktop-audio"], { cwd: path.resolve(process.cwd(), "../.."), stdio: "inherit" })
      const architecture = process.arch
      const destinationDirectory = path.join(process.cwd(), "dist", "native", architecture)
      const destination = path.join(destinationDirectory, "oa-desktop-audio")
      mkdirSync(destinationDirectory, { recursive: true })
      copyFileSync(path.resolve(process.cwd(), "../../target/release/oa-desktop-audio"), destination)
      chmodSync(destination, 0o755)
      const sha256 = createHash("sha256").update(readFileSync(destination)).digest("hex")
      writeFileSync(path.join(destinationDirectory, "manifest.json"), JSON.stringify({ protocolVersion: 1, helperVersion: "0.1.0", architecture, sha256 }) + "\n", { mode: 0o644 })
    },
    packageAfterCopy: async (_forgeConfig, buildPath, _electronVersion, platform, arch) => {
      // The application build bundles every dependency except provider packages whose
      // native payloads must remain relative to their package. Replace the
      // copied workspace node_modules with that explicit runtime allowlist.
      await rm(path.join(buildPath, "node_modules"), { recursive: true, force: true })
      await copyRuntimePackage(buildPath, "@anthropic-ai/claude-agent-sdk")
      await copyRuntimePackage(
        buildPath,
        `@anthropic-ai/claude-agent-sdk-${platform}-${arch}`,
        `@anthropic-ai/claude-agent-sdk-${platform}-${arch}/package.json`,
        0,
        resolveFromClaudeSdk(),
      )
      await copyRuntimePackage(buildPath, "@openai/codex", "@openai/codex/bin/codex.js", 1)
      await copyRuntimePackage(buildPath, `@openai/codex-${platform}-${arch}`, `@openai/codex-${platform}-${arch}/package.json`)
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
      if (process.platform !== "darwin") return makeResults
      const signingReady = developerIdApplication !== undefined && notarizeCredentials !== undefined
      if (!signingReady) {
        if (!allowUnsignedDev) {
          throw new Error(
            "make REFUSED: Developer ID identity and/or notary credentials are absent " +
              "(OA_DEVELOPER_ID_APPLICATION + ASC_API_PRIVATE_KEY_PATH/ASC_API_KEY_ID/ASC_API_ISSUER_ID). " +
              "There is no unsigned release fallback — an unsigned outer artifact is Gatekeeper-dead on arrival " +
              "(docs/teardowns/2026-07-13-t3-code-teardown.md). " +
              "For a local dev artifact only, set OA_ALLOW_UNSIGNED_DEV=1; the output is renamed -UNSIGNED-DEV " +
              "and can never pass release preflight or publish.",
          )
        }
        return makeResults.map((result) => ({
          ...result,
          artifacts: result.artifacts.map((artifact) => {
            const renamed = path.join(path.dirname(artifact), unsignedDevArtifactName(path.basename(artifact)))
            if (renamed !== artifact) renameSync(artifact, renamed)
            return renamed
          }),
        }))
      }
      for (const result of makeResults) {
        const appPath = path.join(
          process.cwd(),
          "out",
          `OpenAgents-${result.platform}-${result.arch}`,
          "OpenAgents.app",
        )
        for (const artifact of result.artifacts.filter((file) => file.endsWith(".dmg"))) {
          notarizeAndStapleDmg(artifact, appPath, notarizeCredentials)
          assertGatekeeperGreen([...gatekeeperImageChecks(artifact), ...gatekeeperAppChecks(appPath)])
        }
      }
      return makeResults
    },
  },
  makers: [
    new MakerDMG({
      format: "ULFO",
      overwrite: true,
      ...(developerIdApplication === undefined ? {} : {
        additionalDMGOptions: {
          "code-sign": {
            "signing-identity": developerIdApplication,
            identifier: OPENAGENTS_DESKTOP_BUNDLE_ID,
          },
        },
      }),
    }, ["darwin"]),
    new MakerZIP({}, ["darwin"]),
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
}

export default config
