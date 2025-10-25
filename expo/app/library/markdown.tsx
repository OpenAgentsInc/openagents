import React from 'react'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'

export default function MarkdownLibraryScreen() {
  useHeaderTitle('MarkdownBlock')
  const md = [
    '# Heading',
    '',
    'Some inline `code` and a list:',
    '',
    '- One',
    '- Two',
    '',
    '```ts',
    'const x: number = 42',
    'console.log(x)',
    '```',
  ].join('\n')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 12 }}>
        <MarkdownBlock markdown={md} />
      </View>
    </ScrollView>
  )
}

