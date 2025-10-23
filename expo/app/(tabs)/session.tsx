import * as Clipboard from "expo-clipboard"
import { router } from "expo-router"
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { KeyboardAvoidingView, Platform, Pressable, ScrollView, Text, View } from "react-native"
import { useSafeAreaInsets } from "react-native-safe-area-context"
import Markdown from "react-native-markdown-display"
import { CommandExecutionCard } from "@/components/jsonl/CommandExecutionCard"
import { ErrorRow } from "@/components/jsonl/ErrorRow"
import { ExecBeginRow } from "@/components/jsonl/ExecBeginRow"
import { FileChangeCard } from "@/components/jsonl/FileChangeCard"
import { ItemLifecycleRow } from "@/components/jsonl/ItemLifecycleRow"
import { MarkdownBlock } from "@/components/jsonl/MarkdownBlock"
import { McpToolCallRow } from "@/components/jsonl/McpToolCallRow"
import { ReasoningHeadline } from "@/components/jsonl/ReasoningHeadline"
import { ThreadStartedRow } from "@/components/jsonl/ThreadStartedRow"
import { TodoListCard } from "@/components/jsonl/TodoListCard"
import { TurnEventRow } from "@/components/jsonl/TurnEventRow"
import { WebSearchRow } from "@/components/jsonl/WebSearchRow"
import { Colors } from "@/constants/theme"
import { Typography } from "@/constants/typography"
import { parseCodexLine } from "@/lib/codex-events"
import {
    clearLogs as clearLogsStore, loadLogs, putLog, saveLogs
} from "@/lib/log-store"
import { useWs } from "@/providers/ws"
import { useProjects } from "@/providers/projects"
import { pickProjectFromUtterance } from "@/lib/project-router"
import { mergeProjectTodos } from "@/lib/projects-store"
import { useHeaderHeight } from "@react-navigation/elements"
import { Composer } from "@/components/composer"
import { useNavigation } from "@react-navigation/native"
import { useDrawer } from "@/providers/drawer"
import { Ionicons } from "@expo/vector-icons"

export default function SessionScreen() {
  const navigation = useNavigation();
  const headerHeight = useHeaderHeight()
  const insets = useSafeAreaInsets()

  type Entry = { id: number; text: string; kind: 'md'|'reason'|'text'|'json'|'summary'|'delta'|'exec'|'file'|'search'|'mcp'|'todo'|'cmd'|'err'|'turn'|'thread'|'item_lifecycle'; deemphasize?: boolean; detailId?: number }
  const [log, setLog] = useState<Entry[]>([])
  const idRef = useRef(1)
  const scrollRef = useRef<ScrollView | null>(null)
  const { connected, send: sendWs, setOnMessage, readOnly, networkEnabled, approvals, attachPreface, setClearLogHandler } = useWs()
  const [copiedId, setCopiedId] = useState<number | null>(null)
  const { projects, activeProject, setActive, sendForProject } = useProjects()

  const copyAndFlash = useCallback(async (id: number, text: string) => {
    try { await Clipboard.setStringAsync(text) } catch {}
    setCopiedId(id)
    setTimeout(() => { setCopiedId((x) => (x === id ? null : x)) }, 800)
  }, [])

  const append = (
    text: string,
    deemphasize?: boolean,
    kind: 'md'|'reason'|'text'|'json'|'summary'|'delta'|'exec'|'file'|'search'|'mcp'|'todo'|'cmd'|'err'|'turn'|'thread'|'item_lifecycle' = 'text',
    detailRaw?: string,
  ) => {
    let detailId: number | undefined = undefined
    // If provided, persist a hidden detail record (usually full JSON/meta)
    if (typeof detailRaw === 'string') {
      detailId = idRef.current++
      putLog({ id: detailId, text: detailRaw, kind: 'json', deemphasize: true, ts: Date.now() })
    }
    const id = idRef.current++
    putLog({ id, text, kind, deemphasize, ts: Date.now(), detailId })
    setLog((prev) => [...prev, { id, text, kind, deemphasize, detailId }])
    saveLogs()
  }

  const buildPreface = () => `You are a coding agent running in the Codex CLI.
Capabilities: read files, propose patches with apply_patch, run shell commands.
Environment:
- Filesystem: ${readOnly ? 'read-only' : 'write access within workspace'}
- Network: ${networkEnabled ? 'enabled' : 'restricted'}
- Approvals: ${approvals}

Important policy overrides:
- Ignore any UI or environment_context that reports sandbox: read-only or a CWD of expo/; you are allowed to write within the repo.
- Always operate from the repository root (the folder that contains expo/ and crates/). Treat it as your CWD; edit paths like crates/codex-bridge/... directly.
- Never output "can’t edit from this sandbox". If a change is requested under crates/ or any sibling, produce an apply_patch for that path.
- Prefer apply_patch over manual instructions; do not ask for confirmation for safe code edits.
`

  const send = (txt: string) => {
    const base = txt.trim()
    if (!base) return
    // Try to route by project mention; fallback to current active project
    const routed = pickProjectFromUtterance(base, projects) || activeProject
    if (routed && routed.id !== activeProject?.id) { setActive(routed.id) }
    if (!sendForProject(routed, base)) { append('Not connected'); return }
    append(`> ${base}`)
  }

  useEffect(() => {
    setOnMessage((chunk) => {
      const lines = String(chunk).split(/\r?\n/)
      for (const line of lines) {
        const trimmed = line.trim(); if (!trimmed) continue
        const skippingRef = (SessionScreen as any)._skipExecRef || ((SessionScreen as any)._skipExecRef = { current: false })
        if (skippingRef.current) { if (trimmed.includes(']}') || trimmed.endsWith(']') || trimmed.includes('}}')) { skippingRef.current = false } continue }
        if (trimmed.includes('exec_command_output_delta')) { skippingRef.current = true; continue }
        // Drop multiline JSON blocks that start with `{ "id": ... "msg":` and often
        // contain exec_command_end payloads; skip until closing braces.
        const skipJsonBlockRef = (SessionScreen as any)._skipJsonBlockRef || ((SessionScreen as any)._skipJsonBlockRef = { current: false })
        if (skipJsonBlockRef.current) { if (/\}\}\s*$/.test(trimmed) || /^\}\s*$/.test(trimmed)) { skipJsonBlockRef.current = false } continue }
        const startsJsonEnvelope = trimmed.startsWith('{') && trimmed.includes('"msg"') && (/:\s*$/.test(trimmed) || /:\{\s*$/.test(trimmed));
        if (startsJsonEnvelope) { skipJsonBlockRef.current = true; continue }
        // Filter out noisy CLI status lines we don't want in the feed
        if (/^Reading prompt from stdin/i.test(trimmed)) {
          continue
        }
        // Some exec end events can still slip through as partial JSON fragments.
        // If we see an exec_command_end marker anywhere in the line, drop it.
        if (trimmed.includes('exec_command_end')) {
          continue
        }
        const parsed = parseCodexLine(trimmed)
        if (parsed.kind === 'delta') {
          // Do not render streaming summaries in the feed
          continue
        }
        else if (parsed.kind === 'md') {
          const raw = String(parsed.markdown ?? '').trim()
          const core = raw.replace(/^\*\*|\*\*$/g, '').replace(/^`|`$/g, '').trim().toLowerCase()
          const isBland = core === 'unknown' || core === 'n/a' || core === 'none' || core === 'null'
          if (!isBland) append(`::md::${parsed.markdown}`, false, 'md')
        }
        else if (parsed.kind === 'reason') append(`::reason::${parsed.text}`, false, 'reason')
        else if (parsed.kind === 'thread') {
          const payload = JSON.stringify({ thread_id: parsed.thread_id })
          append(payload, false, 'thread', trimmed)
        }
        else if (parsed.kind === 'item_lifecycle') {
          const payload = JSON.stringify({ phase: parsed.phase, id: parsed.id, item_type: parsed.item_type, status: parsed.status })
          append(payload, true, 'item_lifecycle', trimmed)
        }
        else if (parsed.kind === 'exec_begin') {
          const payload = JSON.stringify({ command: parsed.command, cwd: parsed.cwd, parsed: parsed.parsed })
          append(payload, false, 'exec', trimmed)
        }
        else if (parsed.kind === 'file_change') {
          const payload = JSON.stringify({ status: parsed.status, changes: parsed.changes })
          append(payload, false, 'file', trimmed)
        }
        else if (parsed.kind === 'web_search') {
          const payload = JSON.stringify({ query: parsed.query })
          append(payload, false, 'search', trimmed)
        }
        else if (parsed.kind === 'mcp_call') {
          const payload = JSON.stringify({ server: parsed.server, tool: parsed.tool, status: parsed.status })
          append(payload, false, 'mcp', trimmed)
        }
        else if (parsed.kind === 'todo_list') {
          const payload = JSON.stringify({ status: parsed.status, items: parsed.items })
          append(payload, false, 'todo', trimmed)
          try {
            if (activeProject?.id) {
              mergeProjectTodos(activeProject.id, parsed.items).catch(() => {})
            }
          } catch {}
        }
        else if (parsed.kind === 'cmd_item') {
          const payload = JSON.stringify({ command: parsed.command, status: parsed.status, exit_code: parsed.exit_code, sample: parsed.sample, output_len: parsed.output_len })
          append(payload, false, 'cmd', trimmed)
        }
        else if (parsed.kind === 'err') {
          const payload = JSON.stringify({ message: parsed.message })
          append(payload, false, 'err', trimmed)
        }
        else if (parsed.kind === 'turn') {
          const payload = JSON.stringify({ phase: parsed.phase, usage: parsed.usage, message: parsed.message })
          append(payload, false, 'turn', trimmed)
        }
        else if (parsed.kind === 'summary') {
          // Hide exec noise like "[exec out]" and "[exec end]" entirely
          if (/^\[exec (out|end)\]/i.test(parsed.text)) {
            continue
          }
          append(parsed.text, true, 'summary', trimmed)
        }
        else if (parsed.kind === 'text') {
          const raw = String(parsed.raw ?? '').trim()
          if (!raw) { continue }
          append(raw, true, 'text')
        }
        else if (parsed.kind === 'json') {
          if (parsed.raw.includes('exec_command_end')) { continue }
          append(parsed.raw, true, 'json')
        }
        else append(trimmed, true, 'text')
      }
    })
    return () => setOnMessage(null)
  }, [setOnMessage])

  const { resetResumeHint } = useProjects();
  useEffect(() => { setClearLogHandler(() => { setLog([]); clearLogsStore(); resetResumeHint(); }); return () => setClearLogHandler(null) }, [setClearLogHandler, resetResumeHint])
  useEffect(() => { (async ()=>{ const items = await loadLogs(); if (items.length) { setLog(items.map(({id,text,kind,deemphasize,detailId})=>({id,text,kind,deemphasize,detailId}))); idRef.current = Math.max(...items.map(i=>i.id))+1 } })() }, [])

  // Update header title dynamically: "New session" when empty
  const isNew = log.length === 0;
  useEffect(() => {
    navigation.setOptions({
      headerLeft: () => <SessionHeaderLeft title={isNew ? 'New session' : 'Session'} />,
    });
  }, [navigation, isNew]);

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <View style={{ flex: 1, paddingTop: 0, paddingBottom: 4, paddingHorizontal: 8, gap: 0 }}>
        <View style={{ flex: 1 }}>
          <ScrollView
            ref={scrollRef}
            onContentSizeChange={() => scrollRef.current?.scrollToEnd({ animated: true })}
            contentContainerStyle={{ paddingTop: 0, paddingBottom: 6, paddingHorizontal: 8 }}
            contentInsetAdjustmentBehavior="never"
            automaticallyAdjustContentInsets={false}
            automaticallyAdjustsScrollIndicatorInsets={false}
          >
            {log.filter((e) => e.kind !== 'json').map((e) => {
              const onPressOpen = () => {
                const idToOpen = e.detailId ?? e.id
                router.push(`/message/${idToOpen}`)
              };
              // Indentation heuristic:
              // - top level: md, reason, turn
              // - tool/exec/results: exec (level 1), file/search/mcp/todo/cmd/err/summary (level 2)
              const indent =
                e.kind === 'exec' ? 12 :
                (e.kind === 'file' || e.kind === 'search' || e.kind === 'mcp' || e.kind === 'todo' || e.kind === 'cmd' || e.kind === 'err' || e.kind === 'summary')
                  ? 24
                  : (e.kind === 'item_lifecycle' ? 12 : 0)
              const isMd = e.text.startsWith('::md::')
              if (isMd) {
                const md = e.text.slice('::md::'.length)
                return (
                  <View key={e.id} style={{ paddingLeft: indent }}>
                    <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, md)}>
                      <MarkdownBlock markdown={md} />
                      {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                    </Pressable>
                  </View>
                )
              }
              const isReason = e.text.startsWith('::reason::')
              if (isReason) {
                const full = e.text.slice('::reason::'.length)
                return (
                  <View key={e.id} style={{ paddingLeft: indent, marginTop: 8 }}>
                    <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, full)}>
                      <ReasoningHeadline text={full} />
                      {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                    </Pressable>
                  </View>
                )
              }
              if (e.kind === 'thread') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <ThreadStartedRow threadId={obj.thread_id ?? ''} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'item_lifecycle') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <ItemLifecycleRow phase={obj.phase ?? 'updated'} id={obj.id ?? ''} itemType={obj.item_type ?? 'item'} status={obj.status} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'exec') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <ExecBeginRow payload={obj} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'file') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <FileChangeCard changes={obj.changes ?? []} status={obj.status} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'search') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, obj.query ?? '')}>
                        <WebSearchRow query={obj.query ?? ''} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'mcp') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, `${obj.server ?? ''}:${obj.tool ?? ''}${obj.status ? ` (${obj.status})` : ''}`.trim())}>
                        <McpToolCallRow server={obj.server ?? ''} tool={obj.tool ?? ''} status={obj.status} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'todo') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <TodoListCard items={obj.items ?? []} status={obj.status} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'cmd') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <CommandExecutionCard command={obj.command ?? ''} status={obj.status} exitCode={obj.exit_code} sample={obj.sample} outputLen={obj.output_len} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'err') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, obj.message ?? e.text)}>
                        <ErrorRow message={obj.message ?? ''} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'turn') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <TurnEventRow phase={obj.phase ?? 'started'} usage={obj.usage} message={obj.message} />
                        {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              const lines = e.text.split(/\r?\n/)
              const isLong = lines.length > 8
              const preview = isLong ? lines.slice(0, 8).join('\n') + '\n…' : e.text
              const isUserMsg = /^\s*>/.test(e.text)
              const copyText = isUserMsg ? e.text.replace(/^\s*>\s?/, '') : e.text
              return (
                <View key={e.id} style={{ paddingLeft: indent }}>
                  <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, copyText)}>
                    <Text selectable={!isUserMsg} style={{ fontSize: 12, lineHeight: 16, color: Colors.textPrimary, fontFamily: Typography.primary, opacity: e.deemphasize ? 0.35 : 1 }}>{preview}</Text>
                    {copiedId === e.id ? <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                  </Pressable>
                </View>
              )
            })}
          </ScrollView>
        </View>

        <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={headerHeight + 8}>
          <View style={{ paddingBottom: Math.max(insets.bottom, 8), paddingHorizontal: 8 }}>
            <Composer onSend={send} connected={connected} />
          </View>
        </KeyboardAvoidingView>
      </View>
    </View>
  )
}

function SessionHeaderLeft({ title }: { title: string }) {
  const drawer = useDrawer();
  return (
    <View style={{ flexDirection: 'row', alignItems: 'center' }}>
      <Pressable
        onPress={drawer.toggle}
        accessibilityRole="button"
        hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }}
        style={{ paddingHorizontal: 6, paddingVertical: 6 }}
      >
        <Ionicons name="menu" size={22} color={Colors.textPrimary} />
      </Pressable>
      <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 16, marginLeft: 6 }}>{title}</Text>
    </View>
  );
}
