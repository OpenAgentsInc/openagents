import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Text } from "./Text"

const meta = {
  title: "Ignite/Components/Text",
  component: Text,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof Text>

export default meta

type Story = StoryObj<typeof meta>

export const StylePresets: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="default" usage="Used for normal body text.">
        <Text>Hello!</Text>
        <Text style={{ paddingTop: 10 }}>Check out{"\n"}my{"\n"}line height</Text>
        <Text style={{ paddingTop: 10 }}>The quick brown fox jumped over the slow lazy dog.</Text>
        <Text>$123,456,789.00</Text>
      </IgniteUseCase>
      <IgniteUseCase text="bold" usage="Used for bolded body text.">
        <Text preset="bold">Osnap! I'm puffy.</Text>
      </IgniteUseCase>
      <IgniteUseCase text="heading" usage="Used for major section headers.">
        <Text preset="heading">Behold!</Text>
      </IgniteUseCase>
      <IgniteUseCase text="subheading" usage="Used for compact section headers.">
        <Text preset="subheading">Operational surface</Text>
      </IgniteUseCase>
      <IgniteUseCase text="formLabel / formHelper" usage="Used around text fields.">
        <Text preset="formLabel">Repository</Text>
        <Text preset="formHelper">Choose a repository connected to this account.</Text>
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const PassingContent: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="text" usage="Pass a value without opening a child.">
        <Text text="Heyo!" />
      </IgniteUseCase>
      <IgniteUseCase text="tx" usage="Look up i18n keys through the Ignite translate surface.">
        <Text tx="common.ok" />
        <Text tx="common.cancel" />
      </IgniteUseCase>
      <IgniteUseCase text="children" usage="Use it like React Native Text.">
        <Text>Passing strings as children.</Text>
      </IgniteUseCase>
      <IgniteUseCase text="nested children" usage="Embed them and change styles too.">
        <Text>Hello <Text preset="bold">bolded</Text> World.</Text>
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const SizeAndWeight: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="sizes" usage="Every current Ignite size token.">
        {(["xxl", "xl", "lg", "md", "sm", "xs", "xxs"] as const).map((size) => (
          <Text key={size} size={size} text={`size=${size}`} />
        ))}
      </IgniteUseCase>
      <IgniteUseCase text="weights" usage="Every primary font weight exposed by the port.">
        {(["light", "normal", "medium", "semiBold", "bold"] as const).map((weight) => (
          <Text key={weight} weight={weight} text={`weight=${weight}`} />
        ))}
      </IgniteUseCase>
      <IgniteUseCase text="Style array" usage="Text with style array.">
        <View style={[{ backgroundColor: "#08111f" }, { borderColor: "#4fd0ff", borderWidth: 1, padding: 12 }]}>
          <Text>Hello <Text preset="bold">bolded</Text> World.</Text>
        </View>
      </IgniteUseCase>
    </IgniteStory>
  ),
}
