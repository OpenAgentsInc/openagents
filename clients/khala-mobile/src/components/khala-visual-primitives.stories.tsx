import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"
import { useSharedValue } from "react-native-reanimated"

import { BlurredPopupProvider, TouchablePopupHandler } from "./blurred-popup"
import { Frame } from "./frame"
import { KhalaListItem } from "./khala-list-item"
import { KhalaText } from "./khala-text"

const meta = {
  title: "Khala/Components/Visual Primitives",
  component: View,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

const HighlightedFrame = () => {
  const highlighted = useSharedValue(true)
  return (
    <Frame highlighted={highlighted} style={{ height: 96, padding: 18 }} visible>
      <KhalaText className="text-accent" variant="body">
        Highlighted frame state
      </KhalaText>
    </Frame>
  )
}

export const Frames: Story = {
  render: () => (
    <View className="gap-5 p-5">
      <Frame style={{ minHeight: 120, padding: 20 }} visible>
        <KhalaText variant="heading">Frame wrapper</KhalaText>
        <KhalaText className="mt-2" variant="muted">
          Self-measured Skia chrome behind normal React Native layout.
        </KhalaText>
      </Frame>
      <HighlightedFrame />
    </View>
  ),
}

export const BlurredPopup: Story = {
  render: () => (
    <BlurredPopupProvider>
      <View className="flex-1 justify-center gap-5 p-5">
        <KhalaText variant="muted">Long-press the row to open the blurred popup menu.</KhalaText>
        <TouchablePopupHandler
          options={[
            { label: "Quote", onPress: () => undefined },
            { label: "Copy", onPress: () => undefined },
            { label: "Dismiss", onPress: () => undefined },
          ]}
        >
          <KhalaListItem
            variant="surface"
            title="Popup target"
            detail="This row is measured on the UI thread before the popup opens."
            meta="hold"
          />
        </TouchablePopupHandler>
      </View>
    </BlurredPopupProvider>
  ),
}
