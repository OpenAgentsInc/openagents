import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, ScrollView } from 'react-native'
import { SessionUpdateToolCall } from '@/components/acp'
import type { ToolCallLike } from '@/types/acp'
import { Colors } from '@/constants/theme'

const meta = {
  title: 'ACP/ToolCall',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const ExecuteInProgress: Story = {
  render: () => {
    const call: ToolCallLike = {
      title: 'Run: bun test',
      kind: 'execute',
      status: 'in_progress',
      content: [{ type: 'content', content: { type: 'text', text: 'Running tests...' } }],
      locations: [],
    }
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
        <SessionUpdateToolCall {...call} />
      </View>
    )
  },
}

export const EditCompleted: Story = {
  render: () => {
    const call: ToolCallLike = {
      title: 'Apply file changes',
      kind: 'edit',
      status: 'completed',
      content: [{ type: 'diff', path: 'src/main.ts', newText: "export const x = 1\n", oldText: "export const x = 0\n" }],
      locations: [{ path: 'src/main.ts', line: 1 }],
    }
    return (
      <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
        <SessionUpdateToolCall {...call} />
      </ScrollView>
    )
  },
}
