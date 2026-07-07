import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { KhalaButton } from "./khala-button"
import { KhalaText } from "./khala-text"

const meta = {
  title: "Khala/Primitives/Button",
  component: KhalaButton,
  args: {
    text: "Start run",
    variant: "primary",
  },
  argTypes: {
    variant: {
      control: "select",
      options: ["primary", "secondary", "ghost", "danger"],
    },
  },
} satisfies Meta<typeof KhalaButton>

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {}

export const Variants: Story = {
  render: () => (
    <View className="gap-3">
      <KhalaButton text="Start run" variant="primary" />
      <KhalaButton text="Review changes" variant="secondary" />
      <KhalaButton text="Skip" variant="ghost" />
      <KhalaButton text="Disconnect" variant="danger" />
    </View>
  ),
}

export const Busy: Story = {
  args: {
    loading: true,
    text: "Syncing",
    variant: "secondary",
  },
}

export const Accessories: Story = {
  render: () => (
    <KhalaButton
      leftAccessory={<KhalaText variant="mono">cmd</KhalaText>}
      rightAccessory={<KhalaText variant="mono">enter</KhalaText>}
      text="Send command"
      variant="secondary"
    />
  ),
}
