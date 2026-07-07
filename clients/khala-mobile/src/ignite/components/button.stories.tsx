import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Button } from "./Button"
import { Icon } from "./Icon"

const meta = {
  title: "Ignite/Components/Button",
  component: Button,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof Button>

export default meta

type Story = StoryObj<typeof meta>

export const StylePresets: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="Default" usage="The default outline button.">
        <Button text="Click It" onPress={() => undefined} />
      </IgniteUseCase>
      <IgniteUseCase text="Filled" usage="The filled button preset.">
        <Button preset="filled" text="Click It" onPress={() => undefined} />
      </IgniteUseCase>
      <IgniteUseCase text="Reversed" usage="The reversed button preset.">
        <Button preset="reversed" text="Click It" onPress={() => undefined} />
      </IgniteUseCase>
      <IgniteUseCase text="Disabled" usage="Disabled behavior for each preset.">
        <Button disabled text="Default disabled" />
        <Button disabled preset="filled" text="Filled disabled" />
        <Button disabled preset="reversed" text="Reversed disabled" />
      </IgniteUseCase>
      <IgniteUseCase text="Array Style" usage="Button with style and textStyle arrays.">
        <Button
          text="Styled"
          onPress={() => undefined}
          style={[{ paddingVertical: 28 }, { borderRadius: 0 }]}
          textStyle={[{ fontSize: 20 }, { color: "#4fd0ff" }]}
        />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const Accessories: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="Left accessory" usage="A caller-supplied left component.">
        <Button
          text="Back"
          LeftAccessory={({ style }) => <Icon icon="<" color="#4fd0ff" containerStyle={style} />}
          onPress={() => undefined}
        />
      </IgniteUseCase>
      <IgniteUseCase text="Right accessory" usage="A caller-supplied right component.">
        <Button
          text="Continue"
          RightAccessory={({ style }) => <Icon icon=">" color="#4fd0ff" containerStyle={style} />}
          onPress={() => undefined}
        />
      </IgniteUseCase>
      <IgniteUseCase text="Children" usage="Content can be passed as children.">
        <Button onPress={() => undefined}>
          Nested button text
        </Button>
      </IgniteUseCase>
      <IgniteUseCase text="Layout row" usage="Buttons compose in normal React Native layout.">
        <View style={{ flexDirection: "row", gap: 10 }}>
          <Button text="Cancel" style={{ flex: 1 }} onPress={() => undefined} />
          <Button preset="reversed" text="Save" style={{ flex: 1 }} onPress={() => undefined} />
        </View>
      </IgniteUseCase>
    </IgniteStory>
  ),
}
