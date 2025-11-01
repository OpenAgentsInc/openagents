import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { ToolCallContentTerminal } from '@/components/acp/ToolCallContentTerminal'
import { Colors } from '@/constants/theme'

const meta = { title: 'ACP/ToolCallContent/Terminal' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <ToolCallContentTerminal terminalId={'example/terminal-1'} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: ToolCallContentTerminal</Text>
        <Text style={{ color: Colors.foreground }}>Displays an embedded terminal placeholder for streaming output.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'terminalId: string — unique id for terminal stream'}</Text>
      </View>
    </View>
  ),
}

