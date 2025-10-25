import React from 'react'
import { ScrollView } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { WebSearchRow } from '@/components/jsonl/WebSearchRow'
import { McpToolCallRow } from '@/components/jsonl/McpToolCallRow'

export default function SearchMcpLibraryScreen() {
  useHeaderTitle('WebSearch & MCP')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <WebSearchRow query="prism-react-renderer themes" />
      <McpToolCallRow server="github" tool="search" status="completed" />
    </ScrollView>
  )
}

