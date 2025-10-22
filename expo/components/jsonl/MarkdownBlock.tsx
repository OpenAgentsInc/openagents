import React from 'react'
import Markdown from 'react-native-markdown-display'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function MarkdownBlock({ markdown }: { markdown: string }) {
  return (
    <Markdown
      style={{
        body: { color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 13, lineHeight: 18 },
        paragraph: { color: Colors.textPrimary },
        code_inline: { backgroundColor: '#0F1217', color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 },
        code_block: { backgroundColor: '#0F1217', color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
        fence: { backgroundColor: '#0F1217', color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
      }}
    >
      {markdown}
    </Markdown>
  )
}

