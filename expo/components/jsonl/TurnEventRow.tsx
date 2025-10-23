import React from 'react'
import { Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function TurnEventRow({ phase, usage, message, showUsage = false, durationMs }: { phase: 'started'|'completed'|'failed'; usage?: { input_tokens: number; cached_input_tokens: number; output_tokens: number }; message?: string; showUsage?: boolean; durationMs?: number }) {
  const isCompleted = phase === 'completed'
  const color = phase === 'failed' ? Colors.danger : isCompleted ? Colors.tertiary : Colors.secondary
  const titleBase = phase === 'started' ? 'Turn started' : isCompleted ? 'Turn completed' : 'Turn failed'
  const duration = typeof durationMs === 'number' ? ` (${(durationMs/1000).toFixed(1)}s)` : ''
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color, fontFamily: Typography.primary, fontSize: 12, textAlign: isCompleted ? 'center' : 'left' }}>{titleBase}{duration}</Text>
      {phase === 'completed' && usage && showUsage ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
          usage: in {usage.input_tokens} (+{usage.cached_input_tokens} cached) out {usage.output_tokens}
        </Text>
      ) : null}
      {phase === 'failed' && message ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{message}</Text>
      ) : null}
    </View>
  )
}
