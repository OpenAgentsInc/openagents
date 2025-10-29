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
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginBottom: 8 }}>ACP Components</Text>
      <LinkRow title="Agent Message" path="/library/acp-message" subtitle="SessionUpdate: agent_message_chunk (markdown)" />
      <LinkRow title="Agent Thought" path="/library/acp-thought" subtitle="SessionUpdate: agent_thought_chunk (markdown, indented)" />
      <LinkRow title="Tool Call" path="/library/acp-tool-call" subtitle="Tool kind, status, content (diff/content/terminal)" />
      <LinkRow title="Plan" path="/library/acp-plan" subtitle="SessionUpdate: plan (entries with status)" />
      <LinkRow title="Available Commands" path="/library/acp-available-commands" subtitle="SessionUpdate: available_commands_update" />
      <LinkRow title="Current Mode" path="/library/acp-current-mode" subtitle="SessionUpdate: current_mode_update" />

      <View style={{ height: 18 }} />
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginBottom: 8 }}>Example Screens</Text>
      <LinkRow title="ACP Example Conversation" path="/library/acp-example-conversation" subtitle="A full chat showcasing ACP components in order" />
    </ScrollView>
  )
}
