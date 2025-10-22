import React from 'react'
import { Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function TurnEventRow({ phase, usage, message }: { phase: 'started'|'completed'|'failed'; usage?: { input_tokens: number; cached_input_tokens: number; output_tokens: number }; message?: string }) {
  const color = phase === 'failed' ? '#FCA5A5' : phase === 'completed' ? '#86EFAC' : Colors.textSecondary
  const title = phase === 'started' ? 'Turn started' : phase === 'completed' ? 'Turn completed' : 'Turn failed'
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color, fontFamily: Typography.bold }}>{title}</Text>
      {phase === 'completed' && usage ? (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>
          usage: in {usage.input_tokens} (+{usage.cached_input_tokens} cached) out {usage.output_tokens}
        </Text>
      ) : null}
      {phase === 'failed' && message ? (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>{message}</Text>
      ) : null}
    </View>
  )
}

