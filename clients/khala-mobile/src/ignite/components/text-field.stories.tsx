import type { Meta, StoryObj } from "@storybook/react-native"
import { useRef, useState } from "react"
import type { TextInput } from "react-native"
import { View } from "react-native"

import { IgniteStory, IgniteStoryScreen, IgniteUseCase } from "../storybook-layout"
import { Text } from "./Text"
import { TextField } from "./TextField"

const meta = {
  title: "Ignite/Components/TextField",
  component: TextField,
  decorators: [(Story) => <IgniteStoryScreen><Story /></IgniteStoryScreen>],
} satisfies Meta<typeof TextField>

export default meta

type Story = StoryObj<typeof meta>

const ControlledTextField = (props: Omit<React.ComponentProps<typeof TextField>, "value" | "onChangeText"> & { initial?: string }) => {
  const [value, setValue] = useState(props.initial ?? "")
  return <TextField {...props} value={value} onChangeText={setValue} />
}

const RefForwardingExample = () => {
  const ref = useRef<TextInput>(null)
  return (
    <TextField
      ref={ref}
      label="Name"
      value="fancy colour"
      onChangeText={() => undefined}
      inputWrapperStyle={{ borderColor: "hotpink", borderRadius: 4, borderWidth: 6 }}
      style={{ backgroundColor: "rebeccapurple", color: "white", padding: 16 }}
      onFocus={() => undefined}
    />
  )
}

export const Labelling: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="Normal text" usage="Use placeholder and label to set the text.">
        <ControlledTextField label="Name" placeholder="omg your name" />
      </IgniteUseCase>
      <IgniteUseCase text="i18n text" usage="The Khala port exposes plain strings; tx keys are covered on Text/Header.">
        <ControlledTextField label="storybook.field" placeholder="storybook.placeholder" />
      </IgniteUseCase>
      <IgniteUseCase text="helper" usage="The newer Ignite TextField includes helper text.">
        <ControlledTextField label="Repository" helper="Use owner/name format." placeholder="OpenAgentsInc/openagents" />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const StyleOverrides: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase noPad text="Container Styles" usage="Useful for applying margins when a form brings its own padding.">
        <ControlledTextField initial="Inigo" label="First Name" containerStyle={{ paddingTop: 0, paddingHorizontal: 40 }} />
        <ControlledTextField initial="Montoya" label="Last Name" containerStyle={{ paddingBottom: 0 }} />
      </IgniteUseCase>
      <IgniteUseCase text="Input Styles" usage="Useful for one-off exceptions.">
        <ControlledTextField
          initial="fancy colour"
          label="Name"
          inputWrapperStyle={{ borderColor: "hotpink", borderRadius: 4, borderWidth: 6 }}
          style={{ backgroundColor: "rebeccapurple", color: "white", padding: 16 }}
        />
        <Text text="* attention designers: i am so sorry" preset="formHelper" />
      </IgniteUseCase>
      <IgniteUseCase text="Style array" usage="Useful for one-off exceptions, but using style arrays.">
        <ControlledTextField
          initial="fancy colour"
          label="Name"
          containerStyle={[{ paddingHorizontal: 30 }, { borderWidth: 8, borderColor: "#4fd0ff" }]}
          style={[
            { backgroundColor: "rebeccapurple", color: "white", padding: 16 },
            { borderWidth: 4, borderRadius: 4, borderColor: "#7fff00" },
          ]}
        />
        <Text text="* attention designers: i am still sorry" preset="formHelper" />
      </IgniteUseCase>
    </IgniteStory>
  ),
}

export const StatesAndAccessories: Story = {
  render: () => (
    <IgniteStory>
      <IgniteUseCase text="error" usage="Error status and helper text.">
        <ControlledTextField initial="feature/old" status="error" label="Branch" helper="Branch must be main." />
      </IgniteUseCase>
      <IgniteUseCase text="disabled" usage="Disabled status.">
        <TextField status="disabled" label="Auth token" value="Stored securely" />
      </IgniteUseCase>
      <IgniteUseCase text="multiline" usage="Multiline input wrapper.">
        <ControlledTextField multiline label="Run note" placeholder="Describe the task" />
      </IgniteUseCase>
      <IgniteUseCase text="Ref Forwarding" usage="The component forwards its TextInput ref.">
        <RefForwardingExample />
      </IgniteUseCase>
    </IgniteStory>
  ),
}
