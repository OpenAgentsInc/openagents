import React from 'react'
import { View, Pressable } from 'react-native'
import Markdown from 'react-native-markdown-display'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { CodeBlock } from '@/components/code-block'
import { InlineToast } from '@/components/inline-toast'
import { copyToClipboard } from '@/lib/copy'

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

  const toCopy = String(headline || '')
  const [copied, setCopied] = React.useState(false)
  React.useEffect(() => {
    if (!copied) return
    const t = setTimeout(() => setCopied(false), 1400)
    return () => clearTimeout(t)
  }, [copied])
  return (
    <View style={{ marginTop: 8, position: 'relative' }}>
      <Pressable onLongPress={async () => { try { await copyToClipboard(toCopy, { haptics: true, local: true }); setCopied(true) } catch {} }}>
      <Markdown
      style={{
        body: { color: Colors.tertiary, fontFamily: Typography.primary, fontSize: 12, lineHeight: 16 },
        paragraph: { color: Colors.tertiary, marginTop: 0, marginBottom: 2 },
        strong: { fontFamily: Typography.bold, color: Colors.tertiary },
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
      </Pressable>
      {copied ? <InlineToast text="Copied" position="bottom" align="right" /> : null}
    </View>
  )
}
