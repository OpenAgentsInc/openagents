import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { SessionUpdateUserMessageChunk } from '@/components/acp'
import { Colors } from '@/constants/theme'

const meta = { title: 'ACP/UserMessage' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <SessionUpdateUserMessageChunk content={{ type: 'text', text: 'User asks: “What changed?”' }} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: SessionUpdateUserMessageChunk</Text>
        <Text style={{ color: Colors.foreground }}>Renders a user message bubble/row.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'content: ContentBlock — use { type: "text", text } for plain text'}</Text>
      </View>
    </View>
  ),
}

