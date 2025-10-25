import React from 'react'
import { ScrollView } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { CommandExecutionCard } from '@/components/jsonl/CommandExecutionCard'

export default function CommandLibraryScreen() {
  useHeaderTitle('CommandExecutionCard')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <CommandExecutionCard
        command="rg -n prism-react-renderer"
        status="completed"
        exitCode={0}
        sample={'README.md:12:prism-react-renderer'}
        outputLen={24}
        collapsed={true}
        maxBodyHeight={120}
      />
      <CommandExecutionCard
        command="rg -n prism-react-renderer"
        status="completed"
        exitCode={0}
        sample={'README.md:12:prism-react-renderer'}
        outputLen={24}
        showExitCode={true}
        showOutputLen={true}
      />
    </ScrollView>
  )
}

