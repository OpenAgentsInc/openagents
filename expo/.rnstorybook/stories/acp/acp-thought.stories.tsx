import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View } from 'react-native'
import { SessionUpdateAgentThoughtChunk } from '@/components/acp'
import { Colors } from '@/constants/theme'

const meta = {
  title: 'ACP/AgentThought',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <SessionUpdateAgentThoughtChunk content={{ type: 'text', text: '_Thinking_ about how to structure this.' }} />
    </View>
  ),
}
