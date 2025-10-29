import React from 'react'
import { View, KeyboardAvoidingView, Platform, ScrollView } from 'react-native'
import { useLocalSearchParams } from 'expo-router'
import { Colors } from '@/constants/theme'
import { useHeaderTitle, useHeaderStore } from '@/lib/header-store'
import { Composer } from '@/components/composer'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { useBridge } from '@/providers/ws'
import type { SessionUpdate } from '@/types/acp'
import { SessionUpdateAgentMessageChunk, SessionUpdateAgentThoughtChunk, SessionUpdateToolCall, SessionUpdatePlan, SessionUpdateAvailableCommandsUpdate, SessionUpdateCurrentModeUpdate } from '@/components/acp'

export default function ThreadEntry() {
  const params = useLocalSearchParams<{ new?: string; focus?: string }>()
  const insets = useSafeAreaInsets()
  const headerHeight = useHeaderStore((s) => s.height)
  const inputRef = React.useRef<any>(null)
  useHeaderTitle('New Thread')
  const ws = useBridge()
  const [acpRows, setAcpRows] = React.useState<Array<{ key: string; update: SessionUpdate }>>([])
  const toolCallIndexRef = React.useRef<Map<string, number>>(new Map())

  // Subscribe to ACP updates globally (no Convex dependency)
  React.useEffect(() => {
    const unsub = ws.addSubscriber((line) => {
      try {
        const s = String(line || '').trim()
        if (!s.startsWith('{')) return
        const obj = JSON.parse(s)
        if (obj?.type !== 'bridge.acp') return
        const update: SessionUpdate | undefined = obj?.notification?.update
        if (!update || typeof update !== 'object') return
        setAcpRows((prev) => {
          const next = [...prev]
          if ((update as any).sessionUpdate === 'tool_call') {
            const id: string | undefined = (update as any).toolCallId || (update as any).tool_call_id
            if (id) {
              const idx = toolCallIndexRef.current.get(id)
              if (typeof idx === 'number') next[idx] = { key: `tool:${id}`, update }
              else { toolCallIndexRef.current.set(id, next.length); next.push({ key: `tool:${id}`, update }) }
              return next
            }
          }
          next.push({ key: `${Date.now()}:${Math.random().toString(36).slice(2)}`, update })
          return next
        })
      } catch {}
    })
    return () => { try { unsub?.() } catch {} }
  }, [ws])

  React.useEffect(() => {
    const t = setTimeout(() => { try { inputRef.current?.focus?.() } catch {} }, 150)
    return () => clearTimeout(t)
  }, [])

  return (
    <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={headerHeight + 4} style={{ flex: 1 }}>
      <View style={{ flex: 1, backgroundColor: Colors.background }}>
        <ScrollView style={{ flex: 1 }} contentContainerStyle={{ padding: 12, gap: 10 }}>
          {acpRows.map((row) => (
            <View key={row.key} style={{ paddingVertical: 2 }}>
              <AcpUpdateRenderer update={row.update} />
            </View>
          ))}
        </ScrollView>
        <View style={{ justifyContent: 'flex-end', paddingBottom: Math.max(insets.bottom, 8), paddingHorizontal: 8 }}>
          <Composer
            onSend={async (txt) => {
              const base = String(txt || '').trim()
              if (!base) return
              const threadDocId = `ephemeral_${Date.now()}`
              try { ws.send(JSON.stringify({ control: 'echo', tag: 'composer.onSend', threadDocId, previewLen: base.length })) } catch {}
              try { ws.send(JSON.stringify({ control: 'run.submit', threadDocId, text: base })) } catch {}
            }}
            connected={true}
            isRunning={false}
            onQueue={() => {}}
            onInterrupt={() => {}}
            queuedMessages={[]}
            prefill={null}
            onDraftChange={() => {}}
            inputRef={inputRef}
          />
        </View>
      </View>
    </KeyboardAvoidingView>
  )
}

function AcpUpdateRenderer({ update }: { update: SessionUpdate }) {
  switch ((update as any).sessionUpdate) {
    case 'agent_message_chunk':
      return <SessionUpdateAgentMessageChunk content={(update as any).content} />
    case 'agent_thought_chunk':
      return <SessionUpdateAgentThoughtChunk content={(update as any).content} />
    case 'tool_call':
      return <SessionUpdateToolCall {...(update as any)} />
    case 'plan':
      return <SessionUpdatePlan entries={(update as any).entries || []} />
    case 'available_commands_update':
      return <SessionUpdateAvailableCommandsUpdate available_commands={(update as any).available_commands || []} />
    case 'current_mode_update':
      return <SessionUpdateCurrentModeUpdate currentModeId={(update as any).currentModeId || (update as any).current_mode_id || ''} />
    default:
      return null
  }
}
