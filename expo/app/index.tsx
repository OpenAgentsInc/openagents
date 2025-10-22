import React, { useEffect, useRef, useState } from "react";
import {
  Platform,
  ScrollView,
  Text,
  TextInput,
  View,
  Pressable,
} from "react-native";

export default function Index() {
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
    <View style={{ flex: 1, paddingTop: 48, paddingHorizontal: 12, gap: 12 }}>
      <View style={{ gap: 8 }}>
        <Text style={{ fontSize: 12, opacity: 0.8 }}>WebSocket URL</Text>
        <TextInput
          value={wsUrl}
          onChangeText={setWsUrl}
          autoCapitalize="none"
          autoCorrect={false}
          placeholder="ws://localhost:8787/ws"
          style={{
            borderWidth: 1,
            borderColor: "#ccc",
            padding: 10,
            borderRadius: 8,
          }}
        />
        <View style={{ flexDirection: "row", gap: 8 }}>
          {!connected ? (
            <Button title="Connect" onPress={connect} />
          ) : (
            <Button title="Disconnect" onPress={disconnect} />
          )}
          <Button title="Clear" onPress={() => setLog("")} />
        </View>
      </View>

      <View style={{ gap: 8 }}>
        <Text style={{ fontSize: 12, opacity: 0.8 }}>Prompt (raw; sent as-is)</Text>
        <TextInput
          value={prompt}
          onChangeText={setPrompt}
          autoCapitalize="none"
          autoCorrect={false}
          placeholder='{"prompt":"Hello"}'
          multiline
          style={{
            borderWidth: 1,
            borderColor: "#ccc",
            padding: 10,
            borderRadius: 8,
            minHeight: 80,
          }}
        />
        <Button title="Send" onPress={send} disabled={!connected || !prompt.trim()} />
      </View>

      <View style={{ flex: 1, borderWidth: 1, borderColor: "#ddd", borderRadius: 8 }}>
        <ScrollView
          ref={scrollRef}
          onContentSizeChange={() => scrollRef.current?.scrollToEnd({ animated: true })}
          contentContainerStyle={{ padding: 10 }}
        >
          <Text
            selectable
            style={{
              fontSize: 12,
              lineHeight: 16,
              fontFamily: Platform.select({ ios: "Menlo", android: "monospace", default: "monospace" }),
            }}
          >
            {log}
          </Text>
        </ScrollView>
      </View>
    </View>
  );
}

function Button({ title, onPress, disabled }: { title: string; onPress: () => void; disabled?: boolean }) {
  return (
    <Pressable
      onPress={disabled ? undefined : onPress}
      style={{
        backgroundColor: disabled ? "#ccc" : "#111",
        paddingHorizontal: 16,
        paddingVertical: 10,
        borderRadius: 8,
      }}
    >
      <Text style={{ color: "white", fontWeight: "600" }}>{title}</Text>
    </Pressable>
  );
}
