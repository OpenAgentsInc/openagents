import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export type TodoItem = { text: string; completed: boolean }

export function TodoListCard({ items, status }: { items: ReadonlyArray<TodoItem>; status?: string }) {
  const done = items.filter(i => i.completed).length
  const total = items.length
  const hdr = typeof status === 'string' ? status : undefined
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 12, padding: 12, gap: 8 }}>
      <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>todo_list{hdr ? ` (${hdr})` : ''}</Text>
      <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>{done}/{total} complete</Text>
      <View style={{ gap: 6 }}>
        {items.map((it, idx) => (
          <View key={idx} style={{ flexDirection: 'row', gap: 8, alignItems: 'flex-start' }}>
            <View style={{ width: 14, height: 14, borderRadius: 3, borderWidth: 1, borderColor: Colors.border, backgroundColor: it.completed ? '#22C55E' : 'transparent' }} />
            <Text style={{ color: it.completed ? Colors.textSecondary : Colors.textPrimary, fontFamily: Typography.primary, textDecorationLine: it.completed ? 'line-through' : 'none' }}>
              {it.text}
            </Text>
          </View>
        ))}
      </View>
    </View>
  )
}

