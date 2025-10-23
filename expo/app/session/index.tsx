import * as Clipboard from "expo-clipboard"
import { router, useLocalSearchParams } from "expo-router"
import React, { useCallback, useEffect, useRef, useState } from "react"
import { KeyboardAvoidingView, Platform, Pressable, ScrollView, Text, View, TextInput, Keyboard } from "react-native"
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
    clearLogs as clearLogsStore, loadLogs, putLog, saveLogs, getLog
} from "@/lib/log-store"
import { useBridge } from "@/providers/ws"
import { useProjects } from "@/providers/projects"
import { pickProjectFromUtterance } from "@/lib/project-router"
import { mergeProjectTodos } from "@/lib/projects-store"
import { Composer } from "@/components/composer"
import { useHeaderStore, useHeaderTitle, useHeaderSubtitle } from "@/lib/header-store"
import { useThreads } from "@/lib/threads-store"
import { useDrawer } from "@/providers/drawer"
import { Ionicons } from "@expo/vector-icons"

export default function SessionScreen() {
  const params = useLocalSearchParams<{ focus?: string }>()
  const headerHeight = useHeaderStore((s) => s.height)
  const setHeaderTitle = useHeaderStore((s) => s.setTitle)
  const insets = useSafeAreaInsets()
  const drawer = useDrawer();

  type Entry = { id: number; text: string; kind: 'md'|'reason'|'text'|'json'|'summary'|'delta'|'exec'|'file'|'search'|'mcp'|'todo'|'cmd'|'err'|'turn'|'thread'|'item_lifecycle'; deemphasize?: boolean; detailId?: number }
  const [log, setLog] = useState<Entry[]>([])
  const idRef = useRef(1)
  const scrollRef = useRef<ScrollView | null>(null)
  const shouldAutoScrollRef = useRef(true)
  const lastLengthRef = useRef(0)
  const lastContentHeightRef = useRef(0)
  const { connected, send: sendWs, setOnMessage, readOnly, networkEnabled, approvals, attachPreface, setClearLogHandler, resumeNextId } = useBridge()
  const [copiedId, setCopiedId] = useState<number | null>(null)
  const [isRunning, setIsRunning] = useState(false)
  const [queuedFollowUps, setQueuedFollowUps] = useState<string[]>([])
  const [composerPrefill, setComposerPrefill] = useState<string | null | undefined>(undefined)
  const flushingFollowUpRef = useRef(false)
  const queueRef = useRef<string[]>([])
  const composerDraftRef = useRef('')
  const composerInputRef = useRef<TextInput | null>(null)
  const sendNowRef = useRef<(text: string) => boolean>(() => false)
  const { projects, activeProject, setActive, sendForProject } = useProjects()
  const setThreadProject = useThreads((s) => s.setThreadProject)
  const lastSentProjectIdRef = useRef<string | null>(null)
  const currentThreadIdRef = useRef<string | null>(null)
  const requireThreadStartRef = useRef<boolean>(true)
  // Working-status state: shows "Working: Ns" until first visible content arrives
  const [awaitingFirst, setAwaitingFirst] = useState(false)
  const awaitingFirstRef = useRef(false)
  const [workingStartedAt, setWorkingStartedAt] = useState<number | null>(null)
  const [workingSeconds, setWorkingSeconds] = useState(0)
  const [kbVisible, setKbVisible] = useState(false)

  // Increment the visible working timer while awaiting first assistant content
  useEffect(() => {
    if (!awaitingFirst || typeof workingStartedAt !== 'number') return
    const tick = () => setWorkingSeconds(Math.max(0, Math.floor((Date.now() - workingStartedAt) / 1000)))
    tick()
    const id = setInterval(tick, 1000)
    return () => clearInterval(id)
  }, [awaitingFirst, workingStartedAt])

  const latestCmdIdByCommand = React.useMemo(() => {
    const m = new Map<string, number>()
    for (const entry of log) {
      if (entry.kind !== 'cmd') continue
      try {
        const obj = JSON.parse(entry.text)
        const cmd = String(obj.command ?? '')
        if (cmd) m.set(cmd, entry.id)
      } catch {}
    }
    return m
  }, [log])

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

  sendNowRef.current = (txt: string): boolean => {
    const base = txt.trim()
    if (!base) return false
    const routed = pickProjectFromUtterance(base, projects) || activeProject
    if (routed && routed.id !== activeProject?.id) { setActive(routed.id) }
    // Remember which project this send belongs to (for mapping when thread starts)
    lastSentProjectIdRef.current = routed?.id ?? activeProject?.id ?? null
    if (!sendForProject(routed, base)) { append('Not connected'); return false }
    append(`> ${base}`)
    setIsRunning(true)
    setAwaitingFirst(true); awaitingFirstRef.current = true
    setWorkingStartedAt(Date.now()); setWorkingSeconds(0)
    // Update header subtitle to the active project name, if any
    try { if (routed?.name) { useHeaderStore.getState().setSubtitle(routed.name) } else if (activeProject?.name) { useHeaderStore.getState().setSubtitle(activeProject.name) } else { useHeaderStore.getState().setSubtitle('') } } catch {}
    return true
  }

  const queueFollowUp = useCallback((message: string) => {
    const base = message.trim()
    if (!base) return
    queueRef.current = [...queueRef.current, base]
    setQueuedFollowUps(queueRef.current.slice())
  }, [setQueuedFollowUps])

  const handleSend = useCallback((txt: string) => {
    const base = txt.trim()
    if (!base) return
    if (isRunning) { queueFollowUp(base); return }
    if (!sendNowRef.current(base)) { queueFollowUp(base) }
  }, [isRunning, queueFollowUp])

  const flushQueuedFollowUp = useCallback(() => {
    if (flushingFollowUpRef.current) return
    if (queueRef.current.length === 0) return
    flushingFollowUpRef.current = true
    const next = queueRef.current.shift()!
    setQueuedFollowUps(queueRef.current.slice())
    const sent = sendNowRef.current(next)
    if (!sent) { queueRef.current.unshift(next); setQueuedFollowUps(queueRef.current.slice()) }
    flushingFollowUpRef.current = false
  }, [setQueuedFollowUps])

  const handleDraftChange = useCallback((value: string) => { composerDraftRef.current = value }, [])

  const handleInterrupt = useCallback(() => {
    const payload = JSON.stringify({ control: 'interrupt' })
    if (!sendWs(payload)) { append('Interrupt failed: not connected') } else { append('> [interrupt] requested', true, 'text') }
  }, [sendWs, append])

  useEffect(() => { if (composerPrefill !== undefined) { const timer = setTimeout(() => setComposerPrefill(undefined), 0); return () => clearTimeout(timer) } }, [composerPrefill])

  useEffect(() => {
    setOnMessage((chunk) => {
      const lines = String(chunk).split(/\r?\n/)
      for (const line of lines) {
        const trimmed = line.trim(); if (!trimmed) continue
        const skippingRef = (SessionScreen as any)._skipExecRef || ((SessionScreen as any)._skipExecRef = { current: false })
        if (skippingRef.current) { if (trimmed.includes(']}') || trimmed.endsWith(']') || trimmed.includes('}}')) { skippingRef.current = false } continue }
        if (trimmed.includes('exec_command_output_delta')) { skippingRef.current = true; continue }
        const skipJsonBlockRef = (SessionScreen as any)._skipJsonBlockRef || ((SessionScreen as any)._skipJsonBlockRef = { current: false })
        if (skipJsonBlockRef.current) { if (/\}\}\s*$/.test(trimmed) || /^\}\s*$/.test(trimmed)) { skipJsonBlockRef.current = false } continue }
        const startsJsonEnvelope = trimmed.startsWith('{') && trimmed.includes('"msg"') && (/:\s*$/.test(trimmed) || /:\{\s*$/.test(trimmed));
        if (startsJsonEnvelope) { skipJsonBlockRef.current = true; continue }
        const low = trimmed.toLowerCase()
        if (low.includes('reading prompt from stdin') || low === 'no prompt provided via stdin.') { continue }
        if (trimmed.includes('exec_command_end')) { continue }
        const parsed = parseCodexLine(trimmed)
        // If we are waiting for a fresh thread.started (e.g., after New Thread),
        // ignore all incoming events until it arrives to avoid mixing threads.
        if (requireThreadStartRef.current && parsed.kind !== 'thread') {
          continue
        }
        if (parsed.kind === 'delta') { continue }
        else if (parsed.kind === 'md') { if (awaitingFirstRef.current) { setAwaitingFirst(false); awaitingFirstRef.current = false } const raw = String(parsed.markdown ?? '').trim(); const core = raw.replace(/^\*\*|\*\*$/g, '').replace(/^`|`$/g, '').trim().toLowerCase(); const isBland = core === 'unknown' || core === 'n/a' || core === 'none' || core === 'null'; if (!isBland) append(`::md::${parsed.markdown}`, false, 'md') }
        else if (parsed.kind === 'reason') { if (awaitingFirstRef.current) { setAwaitingFirst(false); awaitingFirstRef.current = false } append(`::reason::${parsed.text}`, false, 'reason') }
        else if (parsed.kind === 'thread') {
          try {
            const short = String(parsed.thread_id ?? '').split('-')[0] || ''
            if (short) setHeaderTitle(`Thread ${short}`)
            // Thread boundary handling
            const incoming = String(parsed.thread_id || '')
            const current = currentThreadIdRef.current
            if (current && incoming && current !== incoming) {
              // New thread started while we had content — reset feed to avoid mixing
              setLog([])
              idRef.current = 1
            }
            currentThreadIdRef.current = incoming || null
            requireThreadStartRef.current = false
            // Capture mapping thread_id -> projectId used for this send (if any)
            const pid = lastSentProjectIdRef.current
            if (pid && parsed.thread_id) { setThreadProject(parsed.thread_id, pid) }
          } catch {}
          continue
        }
        else if (parsed.kind === 'item_lifecycle') { const payload = JSON.stringify({ phase: parsed.phase, id: parsed.id, item_type: parsed.item_type, status: parsed.status }); append(payload, true, 'item_lifecycle', trimmed) }
        else if (parsed.kind === 'exec_begin') { const payload = JSON.stringify({ command: parsed.command, cwd: parsed.cwd, parsed: parsed.parsed }); append(payload, false, 'exec', trimmed) }
        else if (parsed.kind === 'file_change') { const payload = JSON.stringify({ status: parsed.status, changes: parsed.changes }); append(payload, false, 'file', trimmed) }
        else if (parsed.kind === 'web_search') { const payload = JSON.stringify({ query: parsed.query }); append(payload, false, 'search', trimmed) }
        else if (parsed.kind === 'mcp_call') { const payload = JSON.stringify({ server: parsed.server, tool: parsed.tool, status: parsed.status }); append(payload, false, 'mcp', trimmed) }
        else if (parsed.kind === 'todo_list') { const payload = JSON.stringify({ status: parsed.status, items: parsed.items }); append(payload, false, 'todo', trimmed); try { if (activeProject?.id) { mergeProjectTodos(activeProject.id, parsed.items).catch(() => {}) } } catch {} }
        else if (parsed.kind === 'cmd_item') { const payload = JSON.stringify({ command: parsed.command, status: parsed.status, exit_code: parsed.exit_code, sample: parsed.sample, output_len: parsed.output_len }); append(payload, false, 'cmd', trimmed) }
        else if (parsed.kind === 'err') { const payload = JSON.stringify({ message: parsed.message }); append(payload, false, 'err', trimmed) }
        else if (parsed.kind === 'turn') {
          if (parsed.phase === 'started') { 
            setIsRunning(true);
            setAwaitingFirst(true); awaitingFirstRef.current = true;
            setWorkingStartedAt(Date.now()); setWorkingSeconds(0);
            continue 
          }
          else if (parsed.phase === 'completed' || parsed.phase === 'failed') {
            setIsRunning(false);
            if (awaitingFirstRef.current) { setAwaitingFirst(false); awaitingFirstRef.current = false }
            const message = typeof parsed.message === 'string' ? parsed.message : '';
            const wasInterrupted = parsed.phase === 'failed' && message.toLowerCase().includes('interrupt');
            if (wasInterrupted) {
              if (queueRef.current.length > 0) {
                const existingDraft = composerDraftRef.current;
                const parts = queueRef.current.slice();
                if (existingDraft.trim().length > 0) { parts.push(existingDraft) }
                const newDraft = parts.join('\n');
                queueRef.current = [];
                setQueuedFollowUps([]);
                if (newDraft.trim().length > 0) { setComposerPrefill(newDraft); composerDraftRef.current = newDraft }
              }
            } else { flushQueuedFollowUp() }
          }
          const payload = JSON.stringify({ phase: parsed.phase, usage: parsed.usage, message: parsed.message });
          append(payload, false, 'turn', trimmed)
        }
        else if (parsed.kind === 'summary') { if (/^\[exec (out|end)\]/i.test(parsed.text)) { continue } if (awaitingFirstRef.current) { setAwaitingFirst(false); awaitingFirstRef.current = false } append(parsed.text, true, 'summary', trimmed) }
        else if (parsed.kind === 'text') { const raw = String(parsed.raw ?? '').trim(); if (!raw) { continue } if (awaitingFirstRef.current) { setAwaitingFirst(false); awaitingFirstRef.current = false } append(raw, true, 'text') }
        else if (parsed.kind === 'json') { if (parsed.raw.includes('exec_command_end')) { continue } append(parsed.raw, true, 'json') }
        else append(trimmed, true, 'text')
      }
    })
    return () => setOnMessage(null)
  }, [setOnMessage])

  useEffect(() => {
    setClearLogHandler(() => {
      setLog([])
      setQueuedFollowUps([])
      setIsRunning(false)
      flushingFollowUpRef.current = false
      queueRef.current = []
      composerDraftRef.current = ''
      setComposerPrefill(undefined)
      clearLogsStore()
      // Ensure we don't accept stray events until the next thread starts
      currentThreadIdRef.current = null
      requireThreadStartRef.current = true
      try { setTimeout(() => composerInputRef.current?.focus(), 0) } catch {}
    })
    return () => setClearLogHandler(null)
  }, [setClearLogHandler])

  // Extra safety: if a New Thread action set resumeNextId='new', proactively clear
  useEffect(() => {
    if (resumeNextId === 'new') {
      setLog([])
      setQueuedFollowUps([])
      setIsRunning(false)
      flushingFollowUpRef.current = false
      queueRef.current = []
      composerDraftRef.current = ''
      setComposerPrefill(undefined)
      clearLogsStore()
      currentThreadIdRef.current = null
      requireThreadStartRef.current = true
    }
  }, [resumeNextId])

  // Track keyboard visibility to toggle bottom safe area padding
  useEffect(() => {
    const show = Keyboard.addListener(Platform.OS === 'ios' ? 'keyboardWillShow' : 'keyboardDidShow', () => setKbVisible(true))
    const hide = Keyboard.addListener(Platform.OS === 'ios' ? 'keyboardWillHide' : 'keyboardDidHide', () => setKbVisible(false))
    return () => { try { show.remove(); hide.remove() } catch {} }
  }, [])
  useEffect(() => {
    if (isRunning || !connected) { flushingFollowUpRef.current = false; return }
    if (queueRef.current.length === 0) { return }
    flushQueuedFollowUp()
  }, [isRunning, connected, queuedFollowUps, flushQueuedFollowUp])
  useEffect(() => { (async ()=>{ const items = await loadLogs(); if (items.length) { setLog(items.map(({id,text,kind,deemphasize,detailId})=>({id,text,kind,deemphasize,detailId}))); idRef.current = Math.max(...items.map(i=>i.id))+1 } })() }, [])

  useHeaderTitle('New Thread')
  useHeaderSubtitle(activeProject?.name ?? '')
  useEffect(() => {
    if (params?.focus) {
      const t = setTimeout(() => composerInputRef.current?.focus(), 0)
      return () => clearTimeout(t)
    }
  }, [params?.focus])

  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <View style={{ flex: 1, paddingTop: 0, paddingBottom: 4, paddingHorizontal: 8, gap: 0 }}>
        <View style={{ flex: 1 }}>
          <ScrollView
            ref={scrollRef}
            onScroll={(e) => {
              try {
                const { contentOffset, layoutMeasurement, contentSize } = e.nativeEvent
                const atBottom = contentOffset.y + layoutMeasurement.height >= contentSize.height - 24
                shouldAutoScrollRef.current = atBottom
              } catch {}
            }}
            scrollEventThrottle={32}
            onContentSizeChange={(_, h) => {
              const len = log.length
              const appended = len > lastLengthRef.current
              const grew = h > lastContentHeightRef.current + 4 // ignore tiny jitter
              lastLengthRef.current = len
              lastContentHeightRef.current = h
              if ((appended || grew) && shouldAutoScrollRef.current) {
                try { scrollRef.current?.scrollToEnd({ animated: true }) } catch {}
              }
            }}
            contentContainerStyle={{ paddingTop: 0, paddingBottom: 6, paddingHorizontal: 8 }}
            contentInsetAdjustmentBehavior="never"
            automaticallyAdjustContentInsets={false}
            automaticallyAdjustsScrollIndicatorInsets={false}
          >
            {log.filter((e) => e.kind !== 'json').map((e, idx, arr) => {
              const onPressOpen = () => { const idToOpen = e.detailId ?? e.id; router.push(`/message/${idToOpen}`) };
              // Remove left padding so rows are flush with container margins
              const indent = 0
              const isMd = e.text.startsWith('::md::')
              if (isMd) {
                const md = e.text.slice('::md::'.length)
                return (
                  <View key={e.id} style={{ paddingLeft: indent }}>
                    <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, md)}>
                      <MarkdownBlock markdown={md} />
                      {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
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
                      {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                    </Pressable>
                  </View>
                )
              }
              if (e.kind === 'thread') { return null }
              if (e.kind === 'item_lifecycle') {
                try {
                  const obj = JSON.parse(e.text)
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <ItemLifecycleRow phase={obj.phase ?? 'updated'} id={obj.id ?? ''} itemType={obj.item_type ?? 'item'} status={obj.status} />
                        {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
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
                        {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
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
                        {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
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
                        {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
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
                        {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
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
                        {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'cmd') {
                try {
                  const obj = JSON.parse(e.text)
                  const lastIdForCmd = latestCmdIdByCommand.get(String(obj.command ?? ''))
                  if (typeof lastIdForCmd === 'number' && lastIdForCmd !== e.id) { return null }
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <CommandExecutionCard
                          command={obj.command ?? ''}
                          status={obj.status}
                          exitCode={obj.exit_code}
                          sample={obj.sample}
                          outputLen={obj.output_len}
                          showExitCode={false}
                          collapsed={true}
                          maxBodyHeight={120}
                        />
                        {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
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
                        {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                      </Pressable>
                    </View>
                  )
                } catch {}
              }
              if (e.kind === 'turn') {
                try {
                  const obj = JSON.parse(e.text)
                  if (obj.phase === 'started') return null
                  let durationMs: number | undefined = undefined
                  if (obj.phase === 'completed' || obj.phase === 'failed') {
                    let startId: number | undefined = undefined
                    for (let k = idx - 1; k >= 0; k--) {
                      const it = arr[k]
                      if (it.kind !== 'turn') continue
                      try { const prev = JSON.parse(it.text); if (prev.phase === 'started') { startId = it.id; break } if (prev.phase === 'completed' || prev.phase === 'failed') { break } } catch {}
                    }
                    const endTs = getLog(e.id)?.ts
                    const startTs = startId ? getLog(startId)?.ts : undefined
                    if (typeof endTs === 'number' && typeof startTs === 'number') { durationMs = Math.max(0, endTs - startTs) }
                  }
                  return (
                    <View key={e.id} style={{ paddingLeft: indent }}>
                      <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, e.text)}>
                        <TurnEventRow phase={obj.phase ?? 'started'} usage={obj.usage} message={obj.message} showUsage={false} durationMs={durationMs} />
                        {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
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
              const isLastUserMsg = isUserMsg && (() => { for (let j = arr.length - 1; j >= 0; j--) { const it = arr[j]; if (it.kind === 'json') continue; if (/^\s*>/.test(it.text)) return it.id === e.id; } return false; })();
              return (
                <React.Fragment key={e.id}>
                  <View style={{ paddingLeft: indent }}>
                    <Pressable onPress={onPressOpen} onLongPress={() => copyAndFlash(e.id, copyText)}>
                      <Text selectable={!isUserMsg} style={{ fontSize: 12, lineHeight: 16, color: Colors.foreground, fontFamily: Typography.primary, opacity: e.deemphasize ? 0.35 : 1, marginTop: isUserMsg ? 6 : 0, marginBottom: isUserMsg ? 8 : 0 }}>{preview}</Text>
                      {copiedId === e.id ? <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 11, marginTop: 2 }}>Copied</Text> : null}
                    </Pressable>
                  </View>
                  {awaitingFirst && isLastUserMsg ? (
                    <View style={{ paddingLeft: 0, paddingVertical: 6, paddingHorizontal: 8 }} key={`working-${e.id}`}>
                      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>
                        Working: {workingSeconds}s
                      </Text>
                    </View>
                  ) : null}
                </React.Fragment>
              )
            })}
          </ScrollView>
        </View>

        <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={headerHeight + 4}>
          <View style={{ paddingBottom: kbVisible ? 6 : Math.max(insets.bottom, 8), paddingHorizontal: 8 }}>
            <Composer onSend={handleSend} connected={connected} isRunning={isRunning} onQueue={(m)=>{}} onInterrupt={handleInterrupt} queuedMessages={queuedFollowUps} prefill={composerPrefill} onDraftChange={handleDraftChange} inputRef={composerInputRef} />
          </View>
        </KeyboardAvoidingView>
      </View>
    </View>
  )
}
