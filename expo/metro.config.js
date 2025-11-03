// Expo Metro config with optional Storybook integration
const { getDefaultConfig } = require('expo/metro-config')
const path = require('path')
// Storybook wrapper may be ESM-only in some environments; make it optional.
let withStorybook = (cfg, _opts) => cfg
try {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  withStorybook = require('@storybook/react-native/metro/withStorybook').withStorybook || require('@storybook/react-native/metro/withStorybook')
} catch (e) {
  // Fallback to identity when the module cannot be required by Node (e.g., ESM-only)
}

const config = getDefaultConfig(__dirname)
// Allow importing TS source from local packages (tinyvex, tricoder)
config.watchFolders = Array.from(new Set([
  path.resolve(__dirname, '..', 'packages'),
]))
config.resolver = {
  ...config.resolver,
  // Ensure external packages resolve React and other deps from the app's node_modules
  nodeModulesPaths: [
    path.resolve(__dirname, 'node_modules'),
    path.resolve(__dirname, '..', 'node_modules'),
  ],
  extraNodeModules: {
    // Map local packages for monorepo-style imports
    tinyvex: path.resolve(__dirname, '..', 'packages', 'tinyvex', 'src'),
    tricoder: path.resolve(__dirname, '..', 'packages', 'tricoder', 'src'),
    '@openagentsinc/core': path.resolve(__dirname, '..', 'packages', 'openagents-core', 'src'),
    // Ensure Zustand middleware resolves to CJS variant (via shim directory)
    'zustand/middleware': path.resolve(__dirname, 'shims', 'zustand-middleware'),
  },
}

module.exports = withStorybook(config, {
  // Only enable when explicitly opted-in
  enabled: process.env.EXPO_PUBLIC_USE_STORYBOOK === '1' || process.env.STORYBOOK_ENABLED === 'true',
  // Keep Storybook config in the standard location
  configPath: './.rnstorybook',
})
