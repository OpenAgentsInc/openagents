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
      {exampleItems.map((it) => (
        <View key={it.id}>
          {it.type === 'user_message' ? (
            <SessionUpdateUserMessageChunk content={it.content as any} />
          ) : null}
          {it.type === 'agent_message' ? (
            <SessionUpdateAgentMessageChunk content={it.content as any} />
          ) : null}
          {it.type === 'agent_thought' ? (
            <SessionUpdateAgentThoughtChunk content={it.content as any} />
          ) : null}
          {it.type === 'current_mode_update' ? (
            <SessionUpdateCurrentModeUpdate currentModeId={String(it.currentModeId)} />
          ) : null}
          {it.type === 'available_commands_update' ? (
            <SessionUpdateAvailableCommandsUpdate available_commands={(it as any).available_commands} />
          ) : null}
          {it.type === 'plan' ? (
            <SessionUpdatePlan entries={(it as any).entries} />
          ) : null}
          {it.type === 'tool_call' ? (
            <SessionUpdateToolCall {...(it as any).props} />
          ) : null}
        </View>
      ))}
    </ScrollView>
  ),
}

