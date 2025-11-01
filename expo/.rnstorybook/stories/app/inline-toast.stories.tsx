import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { InlineToast } from '@/components/inline-toast'
import { Colors } from '@/constants/theme'

const meta = { title: 'App/InlineToast' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Positions: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Text style={{ color: Colors.foreground, margin: 16 }}>Inline toasts appear absolutely positioned.</Text>
      <InlineToast text="Copied" position="bottom" align="right" />
      <InlineToast text="Saved" position="top" align="left" />
    </View>
  ),
}

