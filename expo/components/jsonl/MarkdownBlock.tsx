import React from 'react'
import { Pressable, View } from 'react-native'
import Markdown from 'react-native-markdown-display'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { CodeBlock } from '@/components/code-block'
import { InlineToast } from '@/components/inline-toast'
import { copyToClipboard } from '@/lib/copy'

export function MarkdownBlock({ markdown }: { markdown: string }) {
  const [copied, setCopied] = React.useState(false)
  React.useEffect(() => {
    if (!copied) return
    const t = setTimeout(() => setCopied(false), 1400)
    return () => clearTimeout(t)
  }, [copied])
  const rules: any = {
    fence: (node: any) => {
      const lang = typeof node?.params === 'string' ? node.params : (typeof node?.info === 'string' ? node.info : undefined)
      const code = String(node?.content ?? '')
      return <CodeBlock key={String(node?.key ?? `fence-${code.slice(0,16)}`)} code={code} language={lang} />
    },
    code_block: (node: any) => {
      const code = String(node?.content ?? '')
      return <CodeBlock key={String(node?.key ?? `code-${code.slice(0,16)}`)} code={code} />
    },
  }
  return (
    <View style={{ position: 'relative' }}>
    <Pressable onLongPress={async () => { try { await copyToClipboard(String(markdown || ''), { haptics: true, local: true }); setCopied(true) } catch {} }}>
    <Markdown
      style={{
        body: { color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13, lineHeight: 18 },
        paragraph: { color: Colors.foreground, marginTop: 2, marginBottom: 6 },
        heading1: { color: Colors.foreground, fontFamily: Typography.bold, fontSize: 22, lineHeight: 26, marginTop: 10, marginBottom: 6 },
        heading2: { color: Colors.foreground, fontFamily: Typography.bold, fontSize: 18, lineHeight: 22, marginTop: 10, marginBottom: 6 },
        heading3: { color: Colors.foreground, fontFamily: Typography.bold, fontSize: 16, lineHeight: 20, marginTop: 8, marginBottom: 4 },
        heading4: { color: Colors.foreground, fontFamily: Typography.bold, fontSize: 14, lineHeight: 18, marginTop: 6, marginBottom: 4 },
        list_item: { marginTop: 2, marginBottom: 2 },
        code_inline: { backgroundColor: Colors.black, color: Colors.foreground, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 },
        code_block: { backgroundColor: Colors.black, color: Colors.foreground, borderWidth: 1, borderColor: Colors.border, padding: 0 },
        fence: { backgroundColor: Colors.black, color: Colors.foreground, borderWidth: 1, borderColor: Colors.border, padding: 0 },
      }}
      rules={rules}
    >
      {markdown}
    </Markdown>
    </Pressable>
    {copied ? <InlineToast text="Copied" position="bottom" align="right" /> : null}
    </View>
  )
}
