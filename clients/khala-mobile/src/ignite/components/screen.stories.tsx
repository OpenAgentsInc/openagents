import type { Meta, StoryObj } from "@storybook/react-native"
import type { ReactNode } from "react"
import { ScrollView, View, type ViewStyle } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Header } from "./Header"
import { ListItem } from "./ListItem"
import { Text } from "./Text"

type StorybookScreenProps = Readonly<{
  children?: ReactNode
  contentContainerStyle?: ViewStyle
  preset?: "fixed" | "scroll"
}>

const StorybookScreen = ({ children, contentContainerStyle, preset = "fixed" }: StorybookScreenProps) => (
  <SafeAreaView edges={["bottom"]} style={{ flex: 1 }}>
    {preset === "scroll" ? (
      <ScrollView contentContainerStyle={contentContainerStyle}>{children}</ScrollView>
    ) : (
      <View style={[{ flex: 1 }, contentContainerStyle]}>{children}</View>
    )}
  </SafeAreaView>
)

const meta = {
  title: "Ignite/Components/Screen",
  component: StorybookScreen,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof StorybookScreen>

export default meta

type Story = StoryObj<typeof meta>

export const Fixed: Story = {
  render: () => (
    <View style={{ flex: 1 }}>
      <Header title="Fixed screen" leftIcon="<" rightText="New" safeAreaEdges={["top"]} />
      <StorybookScreen preset="fixed" contentContainerStyle={{ padding: 20 }}>
        <View style={{ gap: 14 }}>
          <Text preset="subheading" text="Fixed content" />
          <Text text="This avoids navigation-only scroll hooks in Storybook." />
          <ListItem rightIcon=">" text="Screen row 1" bottomSeparator />
          <ListItem rightIcon=">" text="Screen row 2" bottomSeparator />
        </View>
      </StorybookScreen>
    </View>
  ),
}

export const PresetsWithoutNavigationHooks: Story = {
  render: () => (
    <IgniteStoryScreen>
      <IgniteStory>
        <IgniteUseCase text="fixed" usage="The non-scrolling preset.">
          <View style={{ height: 220 }}>
            <StorybookScreen preset="fixed" contentContainerStyle={{ padding: 14 }}>
              <Text text="Fixed screen body." />
            </StorybookScreen>
          </View>
        </IgniteUseCase>
        <IgniteUseCase text="scroll" usage="Route-free Storybook scroll surface.">
          <View style={{ height: 220 }}>
            <StorybookScreen preset="scroll" contentContainerStyle={{ gap: 10, padding: 14 }}>
              {Array.from({ length: 8 }, (_, index) => (
                <ListItem key={index} text={`Scrollable row ${index + 1}`} bottomSeparator />
              ))}
            </StorybookScreen>
          </View>
        </IgniteUseCase>
      </IgniteStory>
    </IgniteStoryScreen>
  ),
}
