import React from 'react'
import Markdown from 'react-native-markdown-display'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function MarkdownBlock({ markdown }: { markdown: string }) {
  return (
    <Markdown
      style={{
        body: { color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 13, lineHeight: 18 },
        paragraph: { color: Colors.textPrimary, marginTop: 2, marginBottom: 6 },
        heading1: { color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 22, lineHeight: 26, marginTop: 10, marginBottom: 6 },
        heading2: { color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 18, lineHeight: 22, marginTop: 10, marginBottom: 6 },
        heading3: { color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 16, lineHeight: 20, marginTop: 8, marginBottom: 4 },
        heading4: { color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 14, lineHeight: 18, marginTop: 6, marginBottom: 4 },
        list_item: { marginTop: 2, marginBottom: 2 },
        code_inline: { backgroundColor: Colors.codeBg, color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 },
        code_block: { backgroundColor: Colors.codeBg, color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
        fence: { backgroundColor: Colors.codeBg, color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, padding: 8 },
      }}
    >
      {markdown}
    </Markdown>
  )
}
