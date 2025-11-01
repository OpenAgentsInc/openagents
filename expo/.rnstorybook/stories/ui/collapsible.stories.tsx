import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { Collapsible } from '@/components/ui'
import { Colors } from '@/constants/theme'

const meta = { title: 'UI/Collapsible' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <Collapsible open={true}>
        <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 12 }}>
          <Text style={{ color: Colors.foreground }}>This content is visible.</Text>
        </View>
      </Collapsible>
    </View>
  ),
}

export const Closed: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <Collapsible open={false}>
        <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 12 }}>
          <Text style={{ color: Colors.foreground }}>Hidden content</Text>
        </View>
      </Collapsible>
    </View>
  ),
}
