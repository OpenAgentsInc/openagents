import React from 'react'
import { ScrollView, Text, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'

import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock'
import { ReasoningHeadline } from '@/components/jsonl/ReasoningHeadline'
import { ReasoningCard } from '@/components/jsonl/ReasoningCard'
import { ExecBeginRow } from '@/components/jsonl/ExecBeginRow'
import { FileChangeCard } from '@/components/jsonl/FileChangeCard'
import { WebSearchRow } from '@/components/jsonl/WebSearchRow'
import { McpToolCallRow } from '@/components/jsonl/McpToolCallRow'
import { TodoListCard } from '@/components/jsonl/TodoListCard'
import { CommandExecutionCard } from '@/components/jsonl/CommandExecutionCard'
import { TurnEventRow } from '@/components/jsonl/TurnEventRow'
import { ThreadStartedRow } from '@/components/jsonl/ThreadStartedRow'
import { ErrorRow } from '@/components/jsonl/ErrorRow'

export default function ComponentLibraryScreen() {
  useHeaderTitle('Component Library')

  const Section = ({ title, children }: { title: string; children: React.ReactNode }) => (
    <View style={{ gap: 8 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12 }}>{title}</Text>
      <View>{children}</View>
    </View>
  )

  const md = [
    '# Heading',
    '',
    'Some inline `code` and a list:',
    '',
    '- One',
    '- Two',
    '',
    '```ts',
    'const x: number = 42',
    'console.log(x)',
    '```',
  ].join('\n')
  const reasoning = `**Plan**\n\n- Parse lines\n- Render JSONL rows\n- Highlight code`
  const reasoningItem = { type: 'reasoning' as const, text: reasoning }

  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <Section title="MarkdownBlock">
        <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 12 }}>
          <MarkdownBlock markdown={md} />
        </View>
      </Section>

      <Section title="ReasoningHeadline">
        <ReasoningHeadline text={reasoning} />
      </Section>

      <Section title="ReasoningCard">
        <ReasoningCard item={reasoningItem} />
      </Section>

      <Section title="ExecBeginRow (parsed and raw)">
        <ExecBeginRow payload={{ command: ['rg', '-n', 'openagents'], cwd: '/Users/me/code/openagents', parsed: [{ ListFiles: { path: 'docs' } }] }} />
        <ExecBeginRow payload={{ command: 'git status -sb', cwd: '/Users/me/code/openagents' }} />
      </Section>

      <Section title="FileChangeCard">
        <FileChangeCard status="completed" changes={[
          { path: 'expo/app/session/index.tsx', kind: 'update' },
          { path: 'expo/components/code-block.tsx', kind: 'add' },
          { path: 'docs/syntax-highlighting.md', kind: 'add' },
          { path: 'expo/components/jsonl/CommandExecutionCard.tsx', kind: 'update' },
        ]} />
      </Section>

      <Section title="CommandExecutionCard (feed style: no meta)">
        <CommandExecutionCard
          command="rg -n prism-react-renderer"
          status="completed"
          exitCode={0}
          sample={'README.md:12:prism-react-renderer'}
          outputLen={24}
          // defaults hide exit code and output length metadata
        />
      </Section>

      <Section title="CommandExecutionCard (detail style: with meta)">
        <CommandExecutionCard
          command="rg -n prism-react-renderer"
          status="completed"
          exitCode={0}
          sample={'README.md:12:prism-react-renderer'}
          outputLen={24}
          showExitCode={true}
          showOutputLen={true}
        />
      </Section>

      <Section title="WebSearchRow / McpToolCallRow">
        <WebSearchRow query="prism-react-renderer themes" />
        <McpToolCallRow server="github" tool="search" status="completed" />
      </Section>

      <Section title="TodoListCard">
        <TodoListCard status="updated" items={[
          { text: 'Wire up Prism in Markdown', completed: true },
          { text: 'Show raw JSON in detail', completed: true },
          { text: 'Add more samples', completed: false },
        ]} />
      </Section>

      <Section title="Turn / Thread / Error">
        <ThreadStartedRow threadId="abcd1234" />
        <TurnEventRow phase="started" />
        <TurnEventRow phase="completed" usage={{ input_tokens: 1200, cached_input_tokens: 300, output_tokens: 420 }} />
        <ErrorRow message="Something went wrong while fetching." />
      </Section>
    </ScrollView>
  )
}
