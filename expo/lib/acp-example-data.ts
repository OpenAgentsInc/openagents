import type { AvailableCommand, ContentBlock, PlanEntry, ToolCallLike } from '@/types/acp'

export type ExampleItem =
  | { id: string; type: 'user_message'; content: ContentBlock }
  | { id: string; type: 'agent_message'; content: ContentBlock }
  | { id: string; type: 'agent_thought'; content: ContentBlock }
  | { id: string; type: 'current_mode_update'; currentModeId: string }
  | { id: string; type: 'available_commands_update'; available_commands: readonly AvailableCommand[] }
  | { id: string; type: 'plan'; entries: readonly PlanEntry[] }
  | { id: string; type: 'tool_call'; props: ToolCallLike }

export const exampleItems: readonly ExampleItem[] = [
  { id: '1', type: 'user_message', content: { type: 'text', text: 'Could you check the git status and summarize the changes?' } },
  { id: '2', type: 'agent_thought', content: { type: 'text', text: '**Assessing code status with git**\n\nI will run `git status --short` and summarize the output.' } },
  { id: '3', type: 'current_mode_update', currentModeId: 'coding' },
  { id: '4', type: 'available_commands_update', available_commands: [
    { name: 'run', description: 'Execute a shell command' },
    { name: 'edit', description: 'Apply a file change' },
    { name: 'search', description: 'Search the web for context' },
  ] },
  { id: '5', type: 'plan', entries: [
    { content: 'Assess code status with git', priority: 'medium', status: 'in_progress' },
    { content: 'Summarize repository changes', priority: 'low', status: 'pending' },
    { content: 'Report next steps', priority: 'low', status: 'completed' },
  ] },
  { id: '6', type: 'tool_call', props: {
    title: 'Run: bash -lc "git status --short"',
    kind: 'execute',
    status: 'in_progress',
    content: [
      { type: 'content', content: { type: 'text', text: 'Running git status…' } },
      { type: 'terminal', terminalId: 'example/terminal-1' } as any,
    ] as any,
    locations: [{ path: '.', line: undefined }],
  } },
  { id: '7', type: 'tool_call', props: {
    title: 'Run: bash -lc "git status --short"',
    kind: 'execute',
    status: 'completed',
    content: [
      { type: 'content', content: { type: 'text', text: 'M expo/app/convex/thread/[id].tsx\nA expo/app/library/acp-example-conversation.tsx' } },
    ] as any,
  } },
  { id: '8', type: 'tool_call', props: {
    title: 'Edit: update README section',
    kind: 'edit',
    status: 'completed',
    content: [ { type: 'diff', path: 'README.md', oldText: 'Old heading', newText: 'New heading' } as any ] as any,
    locations: [{ path: 'README.md', line: 1 }],
  } },
  { id: '9', type: 'tool_call', props: {
    title: 'Read: openagents docs',
    kind: 'read',
    status: 'completed',
    content: [ { type: 'content', content: { type: 'resource_link', name: 'exec-jsonl-schema.md', uri: 'docs/exec-jsonl-schema.md', mimeType: 'text/markdown' } as any } ] as any,
  } },
  { id: '10', type: 'agent_message', content: { type: 'text', text: 'Summary: One modified file and one new file. No untracked deletions. Next, I can open a PR or stage changes as needed.' } },
]

export function findExampleItem(id: string): ExampleItem | undefined {
  return exampleItems.find((x) => String(x.id) === String(id))
}

