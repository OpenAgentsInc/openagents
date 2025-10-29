import React from "react"
import { Pressable } from 'react-native'
import Markdown from "react-native-markdown-display"
import { Colors } from "@/constants/theme"
import { Typography } from "@/constants/typography"
import { copyToClipboard } from '@/lib/copy'

export function UserMessageRow({ text }: { text: string; numberOfLines?: number }) {
  const t = String(text || '')
  // Prefix each line with "> " to render as a Markdown blockquote
  const asQuote = `> ${t.replace(/\r?\n/g, '\n> ')}`
  return (
    <Pressable onLongPress={async () => { try { await copyToClipboard(asQuote, { haptics: true }) } catch {} }}>
    <Markdown
      style={{
        body: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 13, lineHeight: 18 },
        paragraph: { color: Colors.secondary, marginTop: 0, marginBottom: 0 },
        blockquote: {
          borderLeftWidth: 3,
          borderColor: Colors.border,
          // Nudge the quote left so the vertical rule extends left,
          // but keep text aligned by compensating padding.
          marginLeft: -12,
          paddingLeft: 12,
          marginTop: 6,
          marginBottom: 8,
          backgroundColor: Colors.card,
        },
      }}
    >
      {asQuote}
    </Markdown>
    </Pressable>
  )
}
