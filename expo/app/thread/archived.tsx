import React from 'react'
import { View, Text, ScrollView, Pressable } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useTinyvex } from '@/providers/tinyvex'
import { useArchiveStore } from '@/lib/archive-store'
import { ThreadListItemBase } from '@/components/drawer/ThreadListItem'
import { useHeaderTitle } from '@/lib/header-store'
import { router } from 'expo-router'

export default function ArchivedThreadsScreen() {
  useHeaderTitle('Archived')
  const { threads } = useTinyvex()
  const { archived, unarchive } = useArchiveStore()
  const isArchived = useArchiveStore((s) => s.isArchived)
  const list = React.useMemo(() => {
    const map = archived || {}
    const ids = new Set(Object.keys(map))
    const arr = Array.isArray(threads) ? threads.slice().filter((r: any) => ids.has(String(r.id))) : []
    arr.sort((a: any, b: any) => {
      const at = (a?.updated_at ?? a?.updatedAt ?? a?.created_at ?? a?.createdAt ?? 0) as number
      const bt = (b?.updated_at ?? b?.updatedAt ?? b?.created_at ?? b?.createdAt ?? 0) as number
      return bt - at
    })
    return arr
  }, [threads, archived])
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 10 }}>
      {list.length === 0 ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14 }}>No archived chats.</Text>
      ) : (
        list.map((row: any) => {
          const tid = String(row.id)
          const title = String(row?.title || '')
          const updatedAt = Number(row?.updated_at ?? row?.updatedAt ?? row?.created_at ?? row?.createdAt ?? 0)
          return (
            <View key={tid} style={{ flexDirection: 'row', alignItems: 'center' }}>
              <View style={{ flex: 1 }}>
                <ThreadListItemBase
                  title={title || 'Thread'}
                  timestamp={updatedAt}
                  count={undefined}
                  onPress={() => { try { router.push(`/thread/${encodeURIComponent(tid)}` as any) } catch {} }}
                />
              </View>
              <View style={{ marginLeft: 8 }}>
                <Pressable onPress={() => { try { unarchive(tid) } catch {} }} accessibilityRole="button" style={{ paddingVertical: 6, paddingHorizontal: 10, borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card }}>
                  <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 12 }}>Unarchive</Text>
                </Pressable>
              </View>
            </View>
          )
        })
      )}
    </ScrollView>
  )
}

