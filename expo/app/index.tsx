import React, { useEffect, useMemo, useRef, useState } from "react";
import {
  Platform,
  ScrollView,
  Text,
  TextInput,
  View,
  Pressable,
  SafeAreaView,
} from "react-native";
import { Stack } from "expo-router";
import { Typography } from "@/constants/typography";

export default function Index() {
  const isDark = true; // keep it sleek; could wire to useColorScheme()
  const c = useMemo(
    () =>
      isDark
        ? {
            bg: "#0B0B0F",
            text: "#E5E7EB",
            sub: "#9CA3AF",
            card: "#111318",
            input: "#0F1217",
            border: "#272C35",
            primary: "#3F3F46", // gray-700
            primaryText: "#FFFFFF",
          }
        : {
            bg: "#FFFFFF",
            text: "#0F172A",
            sub: "#475569",
            card: "#F8FAFC",
            input: "#FFFFFF",
            border: "#E2E8F0",
            primary: "#525252", // gray-600
            primaryText: "#FFFFFF",
          },
    [isDark]
  );
  const [wsUrl, setWsUrl] = useState("ws://localhost:8787/ws");
  const [prompt, setPrompt] = useState("");
  const [log, setLog] = useState("");
  const [connected, setConnected] = useState(false);
  const wsRef = useRef<WebSocket | null>(null);
  const scrollRef = useRef<ScrollView | null>(null);

  const append = (line: string) => setLog((prev) => (prev ? prev + "\n" + line : line));

  const connect = () => {
    try {
      const ws = new WebSocket(wsUrl);
      wsRef.current = ws;
      ws.onopen = () => {
        setConnected(true);
        append(`Connected → ${wsUrl}`);
      };
      ws.onmessage = (evt) => {
        const data = typeof evt.data === "string" ? evt.data : String(evt.data);
        append(data);
      };
      ws.onerror = (evt: any) => {
        append(`WS error: ${evt?.message ?? "unknown"}`);
      };
      ws.onclose = () => {
        setConnected(false);
        append("Disconnected");
      };
    } catch (e: any) {
      append(`Failed to connect: ${e?.message ?? e}`);
    }
  };

  const disconnect = () => {
    wsRef.current?.close();
    wsRef.current = null;
  };

  const send = () => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      append("Not connected");
      return;
    }
    const payload = prompt.endsWith("\n") ? prompt : prompt + "\n";
    ws.send(payload);
    append(`>> ${prompt}`);
    setPrompt("");
  };

  useEffect(() => {
    return () => {
      wsRef.current?.close();
    };
  }, []);

  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: c.bg }}>
      <Stack.Screen options={{ title: "Tricoder", headerTitleStyle: { fontFamily: Typography.bold } }} />
      <View style={{ flex: 1, padding: 16, gap: 14 }}>
      <View style={{ gap: 8 }}>
        <Text style={{ fontSize: 12, color: c.sub, fontFamily: Typography.bold }}>WebSocket URL</Text>
        <TextInput
          value={wsUrl}
          onChangeText={setWsUrl}
          autoCapitalize="none"
          autoCorrect={false}
          placeholder="ws://localhost:8787/ws"
          style={{
            borderWidth: 1,
            borderColor: c.border,
            padding: 12,
            borderRadius: 12,
            backgroundColor: c.input,
            color: c.text,
          }}
          placeholderTextColor={c.sub}
        />
        <View style={{ flexDirection: "row", alignItems: "center", gap: 10 }}>
          {!connected ? (
            <Button title="Connect" onPress={connect} color={c.primary} textColor={c.primaryText} />
          ) : (
            <Button title="Disconnect" onPress={disconnect} color="#4B5563" textColor="#FFFFFF" />
          )}
          <Button title="Clear" onPress={() => setLog("")} color={c.card} textColor={c.text} />
          <StatusPill connected={connected} color={c} />
        </View>
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
          style={{
            borderWidth: 1,
            borderColor: c.border,
            padding: 12,
            borderRadius: 12,
            minHeight: 120,
            backgroundColor: c.input,
            color: c.text,
            fontSize: 13,
          }}
          placeholderTextColor={c.sub}
        />
        <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between" }}>
          <Text style={{ color: c.sub, fontSize: 12 }}>Sends raw text to Codex (newline auto‑appended)</Text>
          <Button
            title="Send"
            onPress={send}
            disabled={!connected || !prompt.trim()}
            color={connected && prompt.trim() ? c.primary : c.border}
            textColor={c.primaryText}
          />
        </View>
      </View>

      <View
        style={{
          flex: 1,
          borderWidth: 1,
          borderColor: c.border,
          borderRadius: 12,
          backgroundColor: c.card,
        }}
      >
        <ScrollView
          ref={scrollRef}
          onContentSizeChange={() => scrollRef.current?.scrollToEnd({ animated: true })}
          contentContainerStyle={{ padding: 12 }}
        >
          <Text
            selectable
            style={{
              fontSize: 12,
              lineHeight: 16,
              color: c.text,
            }}
          >
            {log}
          </Text>
        </ScrollView>
      </View>
      </View>
    </SafeAreaView>
  );
}

function Button({
  title,
  onPress,
  disabled,
  color = "#111",
  textColor = "#fff",
}: {
  title: string;
  onPress: () => void;
  disabled?: boolean;
  color?: string;
  textColor?: string;
}) {
  return (
    <Pressable
      onPress={disabled ? undefined : onPress}
      style={{
        backgroundColor: disabled ? "#9CA3AF" : color,
        paddingHorizontal: 16,
        paddingVertical: 12,
        borderRadius: 10,
        alignSelf: "flex-start",
      }}
      hitSlop={6}
      accessibilityRole="button"
    >
      <Text style={{ color: textColor, fontFamily: Typography.bold }}>{title}</Text>
    </Pressable>
  );
}

function StatusPill({ connected, color: c }: { connected: boolean; color: any }) {
  return (
    <View
      style={{
        paddingHorizontal: 10,
        paddingVertical: 6,
        borderRadius: 999,
        borderWidth: 1,
        borderColor: connected ? "#A3A3A3" : c.border,
        backgroundColor: connected ? "rgba(255,255,255,0.06)" : "rgba(255,255,255,0.02)",
      }}
    >
      <Text style={{ color: connected ? "#D4D4D8" : c.sub, fontSize: 12, fontFamily: Typography.bold }}>
        {connected ? "Connected" : "Disconnected"}
      </Text>
    </View>
  );
}
