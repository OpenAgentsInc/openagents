import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { exampleItems } from '@/lib/acp-example-data'
import { SessionUpdateAgentMessageChunk, SessionUpdateUserMessageChunk, SessionUpdateAgentThoughtChunk, SessionUpdatePlan, SessionUpdateToolCall, SessionUpdateAvailableCommandsUpdate, SessionUpdateCurrentModeUpdate } from '@/components/acp'

const meta = {
  title: 'ACP/ExampleConversation',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      {exampleItems.map((it) => {
        switch (it.type) {
          case 'user_message':
            return (
              <View key={it.id}>
                <SessionUpdateUserMessageChunk content={it.content} />
              </View>
            )
          case 'agent_message':
            return (
              <View key={it.id}>
                <SessionUpdateAgentMessageChunk content={it.content} />
              </View>
            )
          case 'agent_thought':
            return (
              <View key={it.id}>
                <SessionUpdateAgentThoughtChunk content={it.content} />
              </View>
            )
          case 'current_mode_update':
            return (
              <View key={it.id}>
                <SessionUpdateCurrentModeUpdate currentModeId={it.currentModeId} />
              </View>
            )
          case 'available_commands_update':
            return (
              <View key={it.id}>
                <SessionUpdateAvailableCommandsUpdate available_commands={it.available_commands} />
              </View>
            )
          case 'plan':
            return (
              <View key={it.id}>
                <SessionUpdatePlan entries={it.entries} />
              </View>
            )
          case 'tool_call':
            return (
              <View key={it.id}>
                <SessionUpdateToolCall {...it.props} />
              </View>
            )
          default:
            return null
        }
      })}
    </ScrollView>
  ),
}
