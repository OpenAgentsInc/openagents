import React from 'react'
import { ScrollView } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { ExecBeginRow } from '@/components/jsonl/ExecBeginRow'

export default function ExecLibraryScreen() {
  useHeaderTitle('ExecBeginRow')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <ExecBeginRow payload={{ command: ['rg', '-n', 'openagents'], cwd: '/Users/me/code/openagents', parsed: [{ ListFiles: { path: 'docs' } }] }} />
      <ExecBeginRow payload={{ command: 'git status -sb', cwd: '/Users/me/code/openagents' }} />
    </ScrollView>
  )
}

