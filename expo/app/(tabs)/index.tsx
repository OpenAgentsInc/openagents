import React, { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
    KeyboardAvoidingView, Platform, Pressable, SafeAreaView, ScrollView, Text,
    TextInput, View
} from "react-native"
import Markdown from "react-native-markdown-display"
import { Typography } from "@/constants/typography"
import { parseCodexLine } from "@/lib/codex-events"
import { useWs } from "@/providers/ws"
import { useHeaderHeight } from "@react-navigation/elements"
import { useRouter } from "expo-router"
import { loadLogs, saveLogs, putLog, clearLogs as clearLogsStore } from "@/lib/log-store"

export default function ConsoleScreen() {
  const headerHeight = useHeaderHeight();
  const router = useRouter();
  const isDark = true;
  const c = useMemo(
    () =>
      isDark
        ? { bg: "#0B0B0F", text: "#E5E7EB", sub: "#9CA3AF", card: "#111318", input: "#0F1217", border: "#272C35", primary: "#3F3F46", primaryText: "#FFFFFF" }
        : { bg: "#FFFFFF", text: "#0F172A", sub: "#475569", card: "#F8FAFC", input: "#FFFFFF", border: "#E2E8F0", primary: "#525252", primaryText: "#FFFFFF" },
    [isDark]
  );
  const [prompt, setPrompt] = useState("Summarize the current repo. Use a maximum of 4 tool calls.");
  // Auto-growing composer sizing (stable, jitter-resistant)
  const LINE_HEIGHT = 18;
  const MIN_LINES = 1;
  const MAX_LINES = 10;
  const PADDING_V = 10; // must match TextInput paddingVertical
  const MIN_HEIGHT = LINE_HEIGHT * MIN_LINES + PADDING_V * 2;
  const MAX_HEIGHT = LINE_HEIGHT * MAX_LINES + PADDING_V * 2;
  const [inputHeight, setInputHeight] = useState(MIN_HEIGHT);
  const lastHeightRef = useRef(MIN_HEIGHT);
  const rafRef = useRef<number | null>(null);
  // If the input is prefilled (esp. with newlines), allow initial growth
  const [growActive, setGrowActive] = useState(() => {
    return !!prompt && (prompt.includes('\n') || prompt.length > 0);
  });

  const setHeightStable = useCallback((target: number, { allowShrink = false }: { allowShrink?: boolean } = {}) => {
    const clamped = Math.max(MIN_HEIGHT, Math.min(MAX_HEIGHT, Math.round(target)));
    // Grow-only during typing; equal or smaller heights are ignored unless allowShrink
    if (!allowShrink && clamped <= lastHeightRef.current) return;
    if (rafRef.current) cancelAnimationFrame(rafRef.current as any);
    rafRef.current = requestAnimationFrame(() => {
      lastHeightRef.current = clamped;
      setInputHeight(clamped);
    });
  }, []);
  type Entry = { id: number; text: string; deemphasize?: boolean }
  const [log, setLog] = useState<Entry[]>([])
  const idRef = useRef(1)
  const scrollRef = useRef<ScrollView | null>(null);
  const { connected, send: sendWs, setOnMessage, readOnly, networkEnabled, approvals, attachPreface, setClearLogHandler } = useWs();

  const append = (text: string, deemphasize?: boolean, kind: 'md'|'reason'|'text'|'json'|'summary'|'delta' = 'text') => {
    const id = idRef.current++
    putLog({ id, text, kind, deemphasize, ts: Date.now() })
    setLog((prev) => [...prev, { id, text, deemphasize }])
    saveLogs()
  }

  const buildPreface = () => {
    return `You are a coding agent running in the Codex CLI.
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
`;
  };

  const send = () => {
    const base = prompt.trim();
    const finalText = attachPreface ? `${buildPreface()}\n\n${base}` : base;
    const payload = finalText.endsWith("\n") ? finalText : finalText + "\n";
    if (!sendWs(payload)) { append("Not connected"); return; }
    append(`> ${base}`);
    setPrompt("");
    // Reset height after send to avoid lingering tall composer
    lastHeightRef.current = MIN_HEIGHT;
    setInputHeight(MIN_HEIGHT);
    setGrowActive(false);
  };

  useEffect(() => {
    setOnMessage((chunk) => {
      const lines = String(chunk).split(/\r?\n/)
      for (const line of lines) {
        const trimmed = line.trim()
        if (!trimmed) continue
        // Skip exec_command_output_delta entire JSON blob (header + chunk rows)
        const skippingRef = (ConsoleScreen as any)._skipExecRef || ((ConsoleScreen as any)._skipExecRef = { current: false })
        if (skippingRef.current) {
          // End skip when closing brackets likely appear
          if (trimmed.includes(']}') || trimmed.endsWith(']') || trimmed.includes('}}')) {
            skippingRef.current = false
          }
          continue
        }
        if (trimmed.includes('exec_command_output_delta')) {
          skippingRef.current = true
          continue
        }
        const parsed = parseCodexLine(trimmed)
        if (parsed.kind === 'delta') {
          // Delta summaries should be deemphasized like raw data
          append(parsed.summary, true)
        } else if (parsed.kind === 'md') {
          // Render markdown-only message for agent_message
          append(`::md::${parsed.markdown}`, false)
        } else if (parsed.kind === 'reason') {
          append(`::reason::${parsed.text}`, false)
        } else if (parsed.kind === 'summary') {
          append(parsed.text, true) // exec_* summaries should be deemphasized
        } else if (parsed.kind === 'json') {
          append(parsed.raw, true) // deemphasize full JSON
        } else {
          // All plain text should also be dim unless we build a custom component
          append(parsed.raw, true)
        }
      }
    })
    return () => setOnMessage(null)
  }, [setOnMessage])

  useEffect(() => {
    // Allow Settings → Clear Log to wipe this feed
    setClearLogHandler(() => { setLog([]); clearLogsStore(); })
    return () => setClearLogHandler(null)
  }, [setClearLogHandler])

  useEffect(() => {
    // Hydrate history on mount
    (async () => {
      const items = await loadLogs();
      if (items.length) {
        setLog(items.map(({ id, text, deemphasize }) => ({ id, text, deemphasize })) )
        idRef.current = Math.max(...items.map(i => i.id)) + 1
      }
    })();
  }, [])

  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: c.bg }}>
      <View style={{ flex: 1, paddingTop: 0, paddingBottom: 14, paddingHorizontal: 8, gap: 14 }}>
        {/* Header status moved to headerRight (dot). Clear Log moved to Settings. */}

        <View style={{ flex: 1 }}>
          <ScrollView
            ref={scrollRef}
            onContentSizeChange={() => scrollRef.current?.scrollToEnd({ animated: true })}
            contentContainerStyle={{ paddingTop: 0, paddingBottom: 6, paddingHorizontal: 8 }}
          >
            {log.map((e) => {
              const isMd = e.text.startsWith('::md::')
              if (isMd) {
                const md = e.text.slice('::md::'.length)
                return (
                  <Markdown
                    key={e.id}
                    style={{
                      body: { color: c.text, fontFamily: Typography.primary, fontSize: 13, lineHeight: 18 },
                      paragraph: { color: c.text },
                      code_inline: { backgroundColor: '#0F1217', color: c.text, borderWidth: 1, borderColor: c.border, paddingHorizontal: 4, paddingVertical: 2 },
                      code_block: { backgroundColor: '#0F1217', color: c.text, borderWidth: 1, borderColor: c.border, padding: 8 },
                      fence: { backgroundColor: '#0F1217', color: c.text, borderWidth: 1, borderColor: c.border, padding: 8 },
                    }}
                  >
                    {md}
                  </Markdown>
                )
              }
              const isReason = e.text.startsWith('::reason::')
              if (isReason) {
                const md = e.text.slice('::reason::'.length)
                return (
                  <Markdown
                    key={e.id}
                    style={{
                      body: { color: c.sub, fontFamily: Typography.primary, fontSize: 12, lineHeight: 18 },
                      paragraph: { color: c.sub },
                      code_inline: { backgroundColor: '#0F1217', color: c.sub, borderWidth: 1, borderColor: c.border, paddingHorizontal: 4, paddingVertical: 2 },
                      code_block: { backgroundColor: '#0F1217', color: c.sub, borderWidth: 1, borderColor: c.border, padding: 8 },
                      fence: { backgroundColor: '#0F1217', color: c.sub, borderWidth: 1, borderColor: c.border, padding: 8 },
                    }}
                  >
                    {md}
                  </Markdown>
                )
              }
              // Collapsible for long plain entries
              const lines = e.text.split(/\r?\n/)
              const isLong = lines.length > 8
              const preview = isLong ? lines.slice(0, 8).join('\n') + '\n…' : e.text
              return (
                <Pressable key={e.id} onPress={() => { if (isLong) router.push(`/message/${e.id}`) }}>
                  <Text selectable style={{ fontSize: 12, lineHeight: 16, color: c.text, fontFamily: Typography.primary, opacity: e.deemphasize ? 0.35 : 1 }}>
                    {preview}
                  </Text>
                </Pressable>
              )
            })}
          </ScrollView>
        </View>

        {/* Composer under the feed, keyboard-safe */}
        <KeyboardAvoidingView
          behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
          keyboardVerticalOffset={headerHeight + 8}
        >
          <View style={{ gap: 6, paddingBottom: 6 }}>
            <View style={{ flexDirection: 'row', alignItems: 'flex-end', gap: 8 }}>
              <TextInput
                value={prompt}
                onChangeText={(t) => { if (!growActive) setGrowActive(true); setPrompt(t); }}
                autoCapitalize="none"
                autoCorrect={false}
                placeholder="Hello, world"
                multiline
                numberOfLines={MIN_LINES}
              onContentSizeChange={(e) => {
                const contentH = e.nativeEvent.contentSize?.height ?? LINE_HEIGHT;
                const target = contentH + PADDING_V * 2;
                // setHeightStable is already grow-only (no shrink) unless allowShrink=true
                setHeightStable(target, { allowShrink: false });
              }}
                scrollEnabled={inputHeight >= MAX_HEIGHT - 1}
                textAlignVertical="top"
                style={{ flex: 1, borderWidth: 1, borderColor: c.border, padding: PADDING_V, height: inputHeight, backgroundColor: c.input, color: c.text, fontSize: 13, lineHeight: LINE_HEIGHT, fontFamily: Typography.primary, borderRadius: 0 }}
                placeholderTextColor={c.sub}
                onFocus={() => setGrowActive(true)}
              />
              <Button title="Send" onPress={send} disabled={!connected || !prompt.trim()} color={connected && prompt.trim() ? c.primary : c.border} textColor={c.primaryText} />
            </View>
          </View>
        </KeyboardAvoidingView>

      </View>
    </SafeAreaView>
  );
}

function Button({ title, onPress, disabled, color = "#111", textColor = "#fff" }: { title: string; onPress: () => void; disabled?: boolean; color?: string; textColor?: string; }) {
  return (
    <Pressable onPress={disabled ? undefined : onPress} style={{ backgroundColor: disabled ? "#9CA3AF" : color, paddingHorizontal: 16, paddingVertical: 12, borderRadius: 0, alignSelf: "flex-start" }} hitSlop={6} accessibilityRole="button">
      <Text style={{ color: textColor, fontFamily: Typography.bold }}>{title}</Text>
    </Pressable>
  );
}

function StatusPill({ connected, color: c }: { connected: boolean; color: any }) {
  return (
    <View style={{ paddingHorizontal: 10, paddingVertical: 6, borderRadius: 999, borderWidth: 1, borderColor: connected ? "#A3A3A3" : c.border, backgroundColor: connected ? "rgba(255,255,255,0.06)" : "rgba(255,255,255,0.02)" }}>
      <Text style={{ color: connected ? "#D4D4D8" : c.sub, fontSize: 12, fontFamily: Typography.bold }}>{connected ? "Connected" : "Disconnected"}</Text>
    </View>
  );
}
