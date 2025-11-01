import type { StorybookConfig } from '@storybook/react-native'

const main: StorybookConfig = {
  // Point to stories inside this folder for now; we can extend later
  stories: ['../.rnstorybook/stories/**/*.stories.@(ts|tsx|js|jsx)'],
  addons: [
    '@storybook/addon-ondevice-actions',
    '@storybook/addon-ondevice-controls',
  ],
}

export default main

