import React from 'react'
import { Pressable, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useRouter } from 'expo-router'

export function ThreadListItemBase({
  title,
  timestamp,
  count,
  onPress,
}: {
  title: string
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
  const updatedAt = (row?.updatedAt ?? row?.createdAt ?? 0) as number
  const count = typeof (row as any)?.messageCount === 'number' ? (row as any).messageCount as number : undefined
  // No Convex: attempt to use provided snippet if present; otherwise fallback to title
  const recent: any[] | undefined | null = undefined
  const lastSnippet = React.useMemo(() => {
    const arr: any[] = Array.isArray(recent) ? recent : []
    // Consider only chat messages
    const msgs = arr.filter((m) => (m?.kind || (m?.role ? 'message' : '')) === 'message')
    if (msgs.length === 0) return row?.title ? String(row.title) : 'New Thread'
    // Pick the one with the largest timestamp if available
    let last = msgs[msgs.length - 1]
    try {
      if (msgs.length > 1) {
        const byTs = [...msgs]
        byTs.sort((a, b) => (Number(a?.ts || 0) - Number(b?.ts || 0)))
        last = byTs[byTs.length - 1]
      }
    } catch {}
    const raw = String(last?.text || '')
    // Basic markdown cleanup and truncation to keep it concise in the drawer
    const cleaned = raw
      .replace(/```[\s\S]*?```/g, '') // remove fenced code blocks
      .replace(/`([^`]*)`/g, '$1') // inline code
      .replace(/^#+\s*/gm, '') // headings
      .replace(/\*\*([^*]+)\*\*/g, '$1') // bold
      .replace(/\*([^*]+)\*/g, '$1') // emphasis
      .replace(/\[(.*?)\]\([^)]*\)/g, '$1') // links [text](url)
      .replace(/\s+/g, ' ') // collapse whitespace
      .trim()
    const maxLen = 48
    return cleaned.length > maxLen ? `${cleaned.slice(0, maxLen - 1)}…` : (cleaned || 'New Thread')
  }, [recent, row?.title])
  const open = () => {
    if (onPress) { try { onPress() } catch {} ; return }
    if (!threadId) return
    try { router.push(`/thread/${encodeURIComponent(threadId)}`) } catch {}
  }
  // Filter out threads that have zero primary chat messages when count is known
  if (typeof count === 'number' && count <= 0) return null
  return (
    <ThreadListItemBase title={lastSnippet || row?.title || 'Thread'} timestamp={updatedAt} count={typeof count === 'number' ? count : undefined} onPress={open} />
  )
}
