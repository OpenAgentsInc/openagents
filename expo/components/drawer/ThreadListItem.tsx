import React from 'react'
import { Pressable, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useRouter } from 'expo-router'
import { useTinyvex } from '@/providers/tinyvex'
import { Ionicons } from '@expo/vector-icons'

export function ThreadListItemBase({
  title,
  meta,
  timestamp,
  count,
  onPress,
}: {
  title: string
  meta?: React.ReactNode
  timestamp?: number | null
  count?: number | null
  onPress?: () => void
}) {
  const ts = typeof timestamp === 'number' && timestamp > 0 ? new Date(timestamp).toLocaleString() : ''
  return (
    <Pressable onPress={onPress} accessibilityRole="button" style={{ paddingVertical: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center' }}>
        <View style={{ flex: 1, paddingRight: 8 }}>
          <Text numberOfLines={1} style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{title || 'Thread'}</Text>
          {meta ? (
            <View style={{ marginTop: 2 }}>{meta}</View>
          ) : null}
          {!!ts && (
            <Text numberOfLines={1} style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{ts}</Text>
          )}
        </View>
        {typeof count === 'number' ? (
          <Text style={{ color: Colors.quaternary, fontFamily: Typography.bold, fontSize: 12 }}>{count}</Text>
        ) : null}
      </View>
    </Pressable>
  )
}

export function DrawerThreadItem({ row, onPress }: { row: any; onPress?: () => void }) {
  const router = useRouter()
  const threadId = String(row?.threadId || row?.thread_id || row?._id || row?.id || '')
  const { messagesByThread } = useTinyvex()
  const msgs = Array.isArray(messagesByThread[threadId]) ? messagesByThread[threadId] : []
  const updatedAt = (row?.updatedAt ?? row?.createdAt ?? 0) as number
  const count = React.useMemo(() => {
    const mc: any = (row as any)?.messageCount
    if (typeof mc === 'number') return mc
    return msgs.length > 0 ? msgs.length : undefined
  }, [row, msgs.length])
  // Use the last message text (assistant or user) as the snippet; fall back to row.title or "Thread"
  const lastSnippet = React.useMemo(() => {
    const arr = msgs && msgs.length > 0 ? msgs : []
    let last = arr[arr.length - 1]
    try {
      if (arr.length > 1) {
        const byTs = [...arr]
        byTs.sort((a: any, b: any) => (Number(a?.ts || 0) - Number(b?.ts || 0)))
        last = byTs[byTs.length - 1]
      }
    } catch {}
    const raw = String((last && (last.text || '')) || '')
    const cleaned = raw
      .replace(/```[\s\S]*?```/g, '')
      .replace(/`([^`]*)`/g, '$1')
      .replace(/^#+\s*/gm, '')
      .replace(/\*\*([^*]+)\*\*/g, '$1')
      .replace(/\*([^*]+)\*/g, '$1')
      .replace(/\[(.*?)\]\([^)]*\)/g, '$1')
      .replace(/\s+/g, ' ')
      .trim()
    const base = cleaned || (row?.title ? String(row.title) : '') || 'Thread'
    const maxLen = 48
    return base.length > maxLen ? `${base.slice(0, maxLen - 1)}…` : base
  }, [msgs, row?.title])
  const open = () => {
    if (onPress) { try { onPress() } catch {} ; return }
    if (!threadId) return
    try { router.push(`/thread/${encodeURIComponent(threadId)}`) } catch {}
  }
  // Filter out threads that have zero primary chat messages when count is known
  if (typeof count === 'number' && count <= 0) return null
  // Provider/source indicator (Codex vs Claude Code)
  const source = String((row?.source || '') as any).toLowerCase()
  const meta = (() => {
    if (source === 'claude_code') {
      return (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          <Ionicons name="flash-outline" size={12} color={Colors.secondary} />
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Claude Code</Text>
        </View>
      )
    }
    if (source === 'codex') {
      return (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          <Ionicons name="code-slash" size={12} color={Colors.secondary} />
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Codex</Text>
        </View>
      )
    }
    return null
  })()
  return (
    <ThreadListItemBase title={lastSnippet || row?.title || 'Thread'} meta={meta as any} timestamp={updatedAt} count={typeof count === 'number' ? count : undefined} onPress={open} />
  )
}
