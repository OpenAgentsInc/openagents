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
import { CommandExecutionCard } from '@/components/jsonl/CommandExecutionCard'
import { ErrorRow } from '@/components/jsonl/ErrorRow'
import { TurnEventRow } from '@/components/jsonl/TurnEventRow'
import { ThreadStartedRow } from '@/components/jsonl/ThreadStartedRow'
import { ItemLifecycleRow } from '@/components/jsonl/ItemLifecycleRow'

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
    cmd_item: { command: 'ls -la', status: 'completed', exit_code: 0, sample: 'README.md\nexpo\ncrates', output_len: 24 } as const,
    err: { message: 'Something went wrong' } as const,
    turn_complete: { phase: 'completed' as const, usage: { input_tokens: 123, cached_input_tokens: 0, output_tokens: 45 } },
    thread_started: { thread_id: '67e5-5044-10b1-426f-9247-bb680e5fe0c8' },
    item_lifecycle_started: { phase: 'started' as const, id: 'item_42', item_type: 'my_custom_item', status: 'in_progress' },
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

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>command_execution</Text>
        <CommandExecutionCard command={samples.cmd_item.command} status={samples.cmd_item.status} exitCode={samples.cmd_item.exit_code} sample={samples.cmd_item.sample} outputLen={samples.cmd_item.output_len} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>error</Text>
        <ErrorRow message={samples.err.message} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>turn.completed</Text>
        <TurnEventRow phase={samples.turn_complete.phase} usage={samples.turn_complete.usage} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>thread.started</Text>
        <ThreadStartedRow threadId={samples.thread_started.thread_id} />
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>item.lifecycle (fallback)</Text>
        <ItemLifecycleRow phase={samples.item_lifecycle_started.phase} id={samples.item_lifecycle_started.id} itemType={samples.item_lifecycle_started.item_type} status={samples.item_lifecycle_started.status} />
      </View>
    </ScrollView>
  )
}
