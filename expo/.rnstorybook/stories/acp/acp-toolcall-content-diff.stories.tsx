import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { ToolCallContentDiff } from '@/components/acp/ToolCallContentDiff'
import { Colors } from '@/constants/theme'

const meta = { title: 'ACP/ToolCallContent/Diff' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <ToolCallContentDiff path={'src/main.ts'} oldText={'export const x = 0\n'} newText={'export const x = 1\n'} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: ToolCallContentDiff</Text>
        <Text style={{ color: Colors.foreground }}>Shows a diff-like code preview for a file path.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'path: string — file path for language hint'}</Text>
        <Text style={{ color: Colors.foreground }}>{'newText: string — resulting file contents'}</Text>
        <Text style={{ color: Colors.foreground }}>{'oldText?: string — optional previous contents'}</Text>
      </View>
    </View>
  ),
}

