import React from 'react'
import { ScrollView } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { FileChangeCard } from '@/components/jsonl/FileChangeCard'

export default function FileChangeLibraryScreen() {
  useHeaderTitle('FileChangeCard')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <FileChangeCard status="completed" changes={[
        { path: 'expo/app/session/index.tsx', kind: 'update' },
        { path: 'expo/components/code-block.tsx', kind: 'add' },
        { path: 'docs/syntax-highlighting.md', kind: 'add' },
        { path: 'expo/components/jsonl/CommandExecutionCard.tsx', kind: 'update' },
      ]} />
    </ScrollView>
  )
}

