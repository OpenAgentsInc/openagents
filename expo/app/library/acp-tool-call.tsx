import React from 'react'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { SessionUpdateToolCall } from '@/components/acp'

export default function AcpToolCallDemo() {
  useHeaderTitle('ACP: Tool Call')
  const call1 = {
    sessionUpdate: 'tool_call',
    toolCallId: 'call_1',
    title: 'Run: bun test',
    kind: 'execute',
    status: 'in_progress',
    content: [{ type: 'content', content: { type: 'text', text: 'Running tests...' } }],
  } as any

  const call2 = {
    sessionUpdate: 'tool_call',
    toolCallId: 'call_2',
    title: 'Apply file changes',
    kind: 'edit',
    status: 'completed',
    content: [{ type: 'diff', path: 'src/main.ts', newText: "export const x = 1\n", oldText: "export const x = 0\n" }],
    locations: [{ path: 'src/main.ts', line: 1 }],
  } as any

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      <View>
        <SessionUpdateToolCall {...call1} />
      </View>
      <View>
        <SessionUpdateToolCall {...call2} />
      </View>
    </ScrollView>
  )
}

