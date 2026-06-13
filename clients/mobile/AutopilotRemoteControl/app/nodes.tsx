import { useCallback, useEffect, useRef, useState } from "react"
import { ActivityIndicator, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native"

import {
  type ConnectInfo,
  type ControlSessionRow,
  decodeConnectCode,
  fetchSessions,
} from "../src/control/control-client"

// Dark-mode tokens mirrored from the openagents.com website.
const C = {
  bg: "#000000",
  bgSecondary: "#151515",
  text: "#d7d8e5",
  textSecondary: "#8a8c93",
  outline: "#525458",
  primary: "#ffffff",
  success: "#00c853",
  warning: "#ffb400",
  danger: "#d32f2f",
  info: "#2979ff",
} as const

const stateTone = (state: string): string => {
  if (state === "completed") return C.success
  if (state === "running" || state === "started") return C.info
  if (state === "queued") return C.warning
  if (state === "failed" || state === "cancelled") return C.danger
  return C.outline
}

const POLL_MS = 4000

export default function NodesScreen() {
  const [code, setCode] = useState("")
  const [conn, setConn] = useState<ConnectInfo | null>(null)
  const [sessions, setSessions] = useState<ControlSessionRow[]>([])
  const [status, setStatus] = useState<"idle" | "connecting" | "connected" | "error">("idle")
  const [error, setError] = useState<string | null>(null)
  const timer = useRef<ReturnType<typeof setInterval> | null>(null)

  const poll = useCallback(async (c: ConnectInfo) => {
    try {
      const rows = await fetchSessions(c)
      setSessions(rows)
      setStatus("connected")
      setError(null)
    } catch (e) {
      setStatus("error")
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  const connect = useCallback(() => {
    const info = decodeConnectCode(code)
    if (info === null) {
      setStatus("error")
      setError("Invalid connect code")
      return
    }
    setConn(info)
    setStatus("connecting")
    void poll(info)
  }, [code, poll])

  useEffect(() => {
    if (conn === null) return
    timer.current = setInterval(() => void poll(conn), POLL_MS)
    return () => {
      if (timer.current) clearInterval(timer.current)
    }
  }, [conn, poll])

  return (
    <View style={styles.container}>
      <ScrollView contentContainerStyle={styles.content}>
        <Text style={styles.h1}>Autopilot</Text>
        <Text style={styles.subtitle}>Nodes</Text>

        {conn === null ? (
          <View style={styles.card}>
            <Text style={styles.cardTitle}>Connect to a Pylon node</Text>
            <Text style={styles.cardBody}>
              Paste the connect code from your node (it carries the tailnet/LAN
              address + token).
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
            <Pressable style={styles.button} onPress={connect}>
              <Text style={styles.buttonText}>Connect</Text>
            </Pressable>
            {error ? <Text style={styles.error}>{error}</Text> : null}
          </View>
        ) : (
          <>
            <View style={styles.statusRow}>
              {status === "connecting" ? <ActivityIndicator color={C.info} /> : null}
              <Text style={styles.statusText}>
                {status === "connected"
                  ? `connected · ${sessions.length} ${sessions.length === 1 ? "session" : "sessions"}`
                  : status === "error"
                    ? `error: ${error ?? "unknown"}`
                    : "connecting…"}
              </Text>
            </View>
            {sessions.length === 0 && status === "connected" ? (
              <View style={styles.card}>
                <Text style={styles.cardBody}>No sessions yet. Spawn one on the node.</Text>
              </View>
            ) : (
              sessions.map((s) => (
                <View key={s.sessionRef} style={styles.row}>
                  <View style={[styles.dot, { backgroundColor: stateTone(s.state) }]} />
                  <View style={styles.rowText}>
                    <Text style={styles.rowLabel}>{s.sessionRef}</Text>
                    <Text style={styles.rowStatus}>
                      {s.adapter} · {s.state}
                    </Text>
                  </View>
                </View>
              ))
            )}
          </>
        )}
      </ScrollView>
    </View>
  )
}

const styles = StyleSheet.create({
  container: { backgroundColor: C.bg, flex: 1 },
  content: { padding: 24, paddingTop: 64 },
  h1: { color: C.primary, fontSize: 22, fontWeight: "700" },
  subtitle: {
    color: C.textSecondary,
    fontSize: 13,
    letterSpacing: 1,
    marginTop: 4,
    textTransform: "uppercase",
  },
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
  statusRow: { alignItems: "center", flexDirection: "row", gap: 10, marginTop: 24 },
  statusText: { color: C.text, fontFamily: "Courier", fontSize: 14 },
  row: {
    alignItems: "center",
    backgroundColor: C.bgSecondary,
    borderColor: C.outline,
    borderRadius: 8,
    borderWidth: 1,
    flexDirection: "row",
    marginTop: 12,
    padding: 14,
  },
  dot: { borderRadius: 6, height: 12, marginRight: 12, width: 12 },
  rowText: { flex: 1 },
  rowLabel: { color: C.text, fontFamily: "Courier", fontSize: 13 },
  rowStatus: { color: C.textSecondary, fontSize: 12, marginTop: 2 },
})
