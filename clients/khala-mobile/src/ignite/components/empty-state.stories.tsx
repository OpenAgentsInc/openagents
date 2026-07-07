import type { Meta, StoryObj } from "@storybook/react-native"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { EmptyState } from "./EmptyState"

const meta = {
  title: "Ignite/Components/EmptyState",
  component: EmptyState,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof EmptyState>

export default meta

type Story = StoryObj<typeof meta>

export const States: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="empty" usage="No data branch with action.">
        <EmptyState heading="No runs yet" content="Start a thread to create the first run." button="Start run" />
      </IgniteUseCase>
      <IgniteUseCase text="loading" usage="Loading branch with spinner.">
        <EmptyState loading heading="Loading runs" content="Checking local state." />
      </IgniteUseCase>
      <IgniteUseCase text="error" usage="Error branch with danger tint.">
        <EmptyState status="error" heading="Unavailable" content="The simulator connection dropped." button="Retry" />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const Styling: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="Text style overrides" usage="Heading/content/button text overrides.">
        <EmptyState
          heading="Styled empty state"
          headingStyle={{ color: "#4fd0ff" }}
          content="Content style is also overridable."
          contentStyle={{ color: "#dbe7f4" }}
          button="Styled action"
          buttonTextStyle={{ color: "#02060d" }}
        />
      </IgniteUseCase>
      <IgniteUseCase text="Container style" usage="Container layout override.">
        <EmptyState
          heading="Padded state"
          content="This one has a border and padding supplied by the caller."
          style={{ borderColor: "#4fd0ff", borderWidth: 1, padding: 24 }}
        />
      </IgniteUseCase>
    </IgniteStory>
  ),
}
