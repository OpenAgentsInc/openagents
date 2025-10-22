import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { KeyboardAvoidingView, Platform, Pressable, SafeAreaView, ScrollView, Text, TextInput, View } from 'react-native'
import Markdown from 'react-native-markdown-display'
import { Typography } from '@/constants/typography'
import { Colors } from '@/constants/theme'
import { parseCodexLine } from '@/lib/codex-events'
import { useWs } from '@/providers/ws'
import { useHeaderHeight } from '@react-navigation/elements'
import { putLog, loadLogs, saveLogs, clearLogs as clearLogsStore } from '@/lib/log-store'

export default function SessionScreen() {
  const headerHeight = useHeaderHeight()
  const ui = useMemo(() => ({ button: '#3F3F46' }), [])

  const [prompt, setPrompt] = useState('')
  const LINE_HEIGHT = 18
  const MIN_LINES = 1
  const MAX_LINES = 10
  const PADDING_V = 10
  const MIN_HEIGHT = LINE_HEIGHT * MIN_LINES + PADDING_V * 2
  const MAX_HEIGHT = LINE_HEIGHT * MAX_LINES + PADDING_V * 2
  const [inputHeight, setInputHeight] = useState(MIN_HEIGHT)
  const lastHeightRef = useRef(MIN_HEIGHT)
  const rafRef = useRef<number | null>(null)
  const [growActive, setGrowActive] = useState(() => !!prompt && (prompt.includes('\n') || prompt.length > 0))
  const setHeightStable = useCallback((target: number, { allowShrink = false }: { allowShrink?: boolean } = {}) => {
    const clamped = Math.max(MIN_HEIGHT, Math.min(MAX_HEIGHT, Math.round(target)))
    if (!allowShrink && clamped <= lastHeightRef.current) return
    if (rafRef.current) cancelAnimationFrame(rafRef.current as any)
    rafRef.current = requestAnimationFrame(() => { lastHeightRef.current = clamped; setInputHeight(clamped) })
  }, [])

  type Entry = { id: number; text: string; deemphasize?: boolean }
  const [log, setLog] = useState<Entry[]>([])
  const idRef = useRef(1)
  const scrollRef = useRef<ScrollView | null>(null)
  const { connected, send: sendWs, setOnMessage, readOnly, networkEnabled, approvals, attachPreface, setClearLogHandler } = useWs()

  const append = (text: string, deemphasize?: boolean, kind: 'md'|'reason'|'text'|'json'|'summary'|'delta' = 'text') => {
    const id = idRef.current++
    putLog({ id, text, kind, deemphasize, ts: Date.now() })
    setLog((prev) => [...prev, { id, text, deemphasize }])
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

  const send = () => {
    const base = prompt.trim()
    const finalText = attachPreface ? `${buildPreface()}\n\n${base}` : base
    const payload = finalText.endsWith('\n') ? finalText : finalText + '\n'
    if (!sendWs(payload)) { append('Not connected'); return }
    append(`> ${base}`)
    setPrompt('')
    lastHeightRef.current = MIN_HEIGHT; setInputHeight(MIN_HEIGHT); setGrowActive(false)
  }

  useEffect(() => {
    setOnMessage((chunk) => {
      const lines = String(chunk).split(/\r?\n/)
      for (const line of lines) {
        const trimmed = line.trim(); if (!trimmed) continue
        const skippingRef = (SessionScreen as any)._skipExecRef || ((SessionScreen as any)._skipExecRef = { current: false })
        if (skippingRef.current) { if (trimmed.includes(']}') || trimmed.endsWith(']') || trimmed.includes('}}')) { skippingRef.current = false } continue }
        if (trimmed.includes('exec_command_output_delta')) { skippingRef.current = true; continue }
        const parsed = parseCodexLine(trimmed)
        if (parsed.kind === 'delta') append(parsed.summary, true, 'summary')
        else if (parsed.kind === 'md') append(`::md::${parsed.markdown}`, false, 'md')
        else if (parsed.kind === 'reason') append(`::reason::${parsed.text}`, false, 'reason')
        else if (parsed.kind === 'summary') append(parsed.text, true, 'summary')
        else if (parsed.kind === 'json') append(parsed.raw, true, 'json')
        else append(parsed.raw, true, 'text')
      }
    })
    return () => setOnMessage(null)
  }, [setOnMessage])

  useEffect(() => { setClearLogHandler(() => { setLog([]); clearLogsStore(); }); return () => setClearLogHandler(null) }, [setClearLogHandler])
  useEffect(() => { (async ()=>{ const items = await loadLogs(); if (items.length) { setLog(items.map(({id,text,deemphasize})=>({id,text,deemphasize}))); idRef.current = Math.max(...items.map(i=>i.id))+1 } })() }, [])

  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: Colors.background }}>
      <View style={{ flex: 1, paddingTop: 0, paddingBottom: 14, paddingHorizontal: 8, gap: 14 }}>
        <View style={{ flex: 1 }}>
          <ScrollView ref={scrollRef} onContentSizeChange={() => scrollRef.current?.scrollToEnd({ animated: true })} contentContainerStyle={{ paddingTop: 0, paddingBottom: 6, paddingHorizontal: 8 }}>
            {log.map((e) => {
              const isMd = e.text.startsWith('::md::')
              if (isMd) {
                const md = e.text.slice('::md::'.length)
                return (
                  <Markdown key={e.id} style={{ body: { color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 13, lineHeight: 18 }, paragraph: { color: Colors.textPrimary }, code_inline: { backgroundColor: '#0F1217', color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 }, code_block: { backgroundColor: '#0F1217', color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, padding: 8 }, fence: { backgroundColor: '#0F1217', color: Colors.textPrimary, borderWidth: 1, borderColor: Colors.border, padding: 8 }, }}>
                    {md}
                  </Markdown>
                )
              }
              const isReason = e.text.startsWith('::reason::')
              if (isReason) {
                const md = e.text.slice('::reason::'.length)
                return (
                  <Markdown key={e.id} style={{ body: { color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12, lineHeight: 18 }, paragraph: { color: Colors.textSecondary }, code_inline: { backgroundColor: '#0F1217', color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, paddingHorizontal: 4, paddingVertical: 2 }, code_block: { backgroundColor: '#0F1217', color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, padding: 8 }, fence: { backgroundColor: '#0F1217', color: Colors.textSecondary, borderWidth: 1, borderColor: Colors.border, padding: 8 }, }}>
                    {md}
                  </Markdown>
                )
              }
              const lines = e.text.split(/\r?\n/)
              const isLong = lines.length > 8
              const preview = isLong ? lines.slice(0, 8).join('\n') + '\n…' : e.text
              return (
                <Text key={e.id} selectable style={{ fontSize: 12, lineHeight: 16, color: Colors.textPrimary, fontFamily: Typography.primary, opacity: e.deemphasize ? 0.35 : 1 }}>{preview}</Text>
              )
            })}
          </ScrollView>
        </View>

        <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={headerHeight + 8}>
          <View style={{ gap: 6, paddingBottom: 6 }}>
            <View style={{ flexDirection: 'row', alignItems: 'flex-end', gap: 8 }}>
              <TextInput
                value={prompt}
                onChangeText={(t) => { if (!growActive) setGrowActive(true); setPrompt(t); }}
                autoCapitalize="none"
                autoCorrect={false}
                placeholder="Type a message"
                multiline
                numberOfLines={MIN_LINES}
                onContentSizeChange={(e) => { const contentH = e.nativeEvent.contentSize?.height ?? LINE_HEIGHT; const target = contentH + PADDING_V * 2; setHeightStable(target, { allowShrink: false }); }}
                scrollEnabled={inputHeight >= MAX_HEIGHT - 1}
                textAlignVertical="top"
                style={{ flex: 1, borderWidth: 1, borderColor: Colors.border, padding: PADDING_V, height: inputHeight, backgroundColor: '#0F1217', color: Colors.textPrimary, fontSize: 13, lineHeight: LINE_HEIGHT, fontFamily: Typography.primary, borderRadius: 0 }}
                placeholderTextColor={Colors.textSecondary}
                onFocus={() => setGrowActive(true)}
              />
              <Pressable onPress={send} disabled={!connected || !prompt.trim()} style={{ backgroundColor: !connected || !prompt.trim() ? Colors.border : ui.button, paddingHorizontal: 16, paddingVertical: 12, borderRadius: 0, alignSelf: 'flex-start' }} accessibilityRole="button">
                <Text style={{ color: '#fff', fontFamily: Typography.bold }}>Send</Text>
              </Pressable>
            </View>
          </View>
        </KeyboardAvoidingView>
      </View>
    </SafeAreaView>
  )
}

