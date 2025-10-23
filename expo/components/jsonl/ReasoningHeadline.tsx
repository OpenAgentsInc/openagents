import React from 'react'
import Markdown from 'react-native-markdown-display'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { CodeBlock } from '@/components/code-block'

export function ReasoningHeadline({ text }: { text: string }) {
  // Extract only the first bold headline (e.g., **Title**) from the full reasoning text
  let headline: string | null = null
  const first = text.indexOf('**')
  if (first >= 0) {
    const second = text.indexOf('**', first + 2)
    if (second > first + 2) headline = text.slice(first, second + 2)
  }
  const sanitize = (s: string) => {
    const core = s.replace(/^\*\*|\*\*$/g, '').replace(/^`|`$/g, '').trim().toLowerCase()
    if (core === 'unknown' || core === 'n/a' || core === 'none' || core === 'null') return '**Reasoning**'
    return s
  }

  if (!headline) {
    const firstLine = text.split(/\r?\n/).find((ln) => ln.trim().length > 0) ?? ''
    const base = firstLine.trim().length ? firstLine.trim() : '**Reasoning**'
    headline = sanitize(base)
  } else {
    headline = sanitize(headline)
  }

  return (
    <Markdown
      style={{
        body: { color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, lineHeight: 16 },
        paragraph: { color: Colors.secondary, marginTop: 0, marginBottom: 2 },
        strong: { fontFamily: Typography.bold, color: Colors.secondary },
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
      {headline}
    </Markdown>
  )
}
