import React from 'react'
import { ScrollView, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { AgentMessageCard } from '@/components/jsonl/AgentMessageCard'
import { ReasoningCard } from '@/components/jsonl/ReasoningCard'
import { ReasoningHeadline } from '@/components/jsonl/ReasoningHeadline'
import { ExecBeginRow } from '@/components/jsonl/ExecBeginRow'
import { FileChangeCard } from '@/components/jsonl/FileChangeCard'
import { WebSearchRow } from '@/components/jsonl/WebSearchRow'
import { McpToolCallRow } from '@/components/jsonl/McpToolCallRow'
import { TodoListCard } from '@/components/jsonl/TodoListCard'

export default function ComponentLibraryScreen() {
  const samples = {
    agent_message: { type: 'agent_message', text: 'This is a basic agent message rendered via AgentMessageCard.' } as const,
    reasoning: { type: 'reasoning', text: '**Summarizing folder structure**\n\nOnly the headline is shown inline; full trace uses a detail view.' } as const,
    exec_begin: { command: ['bash', '-lc', 'cat expo/lib/codex-events.ts'] as const, cwd: '/Users/you/code/repo', parsed: [{ ReadFile: { name: 'expo/lib/codex-events.ts' } }] } as const,
    file_change: { status: 'completed', changes: [{ path: 'src/main.rs', kind: 'update' }, { path: 'README.md', kind: 'add' }] } as const,
    web_search: { query: 'expo updates runtimeVersion' } as const,
    mcp_call: { server: 'search', tool: 'web.search', status: 'completed' } as const,
    todo_list: { status: 'updated', items: [
      { text: 'Scan repo structure', completed: true },
      { text: 'List important files', completed: true },
      { text: 'Summarize key configs', completed: false },
    ] } as const,
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

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>file_change</Text>
        <FileChangeCard changes={samples.file_change.changes} status={samples.file_change.status} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>web_search</Text>
        <WebSearchRow query={samples.web_search.query} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>mcp_tool_call</Text>
        <McpToolCallRow server={samples.mcp_call.server} tool={samples.mcp_call.tool} status={samples.mcp_call.status} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>todo_list</Text>
        <TodoListCard items={samples.todo_list.items} status={samples.todo_list.status} />
      </View>
    </ScrollView>
  )
}
