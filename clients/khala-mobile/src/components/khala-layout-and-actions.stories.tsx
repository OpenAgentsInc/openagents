import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { ActivityIndicator } from "./activity-indicator"
import { ArwesButton } from "./arwes-button"
import { BackgroundGradient } from "./background-gradient"
import { KhalaButton } from "./khala-button"
import { KhalaEmptyState } from "./khala-empty-state"
import { KhalaListItem } from "./khala-list-item"
import { KhalaScreen } from "./khala-screen"
import { KhalaScrollToLatestButton } from "./khala-scroll-to-latest-button"
import { KhalaText } from "./khala-text"
import { KhalaThreadHeader } from "./khala-thread-header"
import { TouchableFeedback } from "./touchable-feedback"

const meta = {
  title: "Khala/Components/Layout And Actions",
  component: View,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

export const ScreenPresets: Story = {
  render: () => (
    <View className="h-full">
      <KhalaScreen preset="scroll" contentClassName="gap-4 p-5">
        <KhalaThreadHeader
          title="Fleet dispatch"
          subtitle="Codex is reviewing a run"
          onOpenMenu={() => undefined}
          onNewThread={() => undefined}
        />
        <KhalaListItem
          variant="surface"
          title="Queued follow-up"
          detail="Ask Claude to review the completed Codex turn."
          meta="2m"
        />
        <KhalaEmptyState
          title="No more transcript yet"
          detail="New runtime parts appear here as they stream in."
          tone="muted"
        />
      </KhalaScreen>
    </View>
  ),
}

export const ActionSurfaces: Story = {
  render: () => (
    <View className="gap-5 p-5">
      <View className="flex-row flex-wrap gap-3">
        <KhalaButton variant="primary" text="Dispatch" onPress={() => undefined} />
        <KhalaButton variant="secondary" text="Queue" onPress={() => undefined} />
        <KhalaButton variant="danger" text="Stop" onPress={() => undefined} />
        <KhalaButton variant="ghost" text="Dismiss" onPress={() => undefined} />
      </View>
      <View className="flex-row items-center gap-4">
        <KhalaButton loading variant="primary" text="Sending" />
        <ActivityIndicator />
        <ActivityIndicator type="large" size={112} />
        <KhalaScrollToLatestButton onPress={() => undefined} />
      </View>
      <TouchableFeedback className="rounded-lg border border-border bg-surfaceRaised p-4">
        <KhalaText>TouchableFeedback cross-fades on press.</KhalaText>
      </TouchableFeedback>
      <ArwesButton style={{ height: 64, width: 220 }} onPress={() => undefined}>
        <View className="flex-1 items-center justify-center">
          <KhalaText className="font-semibold text-accent">Arwes frame button</KhalaText>
        </View>
      </ArwesButton>
    </View>
  ),
}

export const Backgrounds: Story = {
  render: () => (
    <View className="p-5">
      <BackgroundGradient style={{ minHeight: 220, borderRadius: 12, overflow: "hidden" }}>
        <View className="flex-1 justify-end p-5">
          <KhalaText variant="heading">Animated background gradient</KhalaText>
          <KhalaText className="mt-2" variant="muted">
            Self-measuring Skia gradient behind ordinary layout children.
          </KhalaText>
        </View>
      </BackgroundGradient>
    </View>
  ),
}
