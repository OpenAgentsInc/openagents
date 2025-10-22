import React, { useEffect, useMemo, useRef, useState } from "react";
import { ScrollView, Text, TextInput, View, Pressable, SafeAreaView } from "react-native";
import { Typography } from "@/constants/typography";
import { useWs } from "@/providers/ws";

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
  const [log, setLog] = useState("");
  const scrollRef = useRef<ScrollView | null>(null);
  const { connected, send: sendWs, setOnMessage, readOnly, networkEnabled, approvals, attachPreface } = useWs();

  const append = (line: string) => setLog((prev) => (prev ? prev + "\n" + line : line));

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
    setOnMessage((s) => setLog((prev) => (prev ? prev + "\n" + s : s)));
    return () => setOnMessage(null);
  }, [setOnMessage]);

  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: c.bg }}>
      <View style={{ flex: 1, padding: 16, gap: 14 }}>
        <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between" }}>
          <StatusPill connected={connected} color={c} />
          <Button title="Clear Log" onPress={() => setLog("")} color={c.card} textColor={c.text} />
        </View>

        <View style={{ gap: 8 }}>
          <Text style={{ fontSize: 12, color: c.sub, fontFamily: Typography.bold }}>Prompt</Text>
          <TextInput
            value={prompt}
            onChangeText={setPrompt}
            autoCapitalize="none"
            autoCorrect={false}
            placeholder="Hello, world"
            multiline
            style={{ borderWidth: 1, borderColor: c.border, padding: 12, borderRadius: 12, minHeight: 120, backgroundColor: c.input, color: c.text, fontSize: 13, fontFamily: Typography.primary }}
            placeholderTextColor={c.sub}
          />
          <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between" }}>
            <Text style={{ color: c.sub, fontSize: 12 }}>Sends raw text to Codex (newline auto‑appended)</Text>
            <Button title="Send" onPress={send} disabled={!connected || !prompt.trim()} color={connected && prompt.trim() ? c.primary : c.border} textColor={c.primaryText} />
          </View>
        </View>

        <View style={{ flex: 1, borderWidth: 1, borderColor: c.border, borderRadius: 12, backgroundColor: c.card }}>
          <ScrollView ref={scrollRef} onContentSizeChange={() => scrollRef.current?.scrollToEnd({ animated: true })} contentContainerStyle={{ padding: 12 }}>
            <Text selectable style={{ fontSize: 12, lineHeight: 16, color: c.text, fontFamily: Typography.primary }}>
              {log}
            </Text>
          </ScrollView>
        </View>
      </View>
    </SafeAreaView>
  );
}

function Button({ title, onPress, disabled, color = "#111", textColor = "#fff" }: { title: string; onPress: () => void; disabled?: boolean; color?: string; textColor?: string; }) {
  return (
    <Pressable onPress={disabled ? undefined : onPress} style={{ backgroundColor: disabled ? "#9CA3AF" : color, paddingHorizontal: 16, paddingVertical: 12, borderRadius: 10, alignSelf: "flex-start" }} hitSlop={6} accessibilityRole="button">
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
