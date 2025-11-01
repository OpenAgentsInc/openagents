import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { SessionUpdateCurrentModeUpdate } from '@/components/acp'
import { Colors } from '@/constants/theme'

const meta = {
  title: 'ACP/CurrentMode',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const Review: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <SessionUpdateCurrentModeUpdate currentModeId={'review'} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: SessionUpdateCurrentModeUpdate</Text>
        <Text style={{ color: Colors.foreground }}>Displays a simple marker indicating the current mode id.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'currentModeId: string — current active mode key'}</Text>
      </View>
    </View>
  ),
}
