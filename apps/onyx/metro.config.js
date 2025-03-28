const { getDefaultConfig } = require('expo/metro-config');
const path = require('path');

const projectRoot = __dirname;
const monorepoRoot = path.resolve(projectRoot, '../..');

const config = getDefaultConfig(projectRoot);

// Only list the packages within your monorepo that your app uses. No need to add anything else.
// If your monorepo tooling can give you the list of monorepo workspaces linked
// in your app workspace, you can automate this list instead of hardcoding them.
const monorepoPackages = {
  '@openagents/core': path.resolve(monorepoRoot, 'packages/core'),
  '@openagents/ui': path.resolve(monorepoRoot, 'packages/ui'),
};

// 1. Watch the local app directory, and only the shared packages (limiting the scope and speeding it up)
// Note how we change this from `monorepoRoot` to `projectRoot`. This is part of the optimization!
config.watchFolders = [projectRoot, ...Object.values(monorepoPackages)];

// Add the monorepo workspaces and MCP SDK as extraNodeModules
config.resolver.extraNodeModules = {
  ...monorepoPackages,
  '@modelcontextprotocol/sdk': path.resolve(monorepoRoot, 'node_modules/@modelcontextprotocol/sdk/dist/esm')
};

// 2. Let Metro know where to resolve packages and in what order
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(monorepoRoot, 'node_modules'),
  path.resolve(monorepoRoot, 'packages/core/node_modules'),
];

module.exports = config;
