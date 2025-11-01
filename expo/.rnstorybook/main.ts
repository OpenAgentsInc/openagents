import type { StorybookConfig } from '@storybook/react-native'

const main: StorybookConfig = {
  // Story files
  stories: ['../.rnstorybook/stories/**/*.stories.@(ts|tsx|js|jsx)'],
  addons: [
    '@storybook/addon-ondevice-actions',
    '@storybook/addon-ondevice-controls',
  ],
  // Ensure a stable default landing story
  reactNative: {
    initialSelection: { kind: 'App/Home', name: 'Default' },
  },
}

export default main
