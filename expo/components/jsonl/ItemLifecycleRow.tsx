import React from 'react'
import { Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function ItemLifecycleRow({
  phase,
  id,
  itemType,
  status,
}: {
  phase: 'started'|'updated'|'completed'
  id: string
  itemType: string
  status?: string
}) {
  const phaseLabel = phase === 'started' ? 'started' : phase === 'completed' ? 'completed' : 'updated'
  const tone = Colors.textSecondary
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color: tone, fontFamily: Typography.primary }}>
        <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold }}>{itemType}</Text>{' '}
        {phaseLabel}
        {status ? <Text> ({status})</Text> : null}
        {id ? <Text> · {id}</Text> : null}
      </Text>
    </View>
  )
}

