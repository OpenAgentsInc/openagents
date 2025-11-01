// Expo Metro config with optional Storybook integration
const { getDefaultConfig } = require('expo/metro-config')
const withStorybook = require('@storybook/react-native/metro/withStorybook')

const config = getDefaultConfig(__dirname)

module.exports = withStorybook(config, {
  // Only enable when explicitly opted-in
  enabled: process.env.EXPO_PUBLIC_USE_STORYBOOK === '1' || process.env.STORYBOOK_ENABLED === 'true',
  // Keep Storybook config in the standard location
  configPath: './.rnstorybook',
})

