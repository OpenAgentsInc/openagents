import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { SessionUpdateAvailableCommandsUpdate } from '@/components/acp'
import type { AvailableCommand } from '@/types/acp'
import { Colors } from '@/constants/theme'

const meta = {
  title: 'ACP/AvailableCommands',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => {
    const cmds: ReadonlyArray<AvailableCommand> = [
      { name: 'create_plan', description: 'Draft a plan of action' },
      { name: 'search_repo', description: 'Search across repository files' },
    ]
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
        <SessionUpdateAvailableCommandsUpdate available_commands={cmds} />
        <View style={{ marginTop: 12 }}>
          <Text style={{ color: Colors.secondary }}>Component: SessionUpdateAvailableCommandsUpdate</Text>
          <Text style={{ color: Colors.foreground }}>Shows the current set of available commands (name + description).</Text>
          <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
          <Text style={{ color: Colors.foreground }}>{'available_commands: ReadonlyArray<AvailableCommand>'}</Text>
        </View>
      </View>
    )
  },
}
