import React from 'react'
import { ScrollView, View, Text, Pressable, ActivityIndicator, KeyboardAvoidingView, Platform } from 'react-native'
import { useLocalSearchParams, router } from 'expo-router'
import { useQuery, useMutation } from 'convex/react'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { Composer } from '@/components/composer'
import { useHeaderStore } from '@/lib/header-store'

export default function ConvexThreadDetail() {
  const { id } = useLocalSearchParams<{ id: string }>()

  // Load thread (for title) — tolerate null while loading
  const thread = (useQuery as any)('threads:byId', { id }) as any
  useHeaderTitle(thread?.title ? String(thread.title) : 'Thread')

  // Live messages subscription for this thread: use the thread.threadId, not the Convex doc _id
  const messages = (useQuery as any)('messages:forThread', { threadId: thread?.threadId || '' }) as any[] | undefined | null

  const createDemo = (useMutation as any)('messages:createDemo') as (args: { threadId: string }) => Promise<any>
  const createMessage = (useMutation as any)('messages:create') as (args: { threadId: string; role: string; text: string }) => Promise<any>
  const headerHeight = useHeaderStore((s) => s.height)
  const [busy, setBusy] = React.useState(false)
  const onCreateDemo = async () => {
    if (busy) return
    setBusy(true)
    try { await createDemo({ threadId: id! }) } catch {} finally { setBusy(false) }
  }

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12, paddingBottom: 88 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 18 }}>Messages</Text>
        <Pressable onPress={onCreateDemo} disabled={busy} style={{ opacity: busy ? 0.6 : 1, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 10, paddingVertical: 6 }}>
          <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>Add demo message</Text>
        </Pressable>
      </View>

      {messages === undefined ? (
        <ActivityIndicator color={Colors.secondary} />
      ) : messages === null ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No Convex deployment or query missing (messages:forThread).</Text>
      ) : messages.length === 0 ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No messages yet.</Text>
      ) : (
        <View style={{ gap: 10 }}>
          {messages.map((m: any) => (
            <View key={m._id || `${m.threadId}-${m.ts}`} style={{ borderWidth: 1, borderColor: Colors.border, padding: 10 }}>
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{new Date(m.ts).toLocaleString()} · {m.role}</Text>
              <View style={{ height: 6 }} />
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{m.text}</Text>
            </View>
          ))}
        </View>
      )}
      </ScrollView>
      <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={headerHeight + 4}>
        <View style={{ paddingBottom: 8, paddingHorizontal: 8 }}>
          <Composer
            onSend={async (txt) => {
              const base = String(txt || '').trim()
              if (!base || !thread?.threadId) return
              try { await createMessage({ threadId: thread.threadId, role: 'user', text: base }) } catch {}
            }}
            connected={true}
            isRunning={false}
            onQueue={() => {}}
            onInterrupt={() => {}}
            queuedMessages={[]}
            prefill={null}
            onDraftChange={() => {}}
            inputRef={undefined as any}
          />
        </View>
      </KeyboardAvoidingView>
    </View>
  )
}
