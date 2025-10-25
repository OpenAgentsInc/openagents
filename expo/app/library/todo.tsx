import React from 'react'
import { ScrollView } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { TodoListCard } from '@/components/jsonl/TodoListCard'

export default function TodoLibraryScreen() {
  useHeaderTitle('TodoListCard')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <TodoListCard status="updated" items={[
        { text: 'Wire up Prism in Markdown', completed: true },
        { text: 'Show raw JSON in detail', completed: true },
        { text: 'Add more samples', completed: false },
      ]} />
    </ScrollView>
  )
}

