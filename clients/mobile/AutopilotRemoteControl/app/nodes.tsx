import { type ReactNode, useCallback, useEffect, useRef, useState } from "react"
import { ActivityIndicator, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native"

import {
  type ConnectInfo,
  type ControlSessionEventRow,
  type ControlSessionRow,
  type AccountRow,
  type SessionArtifact,
  cancelSession,
  decodeConnectCode,
  fetchAccounts,
  fetchSessionArtifact,
  fetchSessionEvents,
  fetchSessions,
  submitIntent,
} from "../src/control/control-client"
import { parseNodesResponse, pickConnect } from "../src/control/discovery-client"
import { CANONICAL_DARK } from "@openagentsinc/autopilot-control-protocol"

// Discovery broker (Cloud Run today; updates.openagents.com once DNS lands).
// Owner is single-tenant for now ("fine for now security-wise").
const BROKER = "https://oa-updates-ezxz4mgdsq-uc.a.run.app"
const OWNER = "chris"
const POLL_MS = 4000

// CL-31: the shared canonical dark palette, sourced from the protocol package
// (RN-safe) so mobile, desktop, and web stay in theming parity from one source.
const C = CANONICAL_DARK

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
  const [expanded, setExpanded] = useState<number | null>(null)
  const [askTitle, setAskTitle] = useState("")
  const [askBody, setAskBody] = useState("")
  const [askStatus, setAskStatus] = useState<string | null>(null)
  const [nodeName, setNodeName] = useState<string | null>(null)
  const [accounts, setAccounts] = useState<AccountRow[]>([])
  const [artifact, setArtifact] = useState<SessionArtifact | null>(null)
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
          setNodeName(nodes[0].name ?? nodes[0].id ?? "node")
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
  // Accounts change rarely — fetch once per connection (CL-18).
  useEffect(() => {
    if (conn === null) return
    let cancelled = false
    void fetchAccounts(conn)
      .then((rows) => {
        if (!cancelled) setAccounts(rows)
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [conn])

  useEffect(() => {
    if (conn === null || selected === null) return
    setEvents([])
    setArtifact(null)
    void pollEvents(conn, selected)
    void fetchSessionArtifact(conn, selected)
      .then((a) => setArtifact(a.kind === "none" ? null : a))
      .catch(() => {})
    eventsTimer.current = setInterval(() => void pollEvents(conn, selected), POLL_MS)
    return () => {
      if (eventsTimer.current) clearInterval(eventsTimer.current)
    }
  }, [conn, selected, pollEvents])

  const submitAsk = useCallback(() => {
    if (conn === null || askTitle.trim().length === 0) return
    setAskStatus("sending…")
    void submitIntent(conn, { title: askTitle, body: askBody })
      .then((s) => {
        setAskStatus(`sent · ${s}`)
        setAskTitle("")
        setAskBody("")
      })
      .catch((e) => setAskStatus(`error: ${e instanceof Error ? e.message : String(e)}`))
  }, [conn, askTitle, askBody])

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
            {(() => {
              const s = sessions.find((x) => x.sessionRef === selected)
              if (!s) return null
              const verify =
                s.state === "completed"
                  ? `✓ verify passed${s.artifactRef ? ` · artifact ${s.artifactRef.slice(-12)}` : ""}`
                  : s.state === "failed"
                    ? `✗ verify failed${s.errorClass ? ` · ${s.errorClass}` : ""}`
                    : s.state === "cancelled"
                      ? "cancelled"
                      : `${s.state}…`
              const tone =
                s.state === "completed" ? C.success : s.state === "failed" ? C.danger : C.textSecondary
              return <Text style={[styles.verifyLine, { color: tone }]}>{verify}</Text>
            })()}
            {artifact ? (
              <Text style={styles.artifactLine}>
                artifact: {artifact.outcome ?? artifact.kind}
                {artifact.editedFileCount !== null ? ` · ${artifact.editedFileCount} files` : ""}
                {artifact.commandCount !== null ? ` · ${artifact.commandCount} cmds` : ""}
                {artifact.totalTokens !== null ? ` · ${artifact.totalTokens} tok` : ""}
              </Text>
            ) : null}
            {(() => {
              const s = sessions.find((x) => x.sessionRef === selected)
              const cancellable = s && (s.state === "running" || s.state === "queued" || s.state === "started")
              if (!cancellable || conn === null) return null
              return (
                <Pressable
                  style={styles.cancelBtn}
                  onPress={() => {
                    void cancelSession(conn, selected).then(() => poll(conn))
                  }}
                >
                  <Text style={styles.cancelText}>Cancel session</Text>
                </Pressable>
              )
            })()}
            {events.length === 0 ? (
              <View style={styles.card}>
                <Text style={styles.cardBody}>No events yet.</Text>
              </View>
            ) : (
              events.map((e) => {
                const isOpen = expanded === e.eventIndex
                const expandable = e.full.length > 0 || e.detail.length > 30
                return (
                  <Pressable
                    key={e.eventIndex}
                    style={styles.eventRow}
                    onPress={() => expandable && setExpanded(isOpen ? null : e.eventIndex)}
                  >
                    <View style={[styles.dot, { backgroundColor: stateTone(e.state) }]} />
                    <View style={styles.rowText}>
                      <Text style={styles.rowLabel} numberOfLines={isOpen ? undefined : 2}>
                        {isOpen ? e.full || e.detail || e.phase : e.detail || e.phase}
                      </Text>
                      <Text style={styles.rowStatus}>
                        {e.detail ? `${e.phase} · ` : ""}#{e.eventIndex} · {e.observedAt.slice(11, 19) || e.state}
                        {expandable ? (isOpen ? " · tap to collapse" : " · tap to expand") : ""}
                      </Text>
                    </View>
                  </Pressable>
                )
              })
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
              <View style={[styles.dot, { backgroundColor: status === "connected" ? C.success : status === "error" ? C.danger : C.warning }]} />
              <Text style={styles.statusText}>
                {nodeName ? `${nodeName} · ` : ""}
                {status === "connected"
                  ? `online · ${sessions.length} ${sessions.length === 1 ? "session" : "sessions"}`
                  : status === "error"
                    ? `error: ${error ?? "unknown"}`
                    : "connecting…"}
              </Text>
            </View>
            {status === "connected" && sessions.length > 0 ? (
              <Text style={styles.breakdown}>
                {(() => {
                  const by: Record<string, number> = {}
                  for (const s of sessions) by[s.state] = (by[s.state] ?? 0) + 1
                  return ["running", "queued", "completed", "failed", "cancelled"]
                    .filter((k) => by[k])
                    .map((k) => `${by[k]} ${k}`)
                    .join(" · ")
                })()}
              </Text>
            ) : null}
            <View style={styles.card}>
              <Text style={styles.cardTitle}>Ask Autopilot</Text>
              <TextInput
                style={styles.input}
                placeholder="title — what do you want done?"
                placeholderTextColor={C.textSecondary}
                value={askTitle}
                onChangeText={setAskTitle}
              />
              <TextInput
                style={[styles.input, styles.inputMultiline]}
                placeholder="details (optional)"
                placeholderTextColor={C.textSecondary}
                value={askBody}
                onChangeText={setAskBody}
                multiline
              />
              <Pressable style={styles.button} onPress={submitAsk}>
                <Text style={styles.buttonText}>Send to node</Text>
              </Pressable>
              {askStatus ? <Text style={styles.askStatus}>{askStatus}</Text> : null}
            </View>
            {accounts.length > 0 ? (
              <View style={styles.card}>
                <Text style={styles.cardTitle}>Accounts</Text>
                {accounts.map((a, i) => (
                  <View key={`${a.provider}-${i}`} style={styles.acctRow}>
                    <View style={[styles.dot, { backgroundColor: a.ready ? C.success : C.warning }]} />
                    <Text style={styles.acctText}>
                      {a.provider} · {a.homeState} · {a.ready ? "ready" : "blocked"}
                    </Text>
                  </View>
                ))}
              </View>
            ) : null}
            {sessions.length === 0 && status === "connected" ? (
              <View style={styles.card}>
                <Text style={styles.cardBody}>No sessions yet. Spawn one on the node.</Text>
              </View>
            ) : (
              (() => {
                const childrenOf = (ref: string) => sessions.filter((c) => c.parentRef === ref)
                const isTop = (s: ControlSessionRow) =>
                  !s.parentRef || !sessions.some((p) => p.sessionRef === s.parentRef)
                const renderRow = (s: ControlSessionRow, child: boolean) => (
                  <Pressable
                    key={s.sessionRef}
                    style={[styles.row, child ? styles.childRow : null]}
                    onPress={() => setSelected(s.sessionRef)}
                  >
                    <View style={[styles.dot, { backgroundColor: stateTone(s.state) }]} />
                    <View style={styles.rowText}>
                      <Text style={styles.rowLabel} numberOfLines={2}>
                        {child ? "↳ " : ""}
                        {s.latestActivity || s.state}
                      </Text>
                      <Text style={styles.rowStatus}>
                        {s.agentKind ? `${s.agentKind} · ` : ""}
                        {s.state} · {s.sessionRef.slice(-6)}
                      </Text>
                    </View>
                    <Text style={styles.chevron}>›</Text>
                  </Pressable>
                )
                const rows: ReactNode[] = []
                for (const s of sessions.filter(isTop)) {
                  rows.push(renderRow(s, false))
                  for (const c of childrenOf(s.sessionRef)) rows.push(renderRow(c, true))
                }
                return rows
              })()
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
  inputMultiline: { minHeight: 64, textAlignVertical: "top" },
  askStatus: { color: C.textSecondary, fontSize: 12, marginTop: 10 },
  button: { alignItems: "center", backgroundColor: C.primary, borderRadius: 6, marginTop: 12, padding: 12 },
  buttonText: { color: C.bg, fontSize: 15, fontWeight: "700" },
  error: { color: C.danger, fontSize: 13, marginTop: 10 },
  statusRow: { alignItems: "center", flexDirection: "row", gap: 10, marginTop: 24 },
  statusText: { color: C.text, fontFamily: "Courier", fontSize: 14 },
  breakdown: { color: C.textSecondary, fontFamily: "Courier", fontSize: 12, marginTop: 6 },
  acctRow: { alignItems: "center", flexDirection: "row", marginTop: 10 },
  acctText: { color: C.text, fontFamily: "Courier", fontSize: 13 },
  row: { alignItems: "center", backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 8, borderWidth: 1, flexDirection: "row", marginTop: 12, padding: 14 },
  childRow: { marginLeft: 22, marginTop: 6, backgroundColor: C.bg },
  dot: { borderRadius: 6, height: 12, marginRight: 12, width: 12 },
  rowText: { flex: 1 },
  rowLabel: { color: C.text, fontFamily: "Courier", fontSize: 13 },
  rowStatus: { color: C.textSecondary, fontSize: 12, marginTop: 2 },
  chevron: { color: C.textSecondary, fontSize: 20, marginLeft: 8 },
  back: { marginTop: 20 },
  backText: { color: C.info, fontSize: 15 },
  detailRef: { color: C.text, fontFamily: "Courier", fontSize: 13, marginTop: 10 },
  verifyLine: { fontFamily: "Courier", fontSize: 13, marginTop: 8 },
  artifactLine: { color: C.textSecondary, fontFamily: "Courier", fontSize: 12, marginTop: 4 },
  cancelBtn: { alignItems: "center", borderColor: C.danger, borderRadius: 6, borderWidth: 1, marginTop: 12, padding: 10 },
  cancelText: { color: C.danger, fontSize: 14, fontWeight: "600" },
  eventRow: { alignItems: "center", backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 6, borderWidth: 1, flexDirection: "row", marginTop: 8, padding: 12 },
})
