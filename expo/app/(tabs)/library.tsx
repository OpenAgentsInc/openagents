import React from 'react'
import { ScrollView, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { AgentMessageCard } from '@/components/jsonl/AgentMessageCard'
import { ReasoningCard } from '@/components/jsonl/ReasoningCard'
import { ReasoningHeadline } from '@/components/jsonl/ReasoningHeadline'
import { ExecBeginRow } from '@/components/jsonl/ExecBeginRow'

export default function ComponentLibraryScreen() {
  const samples = {
    agent_message: { type: 'agent_message', text: 'This is a basic agent message rendered via AgentMessageCard.' } as const,
    reasoning: { type: 'reasoning', text: '**Summarizing folder structure**\n\nOnly the headline is shown inline; full trace uses a detail view.' } as const,
    exec_begin: { command: ['bash', '-lc', 'ls -la'] as const, cwd: '/Users/you/code/repo', parsed: [{ ListFiles: { cmd: ['ls','-la'], path: 'docs' } }] } as const,
  }

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 14 }}>
      <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 18 }}>Component Library</Text>
      <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>
        Basic renderers for Exec JSONL ThreadItem variants. We will expand this list and reuse these in the session feed.
      </Text>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>agent_message</Text>
        <AgentMessageCard item={samples.agent_message} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>reasoning (headline preview)</Text>
        <ReasoningHeadline text={samples.reasoning.text} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>reasoning (full)</Text>
        <ReasoningCard item={samples.reasoning} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>exec_command_begin</Text>
        <ExecBeginRow payload={samples.exec_begin} />
      </View>
    </ScrollView>
  )
}
