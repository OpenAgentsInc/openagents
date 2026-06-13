const path = require("path")
const { getDefaultConfig } = require("expo/metro-config")

const projectRoot = __dirname
const monorepoRoot = path.resolve(projectRoot, "../../..")

const config = getDefaultConfig(projectRoot)

// Monorepo resolution. Watch the repo root so Metro transforms the workspace
// package (@openagentsinc/autopilot-control-protocol), and add the root
// node_modules to the search paths.
config.watchFolders = [monorepoRoot]
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, "node_modules"),
  path.resolve(monorepoRoot, "node_modules"),
]
// NOTE: do NOT set `disableHierarchicalLookup = true`. That is correct for
// npm/yarn flat hoisting, but Bun uses an isolated `.bun` store where transitive
// deps (expo-modules-core, etc.) are only reachable by walking up the symlinked
// node_modules tree — disabling hierarchical lookup makes the EAS bundle fail
// with "Unable to resolve module expo-modules-core".

module.exports = config
