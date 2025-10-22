import React, { useEffect, useMemo, useRef, useState } from "react"
import { Pressable, SafeAreaView, ScrollView, Text, TextInput, View } from "react-native"
import Markdown from 'react-native-markdown-display'
import { Typography } from "@/constants/typography"
import { useWs } from "@/providers/ws"
import { parseCodexLine } from "@/lib/codex-events"

export default function ConsoleScreen() {
  const isDark = true;
  const c = useMemo(
    () =>
      isDark
        ? { bg: "#0B0B0F", text: "#E5E7EB", sub: "#9CA3AF", card: "#111318", input: "#0F1217", border: "#272C35", primary: "#3F3F46", primaryText: "#FFFFFF" }
        : { bg: "#FFFFFF", text: "#0F172A", sub: "#475569", card: "#F8FAFC", input: "#FFFFFF", border: "#E2E8F0", primary: "#525252", primaryText: "#FFFFFF" },
    [isDark]
  );
  const [prompt, setPrompt] = useState("Summarize the current repo. Use a maximum of 4 tool calls.");
  // Auto-growing composer sizing
  const LINE_HEIGHT = 18;
  const MIN_LINES = 1;
  const MAX_LINES = 10;
  const PADDING_V = 10; // must match TextInput paddingVertical
  const [inputHeight, setInputHeight] = useState(LINE_HEIGHT * MIN_LINES + PADDING_V * 2);
  type Entry = { id: number; text: string; deemphasize?: boolean }
  const [log, setLog] = useState<Entry[]>([])
  const idRef = useRef(1)
  const scrollRef = useRef<ScrollView | null>(null);
  const { connected, send: sendWs, setOnMessage, readOnly, networkEnabled, approvals, attachPreface, setClearLogHandler } = useWs();

  const append = (text: string, deemphasize?: boolean) => setLog((prev) => [...prev, { id: idRef.current++, text, deemphasize }])

  const buildPreface = () => {
    return `You are a coding agent running in the Codex CLI.
Capabilities: read files, propose patches with apply_patch, run shell commands.
Environment:
- Filesystem: ${readOnly ? 'read-only' : 'write access within workspace'}
- Network: ${networkEnabled ? 'enabled' : 'restricted'}
- Approvals: ${approvals}

When unsafe, ask for confirmation and avoid destructive actions.`;
  };

  const send = () => {
    const base = prompt.trim();
    const finalText = attachPreface ? `${buildPreface()}\n\n${base}` : base;
    const payload = finalText.endsWith("\n") ? finalText : finalText + "\n";
    if (!sendWs(payload)) { append("Not connected"); return; }
    append(`>> ${base}`);
    setPrompt("");
  };

  useEffect(() => {
    setOnMessage((chunk) => {
      const lines = String(chunk).split(/\r?\n/)
      for (const line of lines) {
        const trimmed = line.trim()
        if (!trimmed) continue
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
          append(parsed.raw, false)
        }
      }
    })
    return () => setOnMessage(null)
  }, [setOnMessage])

  useEffect(() => {
    // Allow Settings → Clear Log to wipe this feed
    setClearLogHandler(() => setLog([]))
    return () => setClearLogHandler(null)
  }, [setClearLogHandler])

  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: c.bg }}>
      <View style={{ flex: 1, padding: 16, gap: 14 }}>
        {/* Header status moved to headerRight (dot). Clear Log moved to Settings. */}

        <View style={{ flex: 1, borderWidth: 1, borderColor: c.border }}>
          <ScrollView ref={scrollRef} onContentSizeChange={() => scrollRef.current?.scrollToEnd({ animated: true })} contentContainerStyle={{ padding: 12 }}>
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
              return (
                <Text key={e.id} selectable style={{ fontSize: 12, lineHeight: 16, color: c.text, fontFamily: Typography.primary, opacity: e.deemphasize ? 0.2 : 1 }}>
                  {e.text}
                </Text>
              )
            })}
          </ScrollView>
        </View>

        {/* Composer under the feed */}
        <View style={{ gap: 6 }}>
          <Text style={{ fontSize: 12, color: c.sub, fontFamily: Typography.bold }}>Prompt</Text>
          <View style={{ flexDirection: 'row', alignItems: 'flex-end', gap: 8 }}>
            <TextInput
              value={prompt}
              onChangeText={setPrompt}
              autoCapitalize="none"
              autoCorrect={false}
              placeholder="Hello, world"
              multiline
              numberOfLines={MIN_LINES}
              onContentSizeChange={(e) => {
                const h = e.nativeEvent.contentSize.height || LINE_HEIGHT;
                const lines = Math.max(MIN_LINES, Math.round(h / LINE_HEIGHT));
                const clamped = Math.min(MAX_LINES, lines);
                setInputHeight(clamped * LINE_HEIGHT + PADDING_V * 2);
              }}
              scrollEnabled={inputHeight >= MAX_LINES * LINE_HEIGHT + PADDING_V * 2}
              textAlignVertical="top"
              style={{ flex: 1, borderWidth: 1, borderColor: c.border, padding: PADDING_V, height: inputHeight, backgroundColor: c.input, color: c.text, fontSize: 13, lineHeight: LINE_HEIGHT, fontFamily: Typography.primary, borderRadius: 0 }}
              placeholderTextColor={c.sub}
            />
            <Button title="Send" onPress={send} disabled={!connected || !prompt.trim()} color={connected && prompt.trim() ? c.primary : c.border} textColor={c.primaryText} />
          </View>
        </View>

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
