import React from 'react'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { SessionUpdateAgentMessageChunk, SessionUpdateAgentThoughtChunk, SessionUpdatePlan, SessionUpdateToolCall, SessionUpdateAvailableCommandsUpdate, SessionUpdateCurrentModeUpdate } from '@/components/acp'

export default function ACPExampleConversationScreen() {
  useHeaderTitle('ACP Example Conversation')

  // Static example data demonstrating the ACP components in a single, long chat
  const planEntries = [
    { content: 'Assess code status with git', priority: 'medium', status: 'in_progress' },
    { content: 'Summarize repository changes', priority: 'low', status: 'pending' },
    { content: 'Report next steps', priority: 'low', status: 'completed' },
  ] as const

  const availableCommands = [
    { name: 'run', description: 'Execute a shell command' },
    { name: 'edit', description: 'Apply a file change' },
    { name: 'search', description: 'Search the web for context' },
  ] as const

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12, paddingBottom: 32 }}>
      {/* User message */}
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdateAgentMessageChunk content={{ type: 'text', text: 'User: Could you check the git status and summarize the changes?' }} />
      </View>

      {/* Agent thought (markdown) */}
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdateAgentThoughtChunk content={{ type: 'text', text: '**Assessing code status with git**\n\nI will run `git status --short` and summarize the output.' }} />
      </View>

      {/* Current mode + available commands + plan */}
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdateCurrentModeUpdate currentModeId={'coding'} />
      </View>
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdateAvailableCommandsUpdate available_commands={availableCommands as any} />
      </View>
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdatePlan entries={planEntries as any} />
      </View>

      {/* Tool call: execute (in progress) with terminal and text */}
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdateToolCall
          title={'Run: bash -lc "git status --short"'}
          kind={'execute'}
          status={'in_progress'}
          content={[
            { type: 'content', content: { type: 'text', text: 'Running git status…' } },
            { type: 'terminal', terminalId: 'example/terminal-1' } as any,
          ] as any}
          locations={[{ path: '.', line: undefined }] as any}
        />
      </View>

      {/* Tool call: execute (completed) with captured output sample */}
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdateToolCall
          title={'Run: bash -lc "git status --short"'}
          kind={'execute'}
          status={'completed'}
          content={[
            { type: 'content', content: { type: 'text', text: 'M expo/app/convex/thread/[id].tsx\nA expo/app/library/acp-example-conversation.tsx' } },
          ] as any}
        />
      </View>

      {/* Tool call: edit (diff) */}
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdateToolCall
          title={'Edit: update README section'}
          kind={'edit'}
          status={'completed'}
          content={[
            { type: 'diff', path: 'README.md', oldText: 'Old heading', newText: 'New heading' } as any,
          ] as any}
          locations={[{ path: 'README.md', line: 1 }] as any}
        />
      </View>

      {/* Tool call: read (resource link) */}
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdateToolCall
          title={'Read: openagents docs'}
          kind={'read'}
          status={'completed'}
          content={[
            { type: 'content', content: { type: 'resource_link', name: 'exec-jsonl-schema.md', uri: 'docs/exec-jsonl-schema.md', mimeType: 'text/markdown' } as any },
          ] as any}
        />
      </View>

      {/* Agent final message */}
      <View style={{ paddingVertical: 2 }}>
        <SessionUpdateAgentMessageChunk content={{ type: 'text', text: 'Summary: One modified file and one new file. No untracked deletions. Next, I can open a PR or stage changes as needed.' }} />
      </View>
    </ScrollView>
  )
}

