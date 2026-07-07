import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Header } from "./Header"

const meta = {
  title: "Ignite/Components/Header",
  component: Header,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof Header>

export default meta

type Story = StoryObj<typeof meta>

export const Behavior: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase noPad text="default" usage="The default usage.">
        <Header titleTx="demoScreen.howTo" safeAreaEdges={[]} />
      </IgniteUseCase>
      <IgniteUseCase noPad text="leftIcon" usage="A left nav icon.">
        <Header titleTx="demoScreen.howTo" leftIcon="<" onLeftPress={() => undefined} safeAreaEdges={[]} />
      </IgniteUseCase>
      <IgniteUseCase noPad text="rightIcon" usage="A right nav icon.">
        <Header titleTx="demoScreen.howTo" rightIcon="•" onRightPress={() => undefined} safeAreaEdges={[]} />
      </IgniteUseCase>
      <IgniteUseCase noPad text="leftText / rightText" usage="Text actions on both sides.">
        <Header title="Khala Code" leftText="Back" rightText="New" onLeftPress={() => undefined} onRightPress={() => undefined} safeAreaEdges={[]} />
      </IgniteUseCase>
      <IgniteUseCase noPad text="flex title" usage="A title that flexes between actions.">
        <Header title="A long title that can flex" titleMode="flex" leftIcon="<" rightText="Save" safeAreaEdges={[]} />
      </IgniteUseCase>
    </IgniteStory>
  ),
}
