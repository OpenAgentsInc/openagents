import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { CodeBlock } from '@/components/code-block'

export function ToolCallContentDiff({ path, oldText, newText }: { path: string; oldText?: string | null; newText: string }) {
  return (
    <View style={{ gap: 6 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Diff: {path}</Text>
      <CodeBlock code={newText} language={inferLanguage(path)} maxHeight={200} />
      {typeof oldText === 'string' && oldText.length > 0 ? (
        <View>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginBottom: 4 }}>Previous</Text>
          <CodeBlock code={oldText} language={inferLanguage(path)} maxHeight={120} />
        </View>
      ) : null}
    </View>
  )
}

function inferLanguage(path: string | undefined): string | undefined {
  if (!path) return undefined
  const p = path.toLowerCase()
  if (p.endsWith('.ts') || p.endsWith('.tsx')) return 'tsx'
  if (p.endsWith('.js') || p.endsWith('.jsx')) return 'javascript'
  if (p.endsWith('.rs')) return 'rust'
  if (p.endsWith('.py')) return 'python'
  if (p.endsWith('.json')) return 'json'
  if (p.endsWith('.md')) return 'markdown'
  if (p.endsWith('.sh') || p.endsWith('.bash') || p.endsWith('.zsh')) return 'bash'
  return undefined
}
