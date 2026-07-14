import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * Prepare the two native addons used by Electron Forge's DMG maker.
 *
 * Bun keeps transitive dependencies in its isolated `.bun` store. npm's
 * package-root `rebuild macos-alias` therefore reports success without
 * compiling either addon, and the release can fail only after the app has
 * already been signed and notarized. Resolve the exact installed packages
 * from that store and compile both before Forge starts.
 */
import { readdirSync } from "node:fs"
import path from "node:path"

const workspaceRoot = path.resolve(import.meta.dirname, "../../..")
const bunStore = path.join(workspaceRoot, "node_modules", ".bun")
const nativePackages = ["macos-alias", "fs-xattr"] as const

for (const packageName of nativePackages) {
  const prefix = `${packageName}@`
  const storeEntry = readdirSync(bunStore).find((entry) => entry.startsWith(prefix))
  if (storeEntry === undefined) {
    throw new Error(`missing ${packageName} in Bun's installed dependency store`)
  }

  const packageRoot = path.join(bunStore, storeEntry, "node_modules", packageName)
  const result = Runtime.spawnSync(
    ["npm", "exec", "--yes", "--package=node-gyp", "--", "node-gyp", "rebuild"],
    { cwd: packageRoot, stdout: "inherit", stderr: "inherit" },
  )
  if (result.exitCode !== 0) {
    throw new Error(`${packageName} native addon build failed with exit ${result.exitCode}`)
  }
}

console.log("[openagents-desktop] DMG maker native addons ready")
