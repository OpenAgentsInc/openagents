import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, ScrollView, Text } from 'react-native'
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
        <View style={{ marginTop: 12 }}>
          <Text style={{ color: Colors.secondary }}>Component: SessionUpdateToolCall</Text>
          <Text style={{ color: Colors.foreground }}>Renders a tool call row with header, content items, and optional locations.</Text>
          <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
          <Text style={{ color: Colors.foreground }}>{'title: string — display title'}</Text>
          <Text style={{ color: Colors.foreground }}>{'kind: ToolKind — execute | edit | search | read | …'}</Text>
          <Text style={{ color: Colors.foreground }}>{'status: ToolCallStatus — pending | in_progress | completed | failed'}</Text>
          <Text style={{ color: Colors.foreground }}>{'content: ToolCallContent[] — content | diff | terminal'}</Text>
          <Text style={{ color: Colors.foreground }}>{'locations?: ToolCallLocation[] — optional file references'}</Text>
        </View>
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
        <View style={{ marginTop: 12 }}>
          <Text style={{ color: Colors.secondary }}>Component: SessionUpdateToolCall</Text>
          <Text style={{ color: Colors.foreground }}>Example showing a diff content block and a location list.</Text>
          <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props — see above</Text>
        </View>
      </ScrollView>
    )
  },
}
