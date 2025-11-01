import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { SessionUpdateAgentMessageChunk } from '@/components/acp'
import { Colors } from '@/constants/theme'

const meta = {
  title: 'ACP/AgentMessage',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <SessionUpdateAgentMessageChunk content={{ type: 'text', text: '**Hello** from ACP!\n\n- bullet\n- list' }} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: SessionUpdateAgentMessageChunk</Text>
        <Text style={{ color: Colors.foreground }}>Renders a markdown agent message chunk.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'content: ContentBlock — use { type: "text", text } for markdown'}</Text>
      </View>
    </View>
  ),
}
