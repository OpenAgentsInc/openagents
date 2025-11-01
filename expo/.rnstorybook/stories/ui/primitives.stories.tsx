import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Text, Button, TextField, Card, ListItem, Switch, Checkbox, AutoImage, Screen } from '@/components/ui'

const meta = { title: 'UI/Primitives' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const TextVariants: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, gap: 8 }}>
      <Text variant="heading">Heading</Text>
      <Text variant="subheading">Subheading</Text>
      <Text variant="label">Label</Text>
      <Text>Body</Text>
      <Text variant="caption">Caption</Text>
      <Text variant="body" tone="secondary">Secondary tone</Text>
    </View>
  ),
}

export const Buttons: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, gap: 8 }}>
      <Button title="Primary" />
      <Button title="Secondary" variant="secondary" />
      <Button title="Ghost" variant="ghost" />
      <Button title="Delete" variant="destructive" />
      <Button title="Loading" loading={true} />
      <Button title="Disabled" disabled={true} />
    </View>
  ),
}

export const FieldsAndToggles: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, gap: 12 }}>
      <TextField label="Name" placeholder="Your name" />
      <TextField label="Password" placeholder="••••••" secureTextEntry={true} />
      <Switch label="Enable feature" value={true} />
      <Checkbox label="Accept terms" value={true} />
    </View>
  ),
}

export const CardsAndList: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, gap: 12 }}>
      <Card title="Card title">
        <Text>Card content goes here.</Text>
      </Card>
      <ListItem title="Item" subtitle="Subtitle" showChevron={true} />
    </View>
  ),
}

export const AutoImageStory: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <AutoImage source={{ uri: 'https://picsum.photos/200/100' }} />
    </View>
  ),
}

export const ScreenStory: Story = {
  render: () => (
    <Screen preset="scroll">
      <View style={{ padding: 16 }}>
        <Text>Inside Screen (scroll preset)</Text>
      </View>
    </Screen>
  ),
}

