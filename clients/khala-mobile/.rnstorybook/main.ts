import type { StorybookConfig } from "@storybook/react-native"

const main: StorybookConfig = {
  stories: ["../src/**/*.stories.?(ts|tsx)"],
  deviceAddons: [
    "@storybook/addon-ondevice-actions",
    "@storybook/addon-ondevice-backgrounds",
    "@storybook/addon-ondevice-controls",
  ],
}

export default main
