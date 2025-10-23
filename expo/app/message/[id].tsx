import React, { useState, useCallback, useMemo, useEffect } from 'react';
import { useLocalSearchParams, Stack } from 'expo-router';
import { ScrollView, Text, View, Pressable } from 'react-native';
import { useLogStore, getLog, loadLogs } from '@/lib/log-store';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { MarkdownBlock } from '@/components/jsonl/MarkdownBlock';
import { CommandExecutionCard } from '@/components/jsonl/CommandExecutionCard';
import { ReasoningCard } from '@/components/jsonl/ReasoningCard';
import { TurnEventRow } from '@/components/jsonl/TurnEventRow';
import { ExecBeginRow } from '@/components/jsonl/ExecBeginRow';
import { FileChangeCard } from '@/components/jsonl/FileChangeCard';
import { WebSearchRow } from '@/components/jsonl/WebSearchRow';
import { McpToolCallRow } from '@/components/jsonl/McpToolCallRow';
import { TodoListCard } from '@/components/jsonl/TodoListCard';
import { ErrorRow } from '@/components/jsonl/ErrorRow';
import { ItemLifecycleRow } from '@/components/jsonl/ItemLifecycleRow';
import { ThreadStartedRow } from '@/components/jsonl/ThreadStartedRow';
import { Colors } from '@/constants/theme';
import { Typography } from '@/constants/typography';
import * as Clipboard from 'expo-clipboard';
import { useHeaderTitle } from '@/lib/header-store';
import { CodeBlock } from '@/components/code-block';

export default function MessageDetail() {
  const { id } = useLocalSearchParams<{ id: string }>();
  useHeaderTitle(id ? `Message ${id}` : 'Message');
  const num = Number(id);
  const logs = useLogStore((s) => s.logs);
  const [hydrating, setHydrating] = React.useState(true);
  useEffect(() => { let alive = true; loadLogs().catch(() => {}).finally(() => { if (alive) setHydrating(false); }); return () => { alive = false; }; }, []);
  const detail = useMemo(() => {
    if (Number.isNaN(num)) return undefined;
    return logs.find((entry) => entry.id === num) ?? getLog(num);
  }, [logs, num]);
  const insets = useSafeAreaInsets();
  const [copied, setCopied] = useState(false)
  const copy = useCallback(async (text: string) => {
    try { await Clipboard.setStringAsync(text) } catch {}
    setCopied(true); setTimeout(() => setCopied(false), 800)
  }, [])
  const rawText = typeof detail?.text === 'string' ? detail.text : undefined;
  const isUserMsg = typeof rawText === 'string' && /^\s*>/.test(rawText);
  const cleanBody = typeof rawText === 'string' ? rawText.replace(/^\s*>\s?/, '') : '';

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Stack.Screen options={{ headerShown: false }} />
      <ScrollView
        contentContainerStyle={{
          paddingTop: 16,
          paddingHorizontal: 16,
          paddingBottom: 32 + insets.bottom,
        }}
      >
        {!detail ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>{hydrating ? 'Loading message…' : 'Message not found.'}</Text>
        ) : (
          <View style={{ gap: 12 }}>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Kind: {detail.kind}</Text>
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>ID: {detail.id}</Text>
            {detail.ts && (
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Time: {new Date(detail.ts).toLocaleString()}</Text>
            )}
            {detail.kind === 'md' || detail.text.startsWith('::md::') ? (
              <Pressable onLongPress={() => copy(detail.text.startsWith('::md::') ? detail.text.slice('::md::'.length) : detail.text)}>
                <MarkdownBlock markdown={detail.text.startsWith('::md::') ? detail.text.slice('::md::'.length) : detail.text} />
                {copied ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Copied</Text> : null}
              </Pressable>
            ) : detail.kind === 'reason' || detail.text.startsWith('::reason::') ? (
              <Pressable onLongPress={() => copy(detail.text.startsWith('::reason::') ? detail.text.slice('::reason::'.length) : detail.text)}>
                <ReasoningCard item={{ type: 'reasoning', text: detail.text.startsWith('::reason::') ? detail.text.slice('::reason::'.length) : detail.text }} />
                {copied ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Copied</Text> : null}
              </Pressable>
            ) : detail.kind === 'turn' ? (
              (() => {
                try {
                  const obj = JSON.parse(detail.text)
                  return (
                    <TurnEventRow phase={obj.phase ?? 'started'} usage={obj.usage} message={obj.message} showUsage={true} />
                  )
                } catch {
                  return (
                    <Text selectable style={{ color: Colors.foreground, fontFamily: Typography.primary, lineHeight: 18 }}>{detail?.text}</Text>
                  )
                }
              })()
            ) : detail.kind === 'cmd' ? (
              (() => {
                try {
                  const obj = JSON.parse(detail.text);
                  return (
                    <Pressable onLongPress={() => copy(detail.text)}>
                      <CommandExecutionCard command={obj.command ?? ''} status={obj.status} exitCode={obj.exit_code} sample={obj.sample} outputLen={obj.output_len} showExitCode={true} showOutputLen={true} />
                      {copied ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Copied</Text> : null}
                    </Pressable>
                  );
                } catch {
                  return (
                    <Pressable onLongPress={() => copy(detail.text)}>
                      <Text selectable style={{ color: Colors.foreground, fontFamily: Typography.primary, lineHeight: 18 }}>{detail.text}</Text>
                      {copied ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Copied</Text> : null}
                    </Pressable>
                  );
                }
              })()
            ) : detail.kind === 'json' ? (
              (() => {
                // Render a formatted view derived from the raw JSON, then show raw JSON below it
                let formatted: React.ReactNode | null = null
                try {
                  const envelope: any = JSON.parse(detail.text)
                  const evt: any = envelope?.msg ?? envelope
                  if (evt?.type === 'thread.started' && typeof evt?.thread_id === 'string') {
                    formatted = <ThreadStartedRow threadId={evt.thread_id} />
                  } else if (evt?.type === 'error' && typeof evt?.message === 'string') {
                    formatted = <ErrorRow message={evt.message} />
                  } else if (evt?.type === 'turn.started' || evt?.type === 'turn.completed' || evt?.type === 'turn.failed') {
                    const phase = evt?.type?.split('.')?.[1] ?? 'started'
                    formatted = <TurnEventRow phase={phase} usage={evt.usage} message={evt.message} showUsage={true} />
                  } else if (typeof evt?.type === 'string' && evt.type.startsWith('exec_command_')) {
                    if (evt?.type === 'exec_command_begin') {
                      const payload = { command: Array.isArray(evt?.command) ? evt.command : (evt?.command ?? ''), cwd: typeof evt?.cwd === 'string' ? evt.cwd : undefined, parsed: evt?.parsed_cmd }
                      formatted = <ExecBeginRow payload={payload} full={true} />
                    }
                    // other exec_* variants are summarized elsewhere; keep raw below
                  } else if (typeof evt?.type === 'string' && evt.type.startsWith('item.')) {
                    const item: any = evt?.item ?? {}
                    const t = item?.type
                    if (t === 'command_execution') {
                      const command = String(item?.command ?? '')
                      const out: string = typeof item?.aggregated_output === 'string' ? item.aggregated_output : ''
                      const exit_code = typeof item?.exit_code === 'number' ? item.exit_code : null
                      const status = item?.status ?? (evt?.type?.split('.')?.[1] ?? undefined)
                      formatted = (
                        <CommandExecutionCard command={command} status={status} exitCode={exit_code} sample={out} outputLen={out.length} showExitCode={true} showOutputLen={true} />
                      )
                    } else if (t === 'file_change' || Array.isArray(item?.changes)) {
                      const status = item?.status ?? (evt?.type?.split('.')?.[1] ?? undefined)
                      const changes = Array.isArray(item?.changes) ? item.changes : []
                      formatted = <FileChangeCard changes={changes} status={status} limit={null} />
                    } else if (t === 'web_search') {
                      const query = String(item?.query ?? '')
                      formatted = <WebSearchRow query={query} />
                    } else if (t === 'mcp_tool_call') {
                      const server = String(item?.server ?? '')
                      const tool = String(item?.tool ?? '')
                      const status = item?.status ?? (evt?.type?.split('.')?.[1] ?? undefined)
                      formatted = <McpToolCallRow server={server} tool={tool} status={status} />
                    } else if (t === 'todo_list') {
                      const status = evt?.type?.split('.')?.[1] ?? undefined
                      const items = Array.isArray(item?.items) ? item.items.map((it: any) => ({ text: String(it?.text ?? ''), completed: Boolean(it?.completed) })) : []
                      formatted = <TodoListCard items={items} status={status} />
                    } else if (t === 'agent_message' && typeof item?.text === 'string') {
                      formatted = <MarkdownBlock markdown={item.text} />
                    } else if (t === 'reasoning' && typeof item?.text === 'string') {
                      formatted = <ReasoningCard item={{ type: 'reasoning', text: item.text }} />
                    } else {
                      const phase = evt?.type?.split('.')?.[1] ?? 'updated'
                      const id = String(item?.id ?? '')
                      const itemType = String(t ?? 'item')
                      const status = typeof item?.status === 'string' ? item.status : undefined
                      formatted = <ItemLifecycleRow phase={phase as any} id={id} itemType={itemType} status={status} />
                    }
                  }
                } catch {}
                return (
                  <View style={{ gap: 8 }}>
                    {formatted}
                    <View>
                      <Text style={{ color: Colors.secondary, fontFamily: Typography.bold, fontSize: 12, marginBottom: 4 }}>Raw JSON</Text>
                      <Pressable onLongPress={() => copy(detail.text)}>
                        <CodeBlock code={detail.text} language="json" />
                        {copied ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  </View>
                )
              })()
            ) : (
              <Pressable onLongPress={() => copy(isUserMsg ? cleanBody : (detail?.text ?? ''))}>
                <Text selectable={!isUserMsg} style={{ color: Colors.foreground, fontFamily: Typography.primary, lineHeight: 18 }}>{detail?.text}</Text>
                {copied ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12, marginTop: 4 }}>Copied</Text> : null}
              </Pressable>
            )}
          </View>
        )}
      </ScrollView>
    </View>
  );
}
