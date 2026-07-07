import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { KhalaListItem } from "./khala-list-item"

const meta = {
  title: "Khala/Primitives/ListItem",
  component: KhalaListItem,
  args: {
    detail: "Last activity 2m ago",
    meta: "ready",
    title: "openagents",
    variant: "surface",
  },
  argTypes: {
    tone: {
      control: "select",
      options: ["default", "danger", "success", "warning"],
    },
    variant: {
      control: "select",
      options: ["plain", "surface"],
    },
  },
} satisfies Meta<typeof KhalaListItem>

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {}

export const Statuses: Story = {
  render: () => (
    <View className="gap-2">
      <KhalaListItem detail="Codex account is available." meta="ready" title="codex" tone="success" variant="surface" />
      <KhalaListItem detail="Waiting for the next heartbeat." meta="stale" title="pylon" tone="warning" variant="surface" />
      <KhalaListItem detail="Reconnect before dispatching work." meta="offline" title="khala-code" tone="danger" variant="surface" />
    </View>
  ),
}

export const Disabled: Story = {
  args: {
    detail: "This runtime is unavailable on this device.",
    disabled: true,
    meta: "disabled",
    title: "Apple Foundation Models",
    variant: "surface",
  },
}
