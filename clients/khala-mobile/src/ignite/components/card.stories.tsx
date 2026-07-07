import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Card } from "./Card"
import { Icon } from "./Icon"
import { Text } from "./Text"

const meta = {
  title: "Ignite/Components/Card",
  component: Card,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof Card>

export default meta

type Story = StoryObj<typeof meta>

export const Presets: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="default" usage="The default card preset.">
        <Card heading="Run summary" content="Codex completed the local simulator pass." footer="Updated just now" />
      </IgniteUseCase>
      <IgniteUseCase text="reversed" usage="The reversed card preset.">
        <Card preset="reversed" heading="Queued handoff" content="Claude can review the completed runtime turn." footer="Tap to inspect" />
      </IgniteUseCase>
      <IgniteUseCase text="pressable" usage="Cards become touchable when onPress is supplied.">
        <Card heading="Pressable card" content="This card uses TouchableOpacity as its wrapper." onPress={() => undefined} />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const Alignment: Story = {
  render: () => (
    <IgniteStory>
      {(["top", "center", "space-between", "force-footer-bottom"] as const).map((verticalAlignment) => (
        <IgniteUseCase key={verticalAlignment} text={verticalAlignment} usage={`verticalAlignment="${verticalAlignment}"`}>
          <Card
            heading="Alignment"
            content="Body content sits according to the selected vertical alignment."
            footer="Footer"
            style={{ height: 150 }}
            verticalAlignment={verticalAlignment}
          />
        </IgniteUseCase>
      ))}
    </IgniteStory>
  ),
}

export const CustomComponents: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="LeftComponent / RightComponent" usage="Custom components can flank the card body.">
        <Card
          LeftComponent={<Icon icon="!" color="#4fd0ff" containerStyle={{ justifyContent: "center" }} />}
          RightComponent={<Icon icon=">" color="#4fd0ff" containerStyle={{ justifyContent: "center" }} />}
          heading="Attention"
          content="The port preserves the accessory slots."
        />
      </IgniteUseCase>
      <IgniteUseCase text="HeadingComponent / ContentComponent / FooterComponent" usage="Text sections can be replaced completely.">
        <Card
          HeadingComponent={<Text preset="subheading" text="Custom heading" />}
          ContentComponent={<Text text="Custom content component." />}
          FooterComponent={<Text preset="formHelper" text="Custom footer component." />}
        />
      </IgniteUseCase>
    </IgniteStory>
  ),
}
