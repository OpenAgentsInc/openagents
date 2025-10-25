import React from 'react'
import { Pressable, ScrollView, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useRouter } from 'expo-router'

export default function ComponentLibraryScreen() {
  useHeaderTitle('Component Library')
  const router = useRouter()

  const LinkRow = ({ title, path, subtitle }: { title: string; path: string; subtitle?: string }) => (
    <Pressable accessibilityRole="button" onPress={() => router.push(path as any)} style={{ paddingVertical: 12 }}>
      <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{title}</Text>
      {subtitle ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{subtitle}</Text>
      ) : null}
      <View style={{ height: 1, backgroundColor: Colors.border, marginTop: 12 }} />
    </Pressable>
  )

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, paddingBottom: 40 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginBottom: 8 }}>JSONL Components</Text>
      <LinkRow title="MarkdownBlock" path="/library/markdown" subtitle="Fenced/inline code, lists, headers" />
      <LinkRow title="UserMessageRow" path="/library/user-message" subtitle="User-authored content in the feed" />
      <LinkRow title="ReasoningHeadline" path="/library/reasoning-headline" subtitle="Top-line reasoning extraction + markdown" />
      <LinkRow title="ReasoningCard" path="/library/reasoning-card" subtitle="Card with markdown + code" />
      <LinkRow title="ExecBeginRow" path="/library/exec" subtitle="Parsed and raw command rows" />
      <LinkRow title="FileChangeCard" path="/library/file-change" subtitle="Summary (+/~/-) and list of changed files" />
      <LinkRow title="CommandExecutionCard" path="/library/command" subtitle="Command output preview with collapsible body" />
      <LinkRow title="WebSearch & MCP Call" path="/library/search-mcp" subtitle="Rows for web search queries and MCP tool calls" />
      <LinkRow title="TodoListCard" path="/library/todo" subtitle="Agent plan checklist with completion state" />
      <LinkRow title="Turn & Error Rows" path="/library/turn-error" subtitle="Turn lifecycle events and surfaced errors" />
      <LinkRow title="Drawer Components" path="/library/drawer" subtitle="Thread history row with count badge" />
      <LinkRow title="Unused Samples" path="/library/unused" subtitle="Hidden in feed; for reference" />
    </ScrollView>
  )
}
