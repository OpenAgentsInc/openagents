const path = require("path")
const { getDefaultConfig } = require("expo/metro-config")

const projectRoot = __dirname
const monorepoRoot = path.resolve(projectRoot, "../../..")

const config = getDefaultConfig(projectRoot)

config.watchFolders = [monorepoRoot]
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, "node_modules"),
  path.resolve(monorepoRoot, "node_modules"),
]
config.resolver.disableHierarchicalLookup = true

module.exports = config
