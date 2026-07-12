import type { ForgeConfig } from "@electron-forge/shared-types"
import { MakerDMG } from "@electron-forge/maker-dmg"
import { MakerZIP } from "@electron-forge/maker-zip"
import { FusesPlugin } from "@electron-forge/plugin-fuses"
import { FuseV1Options, FuseVersion } from "@electron/fuses"
import { execFileSync } from "node:child_process"
import { cp, mkdir, rm } from "node:fs/promises"
import path from "node:path"
import { createRequire } from "node:module"


export const OPENAGENTS_DESKTOP_BUNDLE_ID = "com.openagents.desktop"
export const OPENAGENTS_DESKTOP_PROTOCOL = "openagents"

const ignoredCheckoutPath = /^\/(src|scripts|tests|docs|receipts)(\/|$)|^\/(README\.md|UPSTREAM\.md|GUARANTEES\.md|tsconfig\.json|forge\.config\.ts)$/
const resolveFromApp = createRequire(path.join(process.cwd(), "package.json"))

const copyRuntimePackage = async (
  buildPath: string,
  packageName: string,
  resolveSpecifier = packageName,
  ascend = 0,
): Promise<void> => {
  let source = path.dirname(resolveFromApp.resolve(resolveSpecifier))
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
    asar: {
      // Both provider packages resolve and spawn native executables relative
      // to their installed package. Executables cannot run inside app.asar.
      unpack: "**/node_modules/{@anthropic-ai/claude-agent-sdk*,@openai/codex*}/**/*",
    },
    // Forge's npm-oriented dependency walker cannot resolve Bun workspace
    // links. The release preflight/ASAR oracle enforces the artifact allowlist
    // after copying; disabling the broken prune avoids silently dropping a
    // provider package or one of its platform executables.
    prune: false,
    derefSymlinks: true,
    icon: "dist/assets/openagents-icon.png",
    ignore: path => ignoredCheckoutPath.test(path),
    protocols: [{ name: "OpenAgents", schemes: [OPENAGENTS_DESKTOP_PROTOCOL] }],
    osxSign: process.env.OA_DEVELOPER_ID_APPLICATION === undefined ? undefined : {
      identity: process.env.OA_DEVELOPER_ID_APPLICATION,
      optionsForFile: () => ({
        entitlements: "build/entitlements.mac.plist",
        hardenedRuntime: true,
      }),
    },
  },
  hooks: {
    generateAssets: async () => {
      execFileSync("bun", ["scripts/build.ts"], { cwd: process.cwd(), stdio: "inherit" })
    },
    packageAfterCopy: async (_forgeConfig, buildPath, _electronVersion, platform, arch) => {
      // Bun bundles every app dependency except the provider packages whose
      // native payloads must remain relative to their package. Replace the
      // copied workspace node_modules with that explicit runtime allowlist.
      await rm(path.join(buildPath, "node_modules"), { recursive: true, force: true })
      await copyRuntimePackage(buildPath, "@anthropic-ai/claude-agent-sdk")
      await copyRuntimePackage(buildPath, "@openai/codex", "@openai/codex/bin/codex.js", 1)
      await copyRuntimePackage(buildPath, `@openai/codex-${platform}-${arch}`, `@openai/codex-${platform}-${arch}/package.json`)
    },
  },
  makers: [
    new MakerDMG({ format: "ULFO", overwrite: true }, ["darwin"]),
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
      [FuseV1Options.LoadBrowserProcessSpecificV8Snapshot]: true,
      [FuseV1Options.GrantFileProtocolExtraPrivileges]: false,
      [FuseV1Options.WasmTrapHandlers]: true,
    }),
  ],
}

export default config
