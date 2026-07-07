import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Icon, PressableIcon } from "./Icon"

const meta = {
  title: "Ignite/Components/Icon",
  component: View,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

export const Names: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="back" usage="The icon for going back.">
        <Icon icon="<" />
      </IgniteUseCase>
      <IgniteUseCase text="bullet" usage="The icon for a bullet point.">
        <Icon icon="•" />
      </IgniteUseCase>
      <IgniteUseCase text="Khala glyphs" usage="The newer port accepts any glyph string.">
        <View style={{ flexDirection: "row", gap: 18 }}>
          <Icon icon="☰" />
          <Icon icon="+" color="#4fd0ff" size={32} />
          <Icon icon="↓" color="#e8c1b4" size={32} />
          <Icon icon="!" color="#ff6b6b" size={32} />
        </View>
      </IgniteUseCase>
      <IgniteUseCase text="PressableIcon" usage="The touchable icon variant.">
        <PressableIcon icon=">" color="#4fd0ff" onPress={() => undefined} />
      </IgniteUseCase>
    </IgniteStory>
  ),
}
