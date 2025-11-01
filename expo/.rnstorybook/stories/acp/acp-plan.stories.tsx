import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View } from 'react-native'
import { SessionUpdatePlan } from '@/components/acp'
import { Colors } from '@/constants/theme'

const meta = {
  title: 'ACP/Plan',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => {
    const entries = [
      { content: 'Parse repository', priority: 'high', status: 'completed' },
      { content: 'Search for vulnerabilities', priority: 'medium', status: 'in_progress' },
      { content: 'Write fixes', priority: 'medium', status: 'pending' },
    ] as const
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
        <SessionUpdatePlan entries={entries as any} />
      </View>
    )
  },
}

