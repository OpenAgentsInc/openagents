// Expo Metro config with optional Storybook integration
const { getDefaultConfig } = require('expo/metro-config')
const path = require('path')
const { withStorybook } = require('@storybook/react-native/metro/withStorybook')

const config = getDefaultConfig(__dirname)
// Allow importing TS source from local packages (tinyvex, tricoder)
config.watchFolders = Array.from(new Set([
  path.resolve(__dirname, '..', 'packages'),
]))
config.resolver = {
  ...config.resolver,
  extraNodeModules: {
    // Map local packages for monorepo-style imports
    tinyvex: path.resolve(__dirname, '..', 'packages', 'tinyvex', 'src'),
    tricoder: path.resolve(__dirname, '..', 'packages', 'tricoder', 'src'),
  },
}

module.exports = withStorybook(config, {
  // Only enable when explicitly opted-in
  enabled: process.env.EXPO_PUBLIC_USE_STORYBOOK === '1' || process.env.STORYBOOK_ENABLED === 'true',
  // Keep Storybook config in the standard location
  configPath: './.rnstorybook',
})
