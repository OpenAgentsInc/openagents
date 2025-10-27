import React from 'react'
import { Pressable, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useRouter } from 'expo-router'
import { useQuery } from 'convex/react'

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
  const threadId = String(row?.threadId || row?._id || row?.id || '')
  const updatedAt = (row?.updatedAt ?? row?.createdAt ?? 0) as number
  const countFromRow = typeof (row as any)?.messageCount === 'number' ? (row as any).messageCount as number : undefined
  const count = countFromRow ?? ((useQuery as any)('messages:countForThread', { threadId }) as number | undefined)
  const open = () => {
    if (onPress) { try { onPress() } catch {} ; return }
    try { router.push(`/convex/thread/${encodeURIComponent(String(row._id || row.id))}`) } catch {}
  }
  // Show all threads; if count is known, display it
  return (
    <ThreadListItemBase title={row?.title || 'Thread'} timestamp={updatedAt} count={typeof count === 'number' ? count : undefined} onPress={open} />
  )
}
