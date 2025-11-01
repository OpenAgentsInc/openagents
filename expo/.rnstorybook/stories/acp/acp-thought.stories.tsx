import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
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
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: SessionUpdateAgentThoughtChunk</Text>
        <Text style={{ color: Colors.foreground }}>Renders an indented markdown “thought” block.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'content: ContentBlock — use { type: "text", text } for markdown'}</Text>
      </View>
    </View>
  ),
}
