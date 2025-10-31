import React from 'react'
import { Pressable, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useRouter } from 'expo-router'
import { useTinyvex } from '@/providers/tinyvex'
import type { ThreadSummaryTs as ThreadRow } from '../../types/bridge/ThreadSummaryTs'
import { Ionicons } from '@expo/vector-icons'

export function ThreadListItemBase({
  title,
  meta,
  timestamp,
  count,
  onPress,
  onLongPress,
}: {
  title: string
  meta?: React.ReactNode
  timestamp?: number | null
  count?: number | null
  onPress?: () => void
  onLongPress?: () => void
}) {
  const ts = typeof timestamp === 'number' && timestamp > 0 ? formatRelative(new Date(timestamp).getTime()) : ''
  return (
    <Pressable onPress={onPress} onLongPress={onLongPress} delayLongPress={300} accessibilityRole="button" style={{ paddingVertical: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center' }}>
        <View style={{ flex: 1, paddingRight: 8 }}>
          <Text numberOfLines={1} style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{title || 'Thread'}</Text>
          {meta ? (
            <View style={{ marginTop: 2 }}>{meta}</View>
          ) : null}
          {!!ts && (
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, marginTop: 2 }}>
              <Ionicons name="time-outline" size={12} color={Colors.tertiary} />
              <Text numberOfLines={1} style={{ color: Colors.tertiary, fontFamily: Typography.primary, fontSize: 12 }}>{ts}</Text>
            </View>
          )}
        </View>
        {typeof count === 'number' ? (
          <Text style={{ color: Colors.quaternary, fontFamily: Typography.bold, fontSize: 12 }}>{count}</Text>
        ) : null}
      </View>
    </Pressable>
  )
}

export function DrawerThreadItem({ row, onPress, onLongPress }: { row: ThreadRow; onPress?: () => void; onLongPress?: () => void }) {
  const router = useRouter()
  const threadId = String(row?.id || '')
  // Hide ephemeral/internal threads
  if (/^ephemeral_/i.test(threadId)) return null
  const { messagesByThread } = useTinyvex()
  const msgs = Array.isArray(messagesByThread[threadId]) ? messagesByThread[threadId] : []
  // Prefer last message timestamp if we have a tail; fall back to row.updatedAt
  const updatedAt = React.useMemo(() => {
    const arr = msgs && msgs.length > 0 ? msgs : []
    if (arr.length > 0) {
      const last = arr[arr.length - 1]
      const ts = Number(last?.ts || 0)
      if (!isNaN(ts) && ts > 0) return ts
    }
    const ts = typeof row.last_message_ts === 'number' ? row.last_message_ts : undefined
    return ts ?? Number(row.updated_at || 0)
  }, [msgs, row])
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
  const providerBadge = (() => {
    if (source === 'claude_code') {
      return (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          <Ionicons name="flash-outline" size={12} color={Colors.quaternary} />
          <Text style={{ color: Colors.quaternary, fontFamily: Typography.primary, fontSize: 12 }}>Claude Code</Text>
        </View>
      )
    }
    if (source === 'codex') {
      return (
        <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
          <Ionicons name="code-slash" size={12} color={Colors.quaternary} />
          <Text style={{ color: Colors.quaternary, fontFamily: Typography.primary, fontSize: 12 }}>Codex</Text>
        </View>
      )
    }
    return null
  })()

  const meta = (() => {
    const tsText = updatedAt ? formatRelative(updatedAt) : ''
    return (
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        {!!tsText && (
          <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
            <Ionicons name="time-outline" size={12} color={Colors.quaternary} />
            <Text numberOfLines={1} style={{ color: Colors.quaternary, fontFamily: Typography.primary, fontSize: 12 }}>{tsText}</Text>
          </View>
        )}
        {providerBadge ? (
          <>
            {!!tsText && (
              <Text style={{ color: Colors.quaternary, fontFamily: Typography.primary, fontSize: 12 }}>•</Text>
            )}
            {providerBadge}
          </>
        ) : null}
      </View>
    )
  })()
  return (
    <ThreadListItemBase title={lastSnippet || (row?.title as any) || 'Thread'} meta={meta as any} timestamp={updatedAt} count={typeof count === 'number' ? count : undefined} onPress={open} onLongPress={onLongPress} />
  )
}

// normalizeTs removed; rely on row.last_message_ts or row.updated_at

function formatRelative(ts: number): string {
  const now = Date.now()
  const diff = Math.max(0, now - ts)
  const sec = Math.floor(diff / 1000)
  if (sec < 5) return 'just now'
  if (sec < 60) return `${sec} seconds ago`
  const min = Math.floor(sec / 60)
  if (min < 60) return `${min} minute${min === 1 ? '' : 's'} ago`
  const hr = Math.floor(min / 60)
  if (hr < 24) return `${hr} hour${hr === 1 ? '' : 's'} ago`
  const day = Math.floor(hr / 24)
  if (day < 7) return `${day} day${day === 1 ? '' : 's'} ago`
  const week = Math.floor(day / 7)
  if (week < 5) return `${week} week${week === 1 ? '' : 's'} ago`
  const month = Math.floor(day / 30)
  if (month < 12) return `${month} month${month === 1 ? '' : 's'} ago`
  const year = Math.floor(day / 365)
  return `${year} year${year === 1 ? '' : 's'} ago`
}
