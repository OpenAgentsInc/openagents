import React from 'react'
import { View } from 'react-native'
import Markdown from 'react-native-markdown-display'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import type { ReasoningItem } from '@/types/exec-jsonl'
import { CodeBlock } from '@/components/code-block'

export function ReasoningCard({ item }: { item: ReasoningItem }) {
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 0, padding: 12 }}>
      <Markdown
        style={{
          body: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, lineHeight: 18 },
          code_inline: { backgroundColor: Colors.black, color: Colors.secondary, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 },
          code_block: { backgroundColor: Colors.black, color: Colors.secondary, borderWidth: 1, borderColor: Colors.border, padding: 0 },
          fence: { backgroundColor: Colors.black, color: Colors.secondary, borderWidth: 1, borderColor: Colors.border, padding: 0 },
        }}
        rules={{
          fence: (node: any) => (
            <CodeBlock key={String(node?.key ?? `fence-${String(node?.content ?? '').slice(0,16)}`)} code={String(node?.content ?? '')} language={String((node?.params ?? node?.info) || '')} />
          ),
          code_block: (node: any) => (
            <CodeBlock key={String(node?.key ?? `code-${String(node?.content ?? '').slice(0,16)}`)} code={String(node?.content ?? '')} />
          ),
        }}
      >
        {item.text}
      </Markdown>
    </View>
  )
}
