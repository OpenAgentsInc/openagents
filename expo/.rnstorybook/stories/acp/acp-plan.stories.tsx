import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View } from 'react-native'
import { SessionUpdatePlan } from '@/components/acp'
import type { PlanEntry } from '@/types/acp'
import { Colors } from '@/constants/theme'

const meta = {
  title: 'ACP/Plan',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => {
    const entries: ReadonlyArray<PlanEntry> = [
      { content: 'Parse repository', priority: 'high', status: 'completed' },
      { content: 'Search for vulnerabilities', priority: 'medium', status: 'in_progress' },
      { content: 'Write fixes', priority: 'medium', status: 'pending' },
    ]
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
        <SessionUpdatePlan entries={entries} />
      </View>
    )
  },
}
