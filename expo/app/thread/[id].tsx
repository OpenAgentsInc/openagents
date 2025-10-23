import React from 'react'
import { useLocalSearchParams, useRouter, Stack } from 'expo-router'
import { ActivityIndicator, FlatList, Pressable, Text, View, KeyboardAvoidingView, Platform } from 'react-native'
import { useThreads } from '@/lib/threads-store'
import { useBridge } from '@/providers/ws'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderStore, useHeaderTitle } from '@/lib/header-store'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'
import { CommandExecutionCard } from '@/components/jsonl/CommandExecutionCard'
import { ReasoningHeadline } from '@/components/jsonl/ReasoningHeadline'
import { Composer } from '@/components/composer'
import { useSafeAreaInsets } from 'react-native-safe-area-context'
import { Ionicons } from '@expo/vector-icons'
import { useProjects } from '@/providers/projects'

export default function ThreadHistoryView() {
  const { id, path } = useLocalSearchParams<{ id: string; path?: string }>()
  const { requestThread } = useBridge()
  const router = useRouter()
  const loadThread = useThreads((s) => s.loadThread)
  const thread = useThreads((s) => (id ? s.thread[id] : undefined))
  const loadingMap = useThreads((s) => s.loadingThread)
  const loading = Boolean(id && loadingMap[id])
  const listRef = React.useRef<FlatList<any> | null>(null)
  const [atBottom, setAtBottom] = React.useState(true)
  const initialScrolledRef = React.useRef(false)
  const prevCountRef = React.useRef(0)
  const prevPartialRef = React.useRef<boolean | undefined>(undefined)
  const headerHeight = useHeaderStore((s) => s.height)
  const insets = useSafeAreaInsets()
  const composerInputRef = React.useRef<any>(null)
  const { activeProject, sendForProject } = useProjects()

  React.useEffect(() => { if (id) loadThread((id2, p) => requestThread(id2, p), id, typeof path === 'string' ? path : undefined).catch(()=>{}) }, [id, path, requestThread, loadThread])
  useHeaderTitle(thread?.title || 'Thread')

  const items = React.useMemo(() => {
    const base = (thread?.items || []).slice()
    if (base.length === 0) return base
    // Drop leading instruction dump or preface blocks
    const first = base[0]
    const text = String(first?.text ?? '')
    const isUserish = first?.role === 'user' || /^\s*>/.test(text) || /^You:\s*/i.test(text)
    const looksLikeInstructions = /<user_instructions>/i.test(text) || /#\s*Repository\s+Guidelines/i.test(text)
    const looksLikeEnvPreface = /<environment_context>/i.test(text) || /\bYou are a coding agent running\b/i.test(text) || /\bEnvironment:\b/.test(text)
    let arr = base
    if (isUserish && (looksLikeInstructions || looksLikeEnvPreface)) arr = base.slice(1)
    // Deduplicate consecutive identical entries after basic sanitization
    const out: typeof arr = []
    for (const it of arr) {
      const prev = out[out.length - 1]
      if (
        prev &&
        prev.kind === it.kind &&
        ((prev.role ?? '') === (it.role ?? '')) &&
        String(prev.text).trim() === String(it.text).trim()
      ) {
        continue
      }
      out.push(it)
    }
    return out
  }, [thread?.items])

  // For command entries, only keep the last entry per command (like live feed)
  const lastCmdIndex = React.useMemo(() => {
    const map = new Map<string, number>()
    items.forEach((it, idx) => {
      if (it.kind !== 'cmd') return
      try {
        const obj = JSON.parse(it.text)
        const cmd = String(obj.command ?? '')
        if (cmd) map.set(cmd, idx)
      } catch {}
    })
    return map
  }, [items])

  // Auto-scroll behaviors
  React.useEffect(() => {
    // Initial snap to bottom when any content is present
    if (!initialScrolledRef.current && items.length > 0) {
      const t = setTimeout(() => {
        try { listRef.current?.scrollToEnd({ animated: false }) } catch {}
        initialScrolledRef.current = true
        prevCountRef.current = items.length
        setAtBottom(true)
      }, 0)
      return () => clearTimeout(t)
    }
    // If items grew (full hydrate), keep pinned to bottom
    if (items.length > prevCountRef.current) {
      const t = setTimeout(() => { try { listRef.current?.scrollToEnd({ animated: false }) } catch {} }, 0)
      prevCountRef.current = items.length
      return () => clearTimeout(t)
    }
  }, [items.length])

  React.useEffect(() => {
    // When we switch from partial preview to full content, ensure bottom
    const was = prevPartialRef.current
    const now = thread?.partial
    if (was === true && now !== true) {
      const t = setTimeout(() => { try { listRef.current?.scrollToEnd({ animated: false }) } catch {} }, 0)
      prevPartialRef.current = now
      return () => clearTimeout(t)
    }
    prevPartialRef.current = now
  }, [thread?.partial])

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Stack.Screen options={{ title: '', headerBackTitle: '' }} />
      {loading && items.length === 0 ? (
        <View style={{ flex: 1, alignItems: 'center', justifyContent: 'center' }}>
          <ActivityIndicator color={Colors.secondary} />
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginTop: 8 }}>Loading…</Text>
        </View>
      ) : (
        <FlatList
          ref={listRef}
          data={items}
          keyExtractor={(_, i) => String(i)}
          contentContainerStyle={{ padding: 12, gap: 10, paddingBottom: 80 }}
          ListHeaderComponent={(() => {
            // Show top loader when we have content but are still hydrating the rest
            if ((thread?.partial || loading) && items.length > 0) {
              return (
                <View style={{ alignItems: 'center', paddingVertical: 8 }}>
                  <ActivityIndicator color={Colors.secondary} />
                  <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Loading earlier messages…</Text>
                </View>
              )
            }
            return null
          })()}
          onScroll={(e) => {
            try {
              const { contentOffset, layoutMeasurement, contentSize } = e.nativeEvent
              const atBot = contentOffset.y + layoutMeasurement.height >= contentSize.height - 24
              setAtBottom(atBot)
            } catch {}
          }}
          scrollEventThrottle={32}
          renderItem={({ item, index }) => {
            const isLastCmd = item.kind !== 'cmd' ? undefined : (() => {
              try {
                const obj = JSON.parse(item.text)
                const cmd = String(obj.command ?? '')
                if (!cmd) return true
                return lastCmdIndex.get(cmd) === index
              } catch { return true }
            })()
            return <Row it={item} isLastCmd={isLastCmd} />
          }}
        />
      )}
      {!atBottom && (
        <Pressable
          onPress={() => { try { listRef.current?.scrollToEnd({ animated: true }) } catch {} }}
          accessibilityRole="button"
          style={{ position: 'absolute', right: 16, bottom: 88, width: 36, height: 36, borderRadius: 18, backgroundColor: Colors.foreground, alignItems: 'center', justifyContent: 'center' }}
        >
          <Ionicons name="chevron-down" size={20} color={Colors.black} />
        </Pressable>
      )}
      <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={headerHeight + 4}>
        <View style={{ paddingBottom: Math.max(insets.bottom, 8), paddingHorizontal: 8 }}>
          <Composer
            onSend={(txt) => {
              if (!id) return
              // Prefer resume_id from parsed thread; fall back to history filename
              const resumeId = thread?.resume_id || (typeof id === 'string' ? id : null)
              try { sendForProject(activeProject, txt, resumeId) } catch {}
              router.replace('/thread?focus=1&resuming=1')
            }}
            connected={true}
            isRunning={false}
            onQueue={() => {}}
            onInterrupt={() => {}}
            queuedMessages={[]}
            prefill={null}
            onDraftChange={() => {}}
            inputRef={composerInputRef}
          />
        </View>
      </KeyboardAvoidingView>
    </View>
  )
}

function sanitizeUserText(s: string): string {
  let text = String(s || '')
  // Remove <environment_context>...</environment_context> blocks if present
  text = text.replace(/<environment_context>[\s\S]*?<\/environment_context>/gi, '').trim()
  // If there is a clear preface separated by blank line(s), prefer the trailing segment
  const parts = text.split(/\n\n+/)
  if (parts.length > 1) {
    text = parts[parts.length - 1].trim()
  }
  return text
}

function Row({ it, isLastCmd }: { it: { ts: number; kind: 'message'|'reason'|'cmd'; role?: 'assistant'|'user'; text: string }, isLastCmd?: boolean }) {
  if (it.kind === 'message' && it.role === 'assistant') {
    return <MarkdownBlock markdown={it.text} />
  }
  if (it.kind === 'message' && it.role === 'user') {
    const text = sanitizeUserText(it.text)
    return (
      <Text style={{ fontSize: 12, lineHeight: 16, color: Colors.foreground, fontFamily: Typography.primary, marginTop: 6, marginBottom: 8 }}>
        {`> ${text}`}
      </Text>
    )
  }
  if (it.kind === 'reason') {
    return <ReasoningHeadline text={it.text} />
  }
  if (it.kind === 'cmd') {
    try {
      const obj = JSON.parse(it.text)
      if (!isLastCmd) return null
      const label = String(obj.command ?? '').trim()
      return (
        <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, paddingVertical: 10, paddingHorizontal: 12, flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between' }}>
          <Text numberOfLines={1} style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 12 }}>{label}</Text>
          <Ionicons name="chevron-forward" size={16} color={Colors.secondary} />
        </View>
      )
    } catch {
      return null
    }
  }
  return <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{it.text}</Text>
}
