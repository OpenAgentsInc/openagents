import { useState } from "react"
import { Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native"
import { CANONICAL_DARK } from "@openagentsinc/autopilot-control-protocol"
import { useConnection } from "../src/connection/ConnectionContext"

const C = CANONICAL_DARK

export default function SettingsScreen() {
  const c = useConnection()
  const [code, setCode] = useState("")
  const [connectError, setConnectError] = useState<string | null>(null)

  function handleConnect() {
    const ok = c.connectManual(code)
    if (!ok) {
      setConnectError("Invalid connect code — paste the full code from your Pylon node.")
    } else {
      setConnectError(null)
      setCode("")
    }
  }

  const statusLabel =
    c.status === "connected"
      ? "online"
      : c.status === "error"
        ? `error: ${c.error ?? "unknown"}`
        : c.status === "discovering"
          ? "discovering…"
          : c.status === "connecting"
            ? "connecting…"
            : "manual"

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.h1}>Settings</Text>

      {/* Connection ─────────────────────────────────────────────────── */}
      <View style={styles.card}>
        <Text style={styles.cardTitle}>Connection</Text>
        <Text style={styles.cardBody}>
          {c.nodeName ? `Node: ${c.nodeName} · ` : ""}
          {statusLabel}
        </Text>

        <Text style={[styles.cardBody, { marginTop: 12 }]}>
          Paste a connect code to connect to a Pylon node manually (tailnet/LAN address + token):
        </Text>
        <TextInput
          style={styles.input}
          placeholder="connect code"
          placeholderTextColor={C.textSecondary}
          autoCapitalize="none"
          autoCorrect={false}
          value={code}
          onChangeText={setCode}
        />
        <Pressable style={styles.button} onPress={handleConnect}>
          <Text style={styles.buttonText}>Connect</Text>
        </Pressable>
        {connectError ? <Text style={styles.error}>{connectError}</Text> : null}
      </View>

      {/* Notifications ──────────────────────────────────────────────── */}
      <View style={styles.card}>
        <Text style={styles.cardTitle}>Notifications</Text>
        <Text style={styles.cardBody}>
          iOS notifications fire on new session state transitions. The app requests notification
          permission on first launch and sends a local notification whenever a session changes state
          (e.g. running → completed or failed).
        </Text>
      </View>

      {/* Theme ──────────────────────────────────────────────────────── */}
      <View style={styles.card}>
        <Text style={styles.cardTitle}>Theme</Text>
        <Text style={styles.cardBody}>Dark</Text>
        <Text style={styles.muted}>
          Theme is read-only. All surfaces share the canonical dark token palette.
        </Text>
      </View>

      {/* Updates ────────────────────────────────────────────────────── */}
      <View style={styles.card}>
        <Text style={styles.cardTitle}>Updates</Text>
        <Text style={styles.cardBody}>
          JS-only changes ship over-the-air via updates.openagents.com — no App Store update
          required. Native changes ship as new TestFlight builds.
        </Text>
      </View>

      {/* About ──────────────────────────────────────────────────────── */}
      <View style={styles.card}>
        <Text style={styles.cardTitle}>About</Text>
        <Text style={styles.cardBody}>Autopilot Remote Control</Text>
        {c.conn ? (
          <Text style={styles.muted}>
            Protocol schema: {(c.conn as unknown as { schema?: string }).schema ?? "—"}
          </Text>
        ) : null}
      </View>
    </ScrollView>
  )
}

const styles = StyleSheet.create({
  container: { backgroundColor: C.bg, flex: 1 },
  content: { padding: 24, paddingTop: 50 },
  h1: { color: C.primary, fontSize: 22, fontWeight: "700" },
  card: {
    backgroundColor: C.bgSecondary,
    borderColor: C.outline,
    borderRadius: 8,
    borderWidth: 1,
    marginTop: 24,
    padding: 18,
  },
  cardTitle: { color: C.primary, fontSize: 16, fontWeight: "600" },
  cardBody: { color: C.textSecondary, fontSize: 14, lineHeight: 20, marginTop: 8 },
  muted: { color: C.textSecondary, fontSize: 12, marginTop: 6 },
  input: {
    backgroundColor: C.bg,
    borderColor: C.outline,
    borderRadius: 6,
    borderWidth: 1,
    color: C.text,
    fontFamily: "Courier",
    fontSize: 13,
    marginTop: 14,
    padding: 12,
  },
  button: {
    alignItems: "center",
    backgroundColor: C.primary,
    borderRadius: 6,
    marginTop: 12,
    padding: 12,
  },
  buttonText: { color: C.bg, fontSize: 15, fontWeight: "700" },
  error: { color: C.danger, fontSize: 13, marginTop: 10 },
})
