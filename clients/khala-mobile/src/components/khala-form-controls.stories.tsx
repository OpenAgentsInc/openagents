import type { Meta, StoryObj } from "@storybook/react-native"
import { useState } from "react"
import { View } from "react-native"

import { KhalaTextField } from "./khala-text-field"
import { Toggle } from "./toggle"

const meta = {
  title: "Khala/Components/Form Controls",
  component: View,
  parameters: { layout: "centered" },
  decorators: [
    (Story) => (
      <View className="w-full gap-5">
        <Story />
      </View>
    ),
  ],
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

const ToggleExample = () => {
  const [checked, setChecked] = useState(true)
  const [radio, setRadio] = useState(true)
  const [switched, setSwitched] = useState(false)

  return (
    <View className="gap-4">
      <Toggle label="Checkbox enabled" value={checked} onValueChange={setChecked} />
      <Toggle variant="radio" label="Radio selected" value={radio} onValueChange={setRadio} />
      <Toggle variant="switch" label="Switch inactive" value={switched} onValueChange={setSwitched} />
      <Toggle disabled label="Disabled checkbox" value />
      <Toggle disabled variant="switch" label="Disabled switch" value={false} />
    </View>
  )
}

export const TextFields: Story = {
  render: () => (
    <View className="gap-4">
      <KhalaTextField label="Repository" placeholder="OpenAgentsInc/openagents" />
      <KhalaTextField label="Branch" value="main" />
      <KhalaTextField label="Token" disabled value="Stored in keychain" />
      <KhalaTextField label="Run note" errorText="A note is required before dispatch." placeholder="Describe the task" />
      <KhalaTextField label="Plain text" mono={false} placeholder="Non-monospace input" />
    </View>
  ),
}

export const Toggles: Story = {
  render: () => <ToggleExample />,
}
