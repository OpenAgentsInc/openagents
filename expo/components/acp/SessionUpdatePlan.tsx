import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import type { PlanEntry } from '@/types/acp'
import { MaterialCommunityIcons } from '@expo/vector-icons'

export function SessionUpdatePlan({ entries }: { entries: ReadonlyArray<PlanEntry> }) {
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 12, gap: 8 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold }}>Plan</Text>
      <View style={{ gap: 6 }}>
        {entries.map((e, i) => (
          <View key={i} style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
            <MaterialCommunityIcons name={iconForStatus(e.status) as unknown as never} size={16} color={colorForStatus(e.status)} />
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{e.content}</Text>
          </View>
        ))}
      </View>
    </View>
  )
}

function iconForStatus(status: PlanEntry['status']): string {
  switch (status) {
    case 'completed':
      return 'check'
    case 'in_progress':
      return 'progress-clock'
    case 'pending':
    default:
      return 'clock-outline'
  }
}

function colorForStatus(status: PlanEntry['status']): string {
  switch (status) {
    case 'completed':
      return Colors.success
    case 'in_progress':
      return Colors.secondary
    case 'pending':
    default:
      return Colors.tertiary
  }
}
