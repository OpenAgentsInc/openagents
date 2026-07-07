import type { Meta, StoryObj } from "@storybook/react-native"
import { useState } from "react"
import { View } from "react-native"

import { Toggle } from "../../components/toggle"
import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"

const meta = {
  title: "Ignite/Components/Checkbox And Switch",
  component: View,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

const ToggleState = ({
  initial = false,
  label,
  variant = "checkbox",
}: {
  initial?: boolean
  label: string
  variant?: "checkbox" | "radio" | "switch"
}) => {
  const [value, setValue] = useState(initial)
  return <Toggle label={label} value={value} variant={variant} onValueChange={setValue} />
}

export const CheckboxBehaviour: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="The Checkbox" usage="Use the checkbox to represent on/off states.">
        <ToggleState label="Toggle me" />
      </IgniteUseCase>
      <IgniteUseCase text="value = true" usage="This is permanently on.">
        <Toggle label="Always on" value />
      </IgniteUseCase>
      <IgniteUseCase text="value = false" usage="This is permanently off.">
        <Toggle label="Always off" value={false} />
      </IgniteUseCase>
      <IgniteUseCase text="radio" usage="The newer Khala toggle also exposes radio state.">
        <ToggleState initial label="Radio selected" variant="radio" />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const CheckboxStyling: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="multiline = true" usage="For really really long text.">
        <ToggleState label="We are an app design and development team. Experts in mobile and web technologies. We create beautiful, functional mobile apps and websites." />
      </IgniteUseCase>
      <IgniteUseCase text=".style" usage="Override the container style.">
        <View style={{ backgroundColor: "purple", marginLeft: 40, paddingLeft: 24, paddingVertical: 18 }}>
          <ToggleState label="Hello there!" />
        </View>
      </IgniteUseCase>
      <IgniteUseCase text="disabled" usage="Disabled checkbox styling.">
        <Toggle disabled label="Disabled checkbox" value />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const SwitchBehaviour: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="The Toggle Switch" usage="Use the switch to represent on/off states.">
        <ToggleState label="Toggle me" variant="switch" />
      </IgniteUseCase>
      <IgniteUseCase text="value = true" usage="This is permanently on.">
        <Toggle label="Always on" value variant="switch" />
      </IgniteUseCase>
      <IgniteUseCase text="value = false" usage="This is permanently off.">
        <Toggle label="Always off" value={false} variant="switch" />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const SwitchStyling: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="Custom Styling" usage="The new switch is token-driven, so the style story demonstrates layout wrappers.">
        <View style={{ borderColor: "#686868", borderWidth: 3, padding: 14 }}>
          <ToggleState initial label="Wrapped switch" variant="switch" />
        </View>
      </IgniteUseCase>
      <IgniteUseCase text="Disabled" usage="Disabled switch styling.">
        <Toggle disabled label="Disabled switch" value={false} variant="switch" />
      </IgniteUseCase>
    </IgniteStory>
  ),
}
