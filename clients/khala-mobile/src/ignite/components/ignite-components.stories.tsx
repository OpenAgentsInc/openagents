import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { Button } from "./Button"
import { Card } from "./Card"
import { EmptyState } from "./EmptyState"
import { Header } from "./Header"
import { Icon, PressableIcon } from "./Icon"
import { ListItem } from "./ListItem"
import { Screen } from "./Screen"
import { Text } from "./Text"
import { TextField } from "./TextField"

const meta = {
  title: "Ignite/Components",
  component: View,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

export const Typography: Story = {
  render: () => (
    <View style={{ gap: 12, padding: 20 }}>
      <Text preset="heading" text="Ignite heading" />
      <Text preset="subheading" text="Subheading for dense app surfaces" />
      <Text text="Default body copy using the ported Ignite Text primitive." />
      <Text size="xs" weight="medium" text="Extra small medium label" />
      <Text size="xxs" text="Tiny metadata" />
    </View>
  ),
}

export const ButtonsAndIcons: Story = {
  render: () => (
    <View style={{ gap: 14, padding: 20 }}>
      <Button text="Default button" onPress={() => undefined} />
      <Button preset="filled" text="Filled button" onPress={() => undefined} />
      <Button preset="reversed" text="Reversed button" onPress={() => undefined} />
      <Button disabled text="Disabled button" />
      <View style={{ flexDirection: "row", gap: 18, alignItems: "center" }}>
        <Icon icon="☰" />
        <Icon icon="✎" color="#4fd0ff" size={30} />
        <PressableIcon icon="↓" color="#e8c1b4" onPress={() => undefined} />
      </View>
    </View>
  ),
}

export const CardsAndLists: Story = {
  render: () => (
    <View style={{ gap: 16, padding: 20 }}>
      <Card
        heading="Run summary"
        content="Codex completed the local simulator pass and pushed the fix."
        footer="Updated just now"
      />
      <Card
        preset="reversed"
        heading="Queued handoff"
        content="Claude can review the completed runtime turn."
        footer="Tap to inspect"
        onPress={() => undefined}
      />
      <ListItem text="Thread list item" leftIcon="☰" rightIcon="›" bottomSeparator />
      <ListItem text="Pressable row" leftIcon="✎" rightIcon="↓" onPress={() => undefined} />
    </View>
  ),
}

export const FieldsAndStates: Story = {
  render: () => (
    <View style={{ gap: 18, padding: 20 }}>
      <TextField label="Repository" placeholder="OpenAgentsInc/openagents" />
      <TextField label="Notes" multiline placeholder="What should the agent do?" />
      <TextField status="error" label="Branch" value="feature/old" helper="Branch must be main." />
      <TextField status="disabled" label="Auth token" value="Stored securely" />
      <EmptyState heading="No runs yet" content="Start a thread to create the first run." button="Start run" />
      <EmptyState loading heading="Loading runs" content="Checking local state." />
      <EmptyState status="error" heading="Unavailable" content="The simulator connection dropped." />
    </View>
  ),
}

export const HeadersAndScreens: Story = {
  render: () => (
    <View style={{ height: "100%" }}>
      <Header
        title="Khala Code"
        leftIcon="☰"
        rightText="New"
        onLeftPress={() => undefined}
        onRightPress={() => undefined}
      />
      <Screen preset="scroll" safeAreaEdges={["bottom"]} contentContainerStyle={{ gap: 16, padding: 20 }}>
        <Text preset="subheading" text="Scrollable screen" />
        <Text text="This story keeps the Ignite Screen wrapper visible with safe-area and keyboard-aware layout." />
        {Array.from({ length: 6 }, (_, index) => (
          <ListItem key={index} text={`Screen row ${index + 1}`} rightIcon="›" bottomSeparator />
        ))}
      </Screen>
    </View>
  ),
}
