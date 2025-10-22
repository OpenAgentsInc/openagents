import React from 'react'
import Markdown from 'react-native-markdown-display'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function ReasoningHeadline({ text }: { text: string }) {
  // Extract only the first bold headline (e.g., **Title**) from the full reasoning text
  let headline: string | null = null
  const first = text.indexOf('**')
  if (first >= 0) {
    const second = text.indexOf('**', first + 2)
    if (second > first + 2) headline = text.slice(first, second + 2)
  }
  if (!headline) {
    const firstLine = text.split(/\r?\n/).find((ln) => ln.trim().length > 0) ?? ''
    headline = firstLine.trim().length ? firstLine.trim() : '**Reasoning**'
  }

  return (
    <Markdown
      style={{
        body: { color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12, lineHeight: 18 },
        paragraph: { color: Colors.textSecondary },
        code_inline: { backgroundColor: '#0F1217', color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 },
        code_block: { backgroundColor: '#0F1217', color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
        fence: { backgroundColor: '#0F1217', color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
      }}
    >
      {headline}
    </Markdown>
  )
}

