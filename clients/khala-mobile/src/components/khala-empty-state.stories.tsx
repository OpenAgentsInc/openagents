import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { KhalaEmptyState } from "./khala-empty-state"

const meta = {
  title: "Khala/Primitives/EmptyState",
  component: KhalaEmptyState,
  args: {
    detail: "Connect a workspace to start a local Khala Code session.",
    title: "No workspace selected",
    tone: "muted",
  },
  argTypes: {
    tone: {
      control: "select",
      options: ["accent", "danger", "muted"],
    },
  },
} satisfies Meta<typeof KhalaEmptyState>

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {}

export const Loading: Story = {
  args: {
    detail: "Preparing the local runtime.",
    loading: true,
    title: "Starting Khala Code",
    tone: "accent",
  },
}

export const Tones: Story = {
  render: () => (
    <View className="gap-3">
      <KhalaEmptyState detail="Waiting for a local session." title="Idle" tone="muted" />
      <KhalaEmptyState detail="New messages are ready." title="Ready" tone="accent" />
      <KhalaEmptyState detail="Authentication needs attention." title="Action required" tone="danger" />
    </View>
  ),
}
