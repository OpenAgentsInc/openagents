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
      <LinkRow title="MarkdownBlock" path="/library/markdown" subtitle="Fenced/inline code, lists, headers" />
      <LinkRow title="ReasoningHeadline" path="/library/reasoning-headline" subtitle="Top-line reasoning extraction + markdown" />
      <LinkRow title="ReasoningCard" path="/library/reasoning-card" subtitle="Card with markdown + code" />
      <LinkRow title="ExecBeginRow" path="/library/exec" subtitle="Parsed and raw command rows" />
      <LinkRow title="FileChangeCard" path="/library/file-change" />
      <LinkRow title="CommandExecutionCard" path="/library/command" subtitle="Feed style and detailed style" />
      <LinkRow title="WebSearch & MCP Call" path="/library/search-mcp" />
      <LinkRow title="TodoListCard" path="/library/todo" />
      <LinkRow title="Turn & Error Rows" path="/library/turn-error" />
      <LinkRow title="Unused Samples" path="/library/unused" subtitle="Hidden in feed; for reference" />
    </ScrollView>
  )
}
