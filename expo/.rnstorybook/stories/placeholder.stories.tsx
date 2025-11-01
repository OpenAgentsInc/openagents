import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'

const meta = {
  title: 'Example/Placeholder',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ padding: 16 }}>
      <Text>Hello Storybook</Text>
    </View>
  ),
}

