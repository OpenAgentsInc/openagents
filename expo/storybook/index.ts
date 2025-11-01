import { getStorybookUI } from '@storybook/react-native'
import './storybook.requires'

// On-device addons
import '@storybook/addon-ondevice-actions/register'
import '@storybook/addon-ondevice-controls/register'

const StorybookUIRoot = getStorybookUI({
  shouldPersistSelection: true,
})

export default StorybookUIRoot

