import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View } from 'react-native'
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
      <SessionUpdateAgentMessageChunk content={{ type: 'text', text: '**Hello** from ACP!\n\n- bullet\n- list' } as any} />
    </View>
  ),
}

