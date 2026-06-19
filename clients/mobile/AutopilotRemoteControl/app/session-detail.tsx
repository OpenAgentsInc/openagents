// CL-59: dedicated, routed session-detail screen. Previously this detail lived
// inline inside app/nodes.tsx (a `selected` branch). Promoting it to its own
// screen lets the session list `navigate("SessionDetail", { sessionRef })`
// instead of mutating local nodes.tsx state, and shares the live node
// projection through `useConnection()` (CL-54) so the screen polls the node
// directly for its events + artifact.

import { useCallback, useEffect, useRef, useState } from "react"
import { Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native"
import { useNavigation, useRoute } from "@react-navigation/native"

import type { ControlSessionEventRow, SessionArtifact } from "../src/control/control-client"
import { useConnection } from "../src/connection/ConnectionContext"
import { CANONICAL_DARK } from "@openagentsinc/autopilot-control-protocol"

// `verifyText` is the pure verify-line projection. It lives in a RN-free module
// so `bun test` can exercise it (repo convention), and is re-exported here so
// the detail screen still owns its public surface.
export { verifyText } from "./session-detail-view-model"

import { verifyText } from "./session-detail-view-model"

const C = CANONICAL_DARK
const POLL_MS = 4000

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

const toneColor = (tone: "ok" | "bad" | "muted"): string =>
  tone === "ok" ? C.success : tone === "bad" ? C.danger : C.textSecondary

export default function SessionDetailScreen() {
  const route = useRoute()
  const sessionRef = (route.params as { sessionRef?: string } | undefined)?.sessionRef ?? null
  const navigation = useNavigation<{
    goBack: () => void
    navigate: (screen: string, params?: { sessionRef: string }) => void
  }>()
  const c = useConnection()

  const session = sessionRef === null ? undefined : c.sessions.find((s) => s.sessionRef === sessionRef)

  const [events, setEvents] = useState<ControlSessionEventRow[]>([])
  const [artifact, setArtifact] = useState<SessionArtifact | null>(null)
  const [expanded, setExpanded] = useState<number | null>(null)
  const [turnText, setTurnText] = useState("")
  const [turnSending, setTurnSending] = useState(false)
  const [turnError, setTurnError] = useState<string | null>(null)
  const eventsTimer = useRef<ReturnType<typeof setInterval> | null>(null)

  const pollEvents = useCallback(
    async (ref: string) => {
      try {
        setEvents(await c.fetchSessionEvents(ref))
      } catch {
        // transient; keep the last good timeline
      }
    },
    [c],
  )

  // Load events + the artifact once, on mount / ref change, then keep the
  // timeline live. #5493: prefer the bridge `session.subscribe` stream — every
  // new event batch triggers an immediate rich-events refresh, so the timeline
  // advances within ~STREAM_MS of activity instead of the 4s poll. When the node
  // has no bridge stream (subscribeSession reports streaming:false) we fall back
  // to the periodic poll. The initial load always runs once regardless.
  useEffect(() => {
    if (sessionRef === null) return
    setEvents([])
    setArtifact(null)
    setExpanded(null)
    setTurnText("")
    setTurnError(null)
    void pollEvents(sessionRef)
    void c
      .fetchSessionArtifact(sessionRef)
      .then((a) => setArtifact(a.kind === "none" ? null : a))
      .catch(() => {})

    // Live stream: refresh the rich timeline whenever new events are replayed.
    const sub = c.subscribeSession(sessionRef, () => void pollEvents(sessionRef))
    if (sub.streaming) {
      return () => sub.dispose()
    }

    // Fallback: no bridge stream — poll the rich events on a timer as before.
    sub.dispose()
    eventsTimer.current = setInterval(() => void pollEvents(sessionRef), POLL_MS)
    return () => {
      if (eventsTimer.current) clearInterval(eventsTimer.current)
    }
  }, [sessionRef, pollEvents, c])

  const cancellable =
    session && (session.state === "running" || session.state === "queued" || session.state === "started")

  return (
    <View style={styles.container}>
      <View style={styles.headerBar}>
        <Pressable style={styles.back} onPress={() => navigation.goBack()}>
          <Text style={styles.backText}>‹ sessions</Text>
        </Pressable>
      </View>
      <ScrollView contentContainerStyle={styles.content}>
        <Text style={styles.detailRef}>{sessionRef ?? "no session"}</Text>
        {session ? (
          (() => {
            const v = verifyText(session)
            return <Text style={[styles.verifyLine, { color: toneColor(v.tone) }]}>{v.text}</Text>
          })()
        ) : null}
        {artifact ? (
          <Text style={styles.artifactLine}>
            artifact: {artifact.outcome ?? artifact.kind}
            {artifact.editedFileCount !== null ? ` · ${artifact.editedFileCount} files` : ""}
            {artifact.commandCount !== null ? ` · ${artifact.commandCount} cmds` : ""}
            {artifact.totalTokens !== null ? ` · ${artifact.totalTokens} tok` : ""}
          </Text>
        ) : null}
        {/* G3 (#5495): open the read-only artifact/diff viewer (changed files,
            verify transcript, raw artifact) for this session. */}
        {artifact && sessionRef !== null ? (
          <Pressable
            style={styles.viewArtifactBtn}
            onPress={() => navigation.navigate("ArtifactViewer", { sessionRef })}
          >
            <Text style={styles.viewArtifactText}>View artifact & diff ›</Text>
          </Pressable>
        ) : null}
        {cancellable && sessionRef !== null ? (
          <Pressable
            style={styles.cancelBtn}
            onPress={() => {
              void c.cancelSession(sessionRef).then(() => {
                c.refresh()
                navigation.goBack()
              })
            }}
          >
            <Text style={styles.cancelText}>Cancel session</Text>
          </Pressable>
        ) : null}
        {session && sessionRef !== null ? (
          <View style={styles.turnBox}>
            <TextInput
              style={styles.turnInput}
              placeholder="Next instruction"
              placeholderTextColor={C.textSecondary}
              multiline
              value={turnText}
              onChangeText={setTurnText}
            />
            {turnError ? <Text style={styles.turnError}>{turnError}</Text> : null}
            <Pressable
              disabled={turnSending || turnText.trim().length === 0}
              style={[
                styles.turnBtn,
                turnSending || turnText.trim().length === 0 ? styles.turnBtnDisabled : null,
              ]}
              onPress={() => {
                const instruction = turnText.trim()
                if (instruction.length === 0) return
                setTurnSending(true)
                setTurnError(null)
                void c
                  .steerTurn({ sessionRef, instruction })
                  .then((childRef) => {
                    setTurnText("")
                    c.refresh()
                    navigation.navigate("SessionDetail", { sessionRef: childRef })
                  })
                  .catch((e) => setTurnError(e instanceof Error ? e.message : "turn failed"))
                  .finally(() => setTurnSending(false))
              }}
            >
              <Text style={styles.turnText}>{turnSending ? "Sending..." : "Send turn"}</Text>
            </Pressable>
          </View>
        ) : null}
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
                  <Text
                    style={[styles.rowLabel, isOpen ? styles.expandedRowLabel : null]}
                    numberOfLines={isOpen ? undefined : 2}
                  >
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
      </ScrollView>
    </View>
  )
}

const styles = StyleSheet.create({
  container: { backgroundColor: C.bg, flex: 1 },
  headerBar: {
    alignItems: "center",
    backgroundColor: C.bg,
    flexDirection: "row",
    justifyContent: "space-between",
    paddingBottom: 4,
    paddingHorizontal: 16,
    paddingTop: 50,
  },
  content: { padding: 24, paddingTop: 12 },
  card: {
    backgroundColor: C.bgSecondary,
    borderColor: C.outline,
    borderRadius: 8,
    borderWidth: 1,
    marginTop: 24,
    padding: 18,
  },
  cardBody: { color: C.textSecondary, fontSize: 14, lineHeight: 20, marginTop: 8 },
  back: { paddingVertical: 4 },
  backText: { color: C.info, fontSize: 15 },
  detailRef: { color: C.text, fontFamily: "Courier", fontSize: 13, marginTop: 10 },
  verifyLine: { fontFamily: "Courier", fontSize: 13, marginTop: 8 },
  artifactLine: { color: C.textSecondary, fontFamily: "Courier", fontSize: 12, marginTop: 4 },
  viewArtifactBtn: {
    alignSelf: "flex-start",
    borderColor: C.info,
    borderRadius: 6,
    borderWidth: 1,
    marginTop: 10,
    paddingHorizontal: 12,
    paddingVertical: 8,
  },
  viewArtifactText: { color: C.info, fontSize: 13, fontWeight: "600" },
  cancelBtn: {
    alignItems: "center",
    borderColor: C.danger,
    borderRadius: 6,
    borderWidth: 1,
    marginTop: 12,
    padding: 10,
  },
  cancelText: { color: C.danger, fontSize: 14, fontWeight: "600" },
  turnBox: {
    backgroundColor: C.bgSecondary,
    borderColor: C.outline,
    borderRadius: 8,
    borderWidth: 1,
    marginTop: 16,
    padding: 12,
  },
  turnInput: {
    color: C.text,
    fontFamily: "Courier",
    fontSize: 14,
    lineHeight: 20,
    minHeight: 76,
    textAlignVertical: "top",
  },
  turnBtn: {
    alignItems: "center",
    backgroundColor: C.info,
    borderRadius: 6,
    marginTop: 10,
    padding: 10,
  },
  turnBtnDisabled: { opacity: 0.45 },
  turnText: { color: C.bg, fontSize: 14, fontWeight: "700" },
  turnError: { color: C.danger, fontSize: 12, marginTop: 8 },
  eventRow: {
    alignItems: "center",
    backgroundColor: C.bgSecondary,
    borderColor: C.outline,
    borderRadius: 6,
    borderWidth: 1,
    flexDirection: "row",
    marginTop: 8,
    padding: 12,
  },
  dot: { borderRadius: 6, height: 12, marginRight: 12, width: 12 },
  rowText: { flex: 1 },
  rowLabel: { color: C.text, fontFamily: "Courier", fontSize: 13, lineHeight: 18 },
  expandedRowLabel: { height: undefined },
  rowStatus: { color: C.textSecondary, fontSize: 12, marginTop: 2 },
})
