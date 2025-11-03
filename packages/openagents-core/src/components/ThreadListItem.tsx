import React from 'react'
import { Pressable, Text, View } from 'react-native'
import { Colors } from '@openagentsinc/theme'
import { Typography } from '@openagentsinc/theme'

export type ThreadListItemProps = {
  title: string
  meta?: React.ReactNode
  timestamp?: number | null
  count?: number | null
  onPress?: () => void
  onLongPress?: () => void
  testID?: string
}

export function ThreadListItem({ title, meta, timestamp, count, onPress, onLongPress, testID }: ThreadListItemProps) {
  const ts = typeof timestamp === 'number' && timestamp > 0 ? formatRelative(new Date(timestamp).getTime()) : ''
  return (
    <Pressable onPress={onPress} onLongPress={onLongPress} delayLongPress={300} accessibilityRole="button" testID={testID} style={{ paddingVertical: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center' }}>
        <View style={{ flex: 1, paddingRight: 8 }}>
          <Text numberOfLines={1} style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{title || 'Thread'}</Text>
          {meta ? (
            <View style={{ marginTop: 2 }}>{meta}</View>
          ) : null}
          {!!ts && !meta && (
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6, marginTop: 2 }}>
              {/* Consumers may choose to include an icon before this timestamp */}
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

