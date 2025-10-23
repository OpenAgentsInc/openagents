import React from 'react'
import { View } from 'react-native'
import Markdown from 'react-native-markdown-display'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import type { ReasoningItem } from '@/types/exec-jsonl'

export function ReasoningCard({ item }: { item: ReasoningItem }) {
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 0, padding: 12 }}>
      <Markdown
        style={{
          body: { color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12, lineHeight: 18 },
          code_inline: { backgroundColor: Colors.codeBg, color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 },
          code_block: { backgroundColor: Colors.codeBg, color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
          fence: { backgroundColor: Colors.codeBg, color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
        }}
      >
        {item.text}
      </Markdown>
    </View>
  )
}
