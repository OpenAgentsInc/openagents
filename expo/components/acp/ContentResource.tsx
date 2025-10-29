import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { CodeBlock } from '@/components/code-block'

export function ContentResource({ resource }: { resource: { text?: string; blob?: string; uri: string; mimeType?: string | null } }) {
  const { text, blob, uri, mimeType } = resource
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 8, gap: 6 }}>
      <Text selectable style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{uri}{mimeType ? ` (${mimeType})` : ''}</Text>
      {typeof text === 'string' ? (
        <CodeBlock code={text} language={mimeType || 'text'} maxHeight={240} />
      ) : blob ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Binary resource ({mimeType || 'application/octet-stream'})</Text>
      ) : null}
    </View>
  )
}

