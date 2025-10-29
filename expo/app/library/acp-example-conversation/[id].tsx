import React from 'react'
import { useLocalSearchParams } from 'expo-router'
import { ScrollView, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { SessionUpdateAgentMessageChunk, SessionUpdateUserMessageChunk, SessionUpdateAgentThoughtChunk, SessionUpdatePlan, SessionUpdateToolCall, SessionUpdateAvailableCommandsUpdate, SessionUpdateCurrentModeUpdate } from '@/components/acp'
import { findExampleItem } from '@/lib/acp-example-data'

export default function ACPExampleItemDetail() {
  const { id } = useLocalSearchParams<{ id: string }>()
  const item = findExampleItem(String(id || ''))
  const title = item ? `Example · ${item.type}` : 'Example Item'
  useHeaderTitle(title)

  const renderComponent = () => {
    if (!item) return null
    if (item.type === 'user_message') return <SessionUpdateUserMessageChunk content={(item as any).content} />
    if (item.type === 'agent_message') return <SessionUpdateAgentMessageChunk content={(item as any).content} />
    if (item.type === 'agent_thought') return <SessionUpdateAgentThoughtChunk content={(item as any).content} />
    if (item.type === 'plan') return <SessionUpdatePlan entries={(item as any).entries} />
    if (item.type === 'available_commands_update') return <SessionUpdateAvailableCommandsUpdate available_commands={(item as any).available_commands} />
    if (item.type === 'current_mode_update') return <SessionUpdateCurrentModeUpdate currentModeId={String((item as any).currentModeId)} />
    if (item.type === 'tool_call') return <SessionUpdateToolCall {...(item as any).props} />
    return null
  }

  const raw = item ? JSON.stringify(item, null, 2) : ''

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 14 }}>
      {!item ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Not found.</Text>
      ) : (
        <View style={{ gap: 12 }}>
          <View>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, marginBottom: 6 }}>Rendered</Text>
            {renderComponent()}
          </View>

          <View>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, marginBottom: 6 }}>Metadata</Text>
            <Text selectable style={{ color: Colors.foreground, fontFamily: Typography.primary }}>id: {String((item as any).id)}</Text>
            <Text selectable style={{ color: Colors.foreground, fontFamily: Typography.primary }}>type: {String((item as any).type)}</Text>
            {item.type === 'tool_call' ? (
              <Text selectable style={{ color: Colors.foreground, fontFamily: Typography.primary }}>title: {String(((item as any).props?.title) || '')}</Text>
            ) : null}
          </View>

          <View>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, marginBottom: 6 }}>Raw JSON</Text>
            <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.black, padding: 10 }}>
              <Text selectable style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, lineHeight: 16 }}>{raw}</Text>
            </View>
          </View>
        </View>
      )}
    </ScrollView>
  )
}
