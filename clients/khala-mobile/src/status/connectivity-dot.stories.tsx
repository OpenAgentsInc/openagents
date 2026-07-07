import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { KhalaText } from "../components/khala-text"
import { ConnectivityDot } from "./connectivity-dot"

const meta = {
  title: "Khala/Status/Connectivity Dot",
  component: ConnectivityDot,
  parameters: { layout: "centered" },
} satisfies Meta<typeof ConnectivityDot>

export default meta

type Story = StoryObj<typeof meta>

export const Live: Story = {
  render: () => (
    <View className="flex-row items-center gap-3">
      <ConnectivityDot />
      <KhalaText variant="muted">Live Khala Code desktop connectivity</KhalaText>
    </View>
  ),
}
