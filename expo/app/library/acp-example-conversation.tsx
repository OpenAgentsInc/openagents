import React from 'react'
import { ScrollView, View, Pressable } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { useRouter } from 'expo-router'
import { SessionUpdateAgentMessageChunk, SessionUpdateUserMessageChunk, SessionUpdateAgentThoughtChunk, SessionUpdatePlan, SessionUpdateToolCall, SessionUpdateAvailableCommandsUpdate, SessionUpdateCurrentModeUpdate } from '@/components/acp'
import { exampleItems } from './acp-example-data'

export default function ACPExampleConversationScreen() {
  useHeaderTitle('ACP Example Conversation')
  const router = useRouter()
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12, paddingBottom: 32 }}>
      {exampleItems.map((it) => (
        <Pressable key={it.id} onPress={() => router.push(`/library/acp-example-conversation/${encodeURIComponent(String(it.id))}` as any)} accessibilityRole="button" style={{ paddingVertical: 2 }}>
          {it.type === 'user_message' ? (
            <SessionUpdateUserMessageChunk content={it.content as any} />
          ) : it.type === 'agent_message' ? (
            <SessionUpdateAgentMessageChunk content={it.content as any} />
          ) : it.type === 'agent_thought' ? (
            <SessionUpdateAgentThoughtChunk content={it.content as any} />
          ) : it.type === 'current_mode_update' ? (
            <SessionUpdateCurrentModeUpdate currentModeId={String((it as any).currentModeId)} />
          ) : it.type === 'available_commands_update' ? (
            <SessionUpdateAvailableCommandsUpdate available_commands={(it as any).available_commands as any} />
          ) : it.type === 'plan' ? (
            <SessionUpdatePlan entries={(it as any).entries as any} />
          ) : it.type === 'tool_call' ? (
            <SessionUpdateToolCall {...(it as any).props} />
          ) : (
            <View />
          )}
        </Pressable>
      ))}
    </ScrollView>
  )
}
