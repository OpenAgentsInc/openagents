import React from 'react'
import { useLocalSearchParams, router } from 'expo-router'
import { ScrollView, Text, View, KeyboardAvoidingView, Platform, Pressable } from 'react-native'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { useTinyvex } from '@/providers/tinyvex'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { Composer } from '@/components/composer'
import { useBridge } from '@/providers/ws'
import { useHeaderTitle, useHeaderStore } from '@/lib/header-store'
import { useAcp, type SessionNotificationWithTs } from '@/providers/acp'
import { useSettings } from '@/lib/settings-store'
import { useThreadProviders } from '@/lib/thread-provider-store'
import { SessionUpdateAgentMessageChunk } from '@/components/acp/SessionUpdateAgentMessageChunk'
import { SessionUpdateAgentThoughtChunk } from '@/components/acp/SessionUpdateAgentThoughtChunk'
import { SessionUpdateUserMessageChunk } from '@/components/acp/SessionUpdateUserMessageChunk'
import { SessionUpdatePlan } from '@/components/acp/SessionUpdatePlan'
import { SessionUpdateToolCall } from '@/components/acp/SessionUpdateToolCall'
import * as Haptics from 'expo-haptics'

export default function ThreadScreen() {
  const { id } = useLocalSearchParams<{ id: string }>()
  const initialId = String(id || '')
  const [threadId, setThreadId] = React.useState<string>('')
  React.useEffect(() => {
    if (initialId === 'new' || initialId === '') {
      const gen = `t-${Date.now()}`
      setThreadId(gen)
      try { router.replace(`/thread/${encodeURIComponent(gen)}` as any) } catch {}
    } else {
      setThreadId(initialId)
    }
    // Only re-run if the URL param changes
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initialId])
  const { eventsForThread } = useAcp()
  const { messagesByThread, subscribeMessages, queryMessages, queryToolCalls, toolCallsByThread } = useTinyvex() as any
  const { send, connected } = useBridge()
  const agentProvider = useSettings((s) => s.agentProvider)
  const setAgentProvider = useSettings((s) => s.setAgentProvider)
  const threadProviders = useThreadProviders()
  // Title for thread screen
  useHeaderTitle('New Thread')
  const acpUpdates = React.useMemo(() => eventsForThread(threadId), [eventsForThread, threadId])
  const tvxMessages = React.useMemo(() => messagesByThread[threadId] || [], [messagesByThread, threadId])

  // Ensure we are subscribed to Tinyvex messages for this thread and have a recent snapshot
  React.useEffect(() => {
    if (!threadId) return
    try { subscribeMessages(threadId) } catch {}
    try { queryMessages(threadId, 50) } catch {}
    try { queryToolCalls?.(threadId, 50) } catch {}
  }, [threadId, subscribeMessages, queryMessages])
  // When navigating into a thread, if we have a recorded provider for it, switch the active agent accordingly
  React.useEffect(() => {
    if (!threadId) return
    try {
      const p = threadProviders.getProvider(threadId)
      if (p) {
        if (p !== agentProvider) setAgentProvider(p)
      } else {
        // Default to Codex for legacy threads with no mapping
        if (agentProvider !== 'codex') setAgentProvider('codex')
      }
    } catch {}
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [threadId])
  const onSend = React.useCallback((text: string) => {
    if (!threadId) return
    const payload: any = { control: 'run.submit', threadDocId: threadId, text, resumeId: 'last' as const }
    if (agentProvider === 'claude_code') payload.provider = 'claude_code'
    try { send(JSON.stringify(payload)) } catch {}
    // Remember provider for this thread so future resumes use the correct agent
    try { threadProviders.setProvider(threadId, agentProvider) } catch {}
  }, [threadId, send, agentProvider])
  const insets = useSafeAreaInsets()
  const headerHeight = useHeaderStore((s) => s.height)
  const keyboardOffset = Platform.OS === 'ios' ? headerHeight : 0
  return (
    <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={keyboardOffset} style={{ flex: 1, backgroundColor: Colors.background }}>
      <View key={`thread-${threadId}`} style={{ flex: 1 }}>
      <ScrollView ref={(r) => { try { (globalThis as any).__threadScroll = r } catch {} }} style={{ flex: 1 }} keyboardShouldPersistTaps='handled' contentContainerStyle={{ padding: 16, gap: 10, paddingBottom: 80 }}
        onContentSizeChange={() => { try { (globalThis as any).__threadScroll?.scrollToEnd?.({ animated: false }) } catch {} }}
      >
        {/* Provider selector: show only before any messages exist */}
        {acpUpdates.length === 0 && tvxMessages.length === 0 && (
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8, marginBottom: 6 }}>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, marginRight: 8 }}>Provider</Text>
            <Pressable
              onPress={async () => {
                try {
                  if (agentProvider !== 'codex') {
                    if (process.env.EXPO_OS === 'ios') {
                      await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light)
                    } else {
                      await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light)
                    }
                  }
                } catch {}
                try { setAgentProvider('codex') } catch {}
              }}
              accessibilityRole='button'
              style={{ paddingVertical: 6, paddingHorizontal: 10, borderWidth: 1, borderColor: agentProvider === 'codex' ? Colors.foreground : Colors.border, backgroundColor: Colors.card }}
            >
              <Text style={{ color: agentProvider === 'codex' ? Colors.foreground : Colors.secondary, fontFamily: Typography.bold }}>Codex</Text>
            </Pressable>
            <Pressable
              onPress={async () => {
                try {
                  if (agentProvider !== 'claude_code') {
                    if (process.env.EXPO_OS === 'ios') {
                      await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light)
                    } else {
                      await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light)
                    }
                  }
                } catch {}
                try { setAgentProvider('claude_code') } catch {}
              }}
              accessibilityRole='button'
              style={{ paddingVertical: 6, paddingHorizontal: 10, borderWidth: 1, borderColor: agentProvider === 'claude_code' ? Colors.foreground : Colors.border, backgroundColor: Colors.card }}
            >
              <Text style={{ color: agentProvider === 'claude_code' ? Colors.foreground : Colors.secondary, fontFamily: Typography.bold }}>Claude Code</Text>
            </Pressable>
          </View>
        )}
        {acpUpdates.length === 0 && tvxMessages.length === 0 && (
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No messages yet.</Text>
        )}
        {(() => {
          type RenderItem = { ts: number; key: string; render: () => React.ReactNode }
          const items: RenderItem[] = []
          const haveTinyvex = tvxMessages.length > 0

          // Tinyvex messages → timeline items
          for (let i = 0; i < tvxMessages.length; i++) {
            const m = tvxMessages[i]
            const ts = Number(m.ts || m.updated_at || m.created_at || Date.now())
            items.push({
              ts,
              key: `tvx-${m.id || i}`,
              render: () => {
                const text = String(m.text || '')
                const content = { type: 'text', text } as any
                if (m.kind === 'reason') return <SessionUpdateAgentThoughtChunk content={content} />
                if ((m.role || '').toLowerCase() === 'user') return <SessionUpdateUserMessageChunk content={content} />
                return <SessionUpdateAgentMessageChunk content={content} />
              },
            })
          }

          // Aggregate tool calls (create + updates) by id from live ACP
          type ToolLike = { title?: any; status?: any; kind?: any; content?: any; locations?: any }
          const toolById = new Map<string, { firstTs: number; props: ToolLike }>()

          const pushPlan = (n: SessionNotificationWithTs, u: any, index: number) => {
            const ts = Number((n as any).addedAt || Date.now())
            items.push({
              ts,
              key: `plan-${ts}-${index}`,
              render: () => <SessionUpdatePlan entries={u.entries || []} />,
            })
          }

          const maybeAddMessageChunk = (n: SessionNotificationWithTs, u: any, index: number) => {
            if (haveTinyvex) return // dedupe against Tinyvex history
            const ts = Number((n as any).addedAt || Date.now())
            if (u?.sessionUpdate === 'user_message_chunk') {
              items.push({ ts, key: `acp-u-${ts}-${index}`, render: () => <SessionUpdateUserMessageChunk content={u.content} /> })
            } else if (u?.sessionUpdate === 'agent_message_chunk') {
              items.push({ ts, key: `acp-a-${ts}-${index}`, render: () => <SessionUpdateAgentMessageChunk content={u.content} /> })
            } else if (u?.sessionUpdate === 'agent_thought_chunk') {
              items.push({ ts, key: `acp-t-${ts}-${index}`, render: () => <SessionUpdateAgentThoughtChunk content={u.content} /> })
            }
          }

          for (let i = 0; i < acpUpdates.length; i++) {
            const n = acpUpdates[i] as SessionNotificationWithTs
            const u: any = (n as any).update
            if (!u) continue
            if (u.sessionUpdate === 'plan') {
              pushPlan(n, u, i)
              continue
            }
            if (u.sessionUpdate === 'tool_call') {
              const id = String((u.id && (u.id as any).value) || (u.id?.toString?.() ?? '')) || `call-${i}`
              const firstTs = Number((n as any).addedAt || Date.now())
              const base: ToolLike = { title: u.title, status: u.status, kind: u.kind, content: u.content, locations: u.locations }
              const cur = toolById.get(id)
              if (!cur) toolById.set(id, { firstTs, props: { ...base } })
              else toolById.set(id, { firstTs: Math.min(cur.firstTs, firstTs), props: { ...cur.props, ...base } })
              continue
            }
            if (u.sessionUpdate === 'tool_call_update') {
              const id = String((u.id && (u.id as any).value) || (u.id?.toString?.() ?? '')) || `update-${i}`
              const ts = Number((n as any).addedAt || Date.now())
              const delta: ToolLike = {
                title: u.fields?.title ?? undefined,
                status: u.fields?.status ?? undefined,
                // Merge additional fields if present
                content: Array.isArray(u.fields?.content) ? u.fields?.content : undefined,
                locations: Array.isArray(u.fields?.locations) ? u.fields?.locations : undefined,
              }
              const prev = toolById.get(id)
              if (prev) {
                toolById.set(id, { firstTs: prev.firstTs, props: { ...prev.props, ...delta } })
              } else {
                // No create seen — synthesize a minimal card so updates render
                toolById.set(id, { firstTs: ts, props: { kind: 'other', ...delta } })
              }
              continue
            }
            // Text/Thought chunks (when no Tinyvex history to dedupe against)
            maybeAddMessageChunk(n, u, i)
          }

          // Emit tool cards as timeline items
          for (const [id, v] of toolById.entries()) {
            const props = v.props as any
            items.push({
              ts: v.firstTs,
              key: `tool-${id}`,
              render: () => <SessionUpdateToolCall {...props} />,
            })
          }

          // Hydrated tool calls from Tinyvex — add minimal cards sorted by updatedAt
          try {
            const trows: any[] = Array.isArray((toolCallsByThread as any)?.[threadId]) ? (toolCallsByThread as any)[threadId] : []
            for (const r of trows) {
              const ts = Number((r?.updated_at ?? r?.updatedAt ?? r?.created_at ?? r?.createdAt ?? Date.now()))
              const title = String(r?.title || 'Tool')
              const statusRaw = String(r?.status || '').toLowerCase()
              const status = (statusRaw.includes('complete') ? 'completed' : (statusRaw.includes('fail') ? 'failed' : (statusRaw.includes('progress') ? 'in_progress' : 'pending')))
              const kindRaw = String(r?.kind || '').toLowerCase()
              let kind: any = 'other'
              if (kindRaw.includes('execute')) kind = 'execute'
              else if (kindRaw.includes('edit')) kind = 'edit'
              else if (kindRaw.includes('search')) kind = 'search'
              else if (kindRaw.includes('read')) kind = 'read'
              else if (kindRaw.includes('delete')) kind = 'delete'
              else if (kindRaw.includes('move')) kind = 'move'
              else if (kindRaw.includes('fetch')) kind = 'fetch'
              else if (kindRaw.includes('think')) kind = 'think'
              else if (kindRaw.includes('switch')) kind = 'switch_mode'
              let locations: any[] = []
              try { const lj = r?.locations_json || r?.locations; if (lj) { const arr = typeof lj === 'string' ? JSON.parse(lj) : lj; if (Array.isArray(arr)) { locations = arr.slice(0, 8) } } } catch {}
              const props: any = { title, status, kind, content: [], locations }
              items.push({ ts, key: `tvx-tool-${String(r?.tool_call_id || r?.toolCallId || r?.id || ts)}`, render: () => <SessionUpdateToolCall {...props} /> })
            }
          } catch {}

          // Final: sort by timestamp, stable
          items.sort((a, b) => a.ts - b.ts)

          return items.map((it) => (
            <View key={it.key} style={{ paddingVertical: 4 }}>
              {it.render()}
            </View>
          ))
        })()}
      </ScrollView>
      <View style={{ paddingTop: 10, paddingHorizontal: 10, paddingBottom: Math.max(10, insets.bottom), borderTopWidth: 1, borderTopColor: Colors.border, backgroundColor: Colors.background }}>
        <Composer onSend={onSend} connected={connected} placeholder={agentProvider === 'claude_code' ? 'Ask Claude Code' : 'Ask Codex'} />
      </View>
      </View>
    </KeyboardAvoidingView>
  )
}
