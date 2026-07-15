import { Runtime } from "@openagentsinc/runtime-platform"
import { createRequire } from "node:module"
import path from "node:path"

/**
 * Compile the native addons used by Electron Forge's DMG maker for the exact
 * Node runtime driving the release. A shared pnpm store can retain binaries
 * built by another Node ABI even when the source worktree itself is clean.
 */
const workspaceRoot = path.resolve(import.meta.dirname, "../../..")
const require = createRequire(import.meta.url)
const nodeGyp = path.join(workspaceRoot, "node_modules", ".bin", "node-gyp")
const nativePackages = ["macos-alias", "fs-xattr"] as const

for (const packageName of nativePackages) {
  const packageJson = require.resolve(`${packageName}/package.json`, {
    paths: [workspaceRoot],
  })
  const result = Runtime.spawnSync([nodeGyp, "rebuild"], {
    cwd: path.dirname(packageJson),
    stdout: "inherit",
    stderr: "inherit",
  })
  if (result.exitCode !== 0) {
    throw new Error(`${packageName} native addon build failed with exit ${result.exitCode}`)
  }
}

console.log(`[openagents-desktop] DMG maker native addons ready for Node ABI ${process.versions.modules}`)
