import { useCallback, useEffect, useRef, useState } from "react"
import { ActivityIndicator, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native"

import {
  type ConnectInfo,
  type ControlSessionEventRow,
  type ControlSessionRow,
  decodeConnectCode,
  fetchSessionEvents,
  fetchSessions,
} from "../src/control/control-client"
import { parseNodesResponse, pickConnect } from "../src/control/discovery-client"

// Discovery broker (Cloud Run today; updates.openagents.com once DNS lands).
// Owner is single-tenant for now ("fine for now security-wise").
const BROKER = "https://oa-updates-ezxz4mgdsq-uc.a.run.app"
const OWNER = "chris"
const POLL_MS = 4000

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

const stateTone = (state: string): string =>
  state === "completed"
    ? C.success
    : state === "running" || state === "started"
      ? C.info
      : state === "queued"
        ? C.warning
        : state === "failed" || state === "cancelled"
          ? C.danger
          : C.outline

type Status = "discovering" | "manual" | "connecting" | "connected" | "error"

export default function NodesScreen() {
  const [code, setCode] = useState("")
  const [conn, setConn] = useState<ConnectInfo | null>(null)
  const [sessions, setSessions] = useState<ControlSessionRow[]>([])
  const [status, setStatus] = useState<Status>("discovering")
  const [error, setError] = useState<string | null>(null)
  const [selected, setSelected] = useState<string | null>(null)
  const [events, setEvents] = useState<ControlSessionEventRow[]>([])
  const timer = useRef<ReturnType<typeof setInterval> | null>(null)
  const eventsTimer = useRef<ReturnType<typeof setInterval> | null>(null)

  const poll = useCallback(async (c: ConnectInfo) => {
    try {
      setSessions(await fetchSessions(c))
      setStatus("connected")
      setError(null)
    } catch (e) {
      setStatus("error")
      setError(e instanceof Error ? e.message : String(e))
    }
  }, [])

  // Auto-detect: on launch, ask the broker for this owner's nodes and connect to
  // the first reachable one (tailnet-first). Falls back to manual paste.
  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const res = await fetch(`${BROKER}/${OWNER}/nodes`)
        const nodes = parseNodesResponse(await res.json())
        if (cancelled) return
        if (nodes.length > 0) {
          const info = pickConnect(nodes[0])
          setConn(info)
          setStatus("connecting")
          void poll(info)
          return
        }
        setStatus("manual")
      } catch {
        if (!cancelled) setStatus("manual")
      }
    })()
    return () => {
      cancelled = true
    }
  }, [poll])

  useEffect(() => {
    if (conn === null) return
    timer.current = setInterval(() => void poll(conn), POLL_MS)
    return () => {
      if (timer.current) clearInterval(timer.current)
    }
  }, [conn, poll])

  // Live session-detail timeline: poll the selected session's recent events.
  const pollEvents = useCallback(
    async (c: ConnectInfo, sessionRef: string) => {
      try {
        setEvents(await fetchSessionEvents(c, sessionRef))
      } catch {
        // transient; keep the last good timeline
      }
    },
    [],
  )
  useEffect(() => {
    if (conn === null || selected === null) return
    setEvents([])
    void pollEvents(conn, selected)
    eventsTimer.current = setInterval(() => void pollEvents(conn, selected), POLL_MS)
    return () => {
      if (eventsTimer.current) clearInterval(eventsTimer.current)
    }
  }, [conn, selected, pollEvents])

  const connectManual = useCallback(() => {
    const info = decodeConnectCode(code)
    if (info === null) {
      setError("Invalid connect code")
      return
    }
    setConn(info)
    setStatus("connecting")
    void poll(info)
  }, [code, poll])

  return (
    <View style={styles.container}>
      <ScrollView contentContainerStyle={styles.content}>
        <Text style={styles.h1}>Autopilot</Text>
        <Text style={styles.subtitle}>Nodes</Text>

        {selected !== null ? (
          <>
            <Pressable style={styles.back} onPress={() => setSelected(null)}>
              <Text style={styles.backText}>‹ sessions</Text>
            </Pressable>
            <Text style={styles.detailRef}>{selected}</Text>
            {events.length === 0 ? (
              <View style={styles.card}>
                <Text style={styles.cardBody}>No events yet.</Text>
              </View>
            ) : (
              events.map((e) => (
                <View key={e.eventIndex} style={styles.eventRow}>
                  <View style={[styles.dot, { backgroundColor: stateTone(e.state) }]} />
                  <View style={styles.rowText}>
                    <Text style={styles.rowLabel}>{e.detail || e.phase}</Text>
                    <Text style={styles.rowStatus}>
                      {e.detail ? `${e.phase} · ` : ""}#{e.eventIndex} · {e.observedAt.slice(11, 19) || e.state}
                    </Text>
                  </View>
                </View>
              ))
            )}
          </>
        ) : status === "discovering" ? (
          <View style={styles.statusRow}>
            <ActivityIndicator color={C.info} />
            <Text style={styles.statusText}>finding your node…</Text>
          </View>
        ) : conn === null ? (
          <View style={styles.card}>
            <Text style={styles.cardTitle}>Connect to a Pylon node</Text>
            <Text style={styles.cardBody}>
              No node found automatically. Paste a connect code (tailnet/LAN
              address + token) to connect manually.
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
            <Pressable style={styles.button} onPress={connectManual}>
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
                <Pressable
                  key={s.sessionRef}
                  style={styles.row}
                  onPress={() => setSelected(s.sessionRef)}
                >
                  <View style={[styles.dot, { backgroundColor: stateTone(s.state) }]} />
                  <View style={styles.rowText}>
                    <Text style={styles.rowLabel}>{s.sessionRef}</Text>
                    <Text style={styles.rowStatus}>
                      {s.adapter} · {s.state}
                    </Text>
                  </View>
                  <Text style={styles.chevron}>›</Text>
                </Pressable>
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
  subtitle: { color: C.textSecondary, fontSize: 13, letterSpacing: 1, marginTop: 4, textTransform: "uppercase" },
  card: { backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 8, borderWidth: 1, marginTop: 24, padding: 18 },
  cardTitle: { color: C.primary, fontSize: 16, fontWeight: "600" },
  cardBody: { color: C.textSecondary, fontSize: 14, lineHeight: 20, marginTop: 8 },
  input: { backgroundColor: C.bg, borderColor: C.outline, borderRadius: 6, borderWidth: 1, color: C.text, fontFamily: "Courier", fontSize: 13, marginTop: 14, padding: 12 },
  button: { alignItems: "center", backgroundColor: C.primary, borderRadius: 6, marginTop: 12, padding: 12 },
  buttonText: { color: C.bg, fontSize: 15, fontWeight: "700" },
  error: { color: C.danger, fontSize: 13, marginTop: 10 },
  statusRow: { alignItems: "center", flexDirection: "row", gap: 10, marginTop: 24 },
  statusText: { color: C.text, fontFamily: "Courier", fontSize: 14 },
  row: { alignItems: "center", backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 8, borderWidth: 1, flexDirection: "row", marginTop: 12, padding: 14 },
  dot: { borderRadius: 6, height: 12, marginRight: 12, width: 12 },
  rowText: { flex: 1 },
  rowLabel: { color: C.text, fontFamily: "Courier", fontSize: 13 },
  rowStatus: { color: C.textSecondary, fontSize: 12, marginTop: 2 },
  chevron: { color: C.textSecondary, fontSize: 20, marginLeft: 8 },
  back: { marginTop: 20 },
  backText: { color: C.info, fontSize: 15 },
  detailRef: { color: C.text, fontFamily: "Courier", fontSize: 13, marginTop: 10 },
  eventRow: { alignItems: "center", backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 6, borderWidth: 1, flexDirection: "row", marginTop: 8, padding: 12 },
})
