import React from 'react'
import Markdown from 'react-native-markdown-display'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function UserMessageRow({ text }: { text: string; numberOfLines?: number }) {
  const t = String(text || '')
  // Prefix each line with "> " to render as a Markdown blockquote
  const asQuote = `> ${t.replace(/\r?\n/g, '\n> ')}`
  return (
    <Markdown
      style={{
        body: { color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, lineHeight: 18 },
        paragraph: { color: Colors.foreground, marginTop: 0, marginBottom: 0 },
        blockquote: {
          borderLeftWidth: 3,
          borderColor: Colors.border,
          paddingLeft: 10,
          marginTop: 6,
          marginBottom: 8,
          backgroundColor: Colors.card,
        },
      }}
    >
      {asQuote}
    </Markdown>
  )
}
