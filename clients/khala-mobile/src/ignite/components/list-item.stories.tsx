import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Icon } from "./Icon"
import { ListItem } from "./ListItem"
import { Text } from "./Text"

const meta = {
  title: "Ignite/Components/ListItem",
  component: ListItem,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof ListItem>

export default meta

type Story = StoryObj<typeof meta>

export const Behavior: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="default" usage="The default list row.">
        <ListItem text="Thread list item" />
      </IgniteUseCase>
      <IgniteUseCase text="pressable" usage="Rows become touchable when onPress is supplied.">
        <ListItem text="Pressable row" onPress={() => undefined} />
      </IgniteUseCase>
      <IgniteUseCase text="children" usage="Rows can receive children.">
        <ListItem>
          <Text text="Child content" />
        </ListItem>
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const IconsAndSeparators: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="leftIcon" usage="A left action glyph.">
        <ListItem leftIcon="<" text="Back row" />
      </IgniteUseCase>
      <IgniteUseCase text="rightIcon" usage="A right action glyph.">
        <ListItem rightIcon=">" text="Open details" />
      </IgniteUseCase>
      <IgniteUseCase text="both icons" usage="Left and right icon slots together.">
        <ListItem leftIcon="☰" rightIcon=">" text="Navigation row" />
      </IgniteUseCase>
      <IgniteUseCase text="separators" usage="Top and bottom separator options.">
        <ListItem topSeparator bottomSeparator text="Separated row" />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const CustomComponents: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="LeftComponent / RightComponent" usage="Custom React elements can replace icon slots.">
        <ListItem
          LeftComponent={<Icon icon="!" color="#4fd0ff" containerStyle={{ justifyContent: "center", marginEnd: 12 }} />}
          RightComponent={<Icon icon="↓" color="#e8c1b4" containerStyle={{ justifyContent: "center", marginStart: 12 }} />}
          text="Custom component row"
        />
      </IgniteUseCase>
      <IgniteUseCase text="height / textStyle" usage="Height and text style overrides.">
        <ListItem height={76} text="Tall styled row" textStyle={{ color: "#4fd0ff", fontSize: 18 }} />
      </IgniteUseCase>
    </IgniteStory>
  ),
}
