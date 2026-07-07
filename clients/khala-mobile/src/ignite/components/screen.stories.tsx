import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Header } from "./Header"
import { ListItem } from "./ListItem"
import { Screen } from "./Screen"
import { Text } from "./Text"

const meta = {
  title: "Ignite/Components/Screen",
  component: Screen,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof Screen>

export default meta

type Story = StoryObj<typeof meta>

export const Fixed: Story = {
  render: () => (
    <View style={{ flex: 1 }}>
      <Header title="Fixed screen" leftIcon="<" rightText="New" safeAreaEdges={["top"]} />
      <Screen preset="fixed" safeAreaEdges={["bottom"]} contentContainerStyle={{ padding: 20 }}>
        <View style={{ gap: 14 }}>
          <Text preset="subheading" text="Fixed content" />
          <Text text="This avoids navigation-only scroll hooks in Storybook." />
          <ListItem rightIcon=">" text="Screen row 1" bottomSeparator />
          <ListItem rightIcon=">" text="Screen row 2" bottomSeparator />
        </View>
      </Screen>
    </View>
  ),
}

export const PresetsWithoutNavigationHooks: Story = {
  render: () => (
    <IgniteStoryScreen>
      <IgniteStory>
        <IgniteUseCase text="fixed" usage="The non-scrolling preset.">
          <View style={{ height: 220 }}>
            <Screen preset="fixed" contentContainerStyle={{ padding: 14 }}>
              <Text text="Fixed screen body." />
            </Screen>
          </View>
        </IgniteUseCase>
        <IgniteUseCase text="auto/scroll note" usage="In app navigation these use scroll hooks; Storybook examples keep fixed to avoid route-object errors.">
          <Text text="The production Screen supports fixed, scroll, and auto. Storybook renders the route-free safe fixed path." />
        </IgniteUseCase>
      </IgniteStory>
    </IgniteStoryScreen>
  ),
}
