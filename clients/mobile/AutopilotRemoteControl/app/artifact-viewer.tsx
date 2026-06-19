// G3 (EPIC #5492 / #5495): read-only artifact / diff viewer. Promotes the
// session-detail artifact row from "stats only" to a real viewer of what the
// agent actually did. It reads the retained proof/failure artifact over the
// bridge `artifact.read` verb (read_artifact capability; dev-token
// session.artifact fallback) via useConnection().fetchSessionArtifactContent,
// projects it with the shared protocol projectArtifactContentView, and renders:
//   - the changed-file list (the node's projection-safe "diff" view),
//   - the dev-check command transcript (verify pass/fail with exit codes),
//   - deviations + failure error class/ref,
//   - the verbatim pretty-printed artifact JSON (toggle).
// The node redaction-scans the artifact at write time, so everything here is
// projection-safe; this screen never mutates node state.

import { useEffect, useState } from "react"
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native"
import { useNavigation, useRoute } from "@react-navigation/native"

import { useConnection } from "../src/connection/ConnectionContext"
import { CANONICAL_DARK, type ArtifactContentView } from "@openagentsinc/autopilot-control-protocol"

const C = CANONICAL_DARK

const statusTone = (status: string): string =>
  status === "passed" || status === "completed" || status === "ok"
    ? C.success
    : status === "failed" || status === "error"
      ? C.danger
      : C.textSecondary

// Per-file diff-status glyph. The node ships file-level status, not raw diff
// bytes, so the "diff" surface is the changed-file set with +/-/~ markers.
const fileMark = (status: string): string =>
  status === "added"
    ? "+"
    : status === "deleted"
      ? "-"
      : status === "renamed"
        ? "»"
        : status === "untracked"
          ? "?"
          : "~"

const fileTone = (status: string): string =>
  status === "added"
    ? C.success
    : status === "deleted"
      ? C.danger
      : status === "untracked"
        ? C.warning
        : C.info

type LoadState = "loading" | "ready" | "empty" | "error"

export default function ArtifactViewerScreen() {
  const route = useRoute()
  const sessionRef = (route.params as { sessionRef?: string } | undefined)?.sessionRef ?? null
  const navigation = useNavigation<{ goBack: () => void }>()
  const c = useConnection()

  const [view, setView] = useState<ArtifactContentView | null>(null)
  const [load, setLoad] = useState<LoadState>("loading")
  const [showRaw, setShowRaw] = useState(false)

  useEffect(() => {
    if (sessionRef === null) {
      setLoad("empty")
      return
    }
    let cancelled = false
    setLoad("loading")
    setView(null)
    c.fetchSessionArtifactContent(sessionRef)
      .then((v) => {
        if (cancelled) return
        if (v === null) {
          setLoad("empty")
          return
        }
        setView(v)
        setLoad("ready")
      })
      .catch(() => {
        if (!cancelled) setLoad("error")
      })
    return () => {
      cancelled = true
    }
  }, [sessionRef, c])

  return (
    <View style={styles.container}>
      <View style={styles.headerBar}>
        <Pressable style={styles.back} onPress={() => navigation.goBack()}>
          <Text style={styles.backText}>‹ session</Text>
        </Pressable>
      </View>
      <ScrollView contentContainerStyle={styles.content}>
        <Text style={styles.detailRef}>{sessionRef ?? "no session"}</Text>

        {load === "loading" ? (
          <View style={styles.card}>
            <Text style={styles.cardBody}>Loading artifact…</Text>
          </View>
        ) : null}

        {load === "empty" ? (
          <View style={styles.card}>
            <Text style={styles.cardBody}>No retained artifact for this session yet.</Text>
          </View>
        ) : null}

        {load === "error" ? (
          <View style={styles.card}>
            <Text style={[styles.cardBody, { color: C.danger }]}>Could not read the artifact.</Text>
          </View>
        ) : null}

        {load === "ready" && view !== null ? (
          <>
            <Text style={[styles.kindLine, { color: statusTone(view.outcome ?? view.kind) }]}>
              {view.kind} · {view.outcome ?? "—"}
              {view.devCheckState !== null ? ` · check: ${view.devCheckState}` : ""}
            </Text>
            {view.schemaRef !== null ? <Text style={styles.schemaLine}>{view.schemaRef}</Text> : null}
            {view.dirtySummary !== null ? <Text style={styles.schemaLine}>working tree: {view.dirtySummary}</Text> : null}

            {view.errorClass !== null ? (
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>FAILURE</Text>
                <Text style={[styles.rowLabel, { color: C.danger }]}>{view.errorClass}</Text>
                {view.errorDigestRef !== null ? (
                  <Text style={styles.rowStatus}>{view.errorDigestRef}</Text>
                ) : null}
              </View>
            ) : null}

            <View style={styles.section}>
              <Text style={styles.sectionTitle}>CHANGED FILES ({view.changedFiles.length})</Text>
              {view.changedFiles.length === 0 ? (
                <Text style={styles.cardBody}>No file changes recorded.</Text>
              ) : (
                view.changedFiles.map((f, i) => (
                  <View key={`${f.fileRef}.${i}`} style={styles.fileRow}>
                    <Text style={[styles.fileMark, { color: fileTone(f.status) }]}>{fileMark(f.status)}</Text>
                    <View style={styles.rowText}>
                      <Text style={styles.rowLabel} numberOfLines={2}>
                        {f.fileRef}
                      </Text>
                      <Text style={styles.rowStatus}>
                        {f.status}
                        {f.area !== null ? ` · ${f.area}` : ""}
                        {f.extension !== null ? ` · .${f.extension}` : ""}
                      </Text>
                    </View>
                  </View>
                ))
              )}
            </View>

            <View style={styles.section}>
              <Text style={styles.sectionTitle}>VERIFY ({view.commandResults.length})</Text>
              {view.commandResults.length === 0 ? (
                <Text style={styles.cardBody}>No verify commands recorded.</Text>
              ) : (
                view.commandResults.map((cmd, i) => (
                  <View key={`${cmd.commandRef}.${i}`} style={styles.eventRow}>
                    <View style={[styles.dot, { backgroundColor: statusTone(cmd.status) }]} />
                    <View style={styles.rowText}>
                      <Text style={styles.rowLabel} numberOfLines={2}>
                        {cmd.reasonRef ?? cmd.commandRef}
                      </Text>
                      <Text style={styles.rowStatus}>
                        {cmd.status}
                        {cmd.exitCode !== null ? ` · exit ${cmd.exitCode}` : ""}
                        {cmd.durationMs !== null ? ` · ${cmd.durationMs}ms` : ""}
                        {cmd.stdoutBytes !== null ? ` · out ${cmd.stdoutBytes}b` : ""}
                        {cmd.stderrBytes !== null && cmd.stderrBytes > 0 ? ` · err ${cmd.stderrBytes}b` : ""}
                      </Text>
                    </View>
                  </View>
                ))
              )}
            </View>

            {view.deviations.length > 0 ? (
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>DEVIATIONS</Text>
                {view.deviations.map((d, i) => (
                  <Text key={`${d}.${i}`} style={[styles.rowLabel, { color: C.warning }]}>
                    {d}
                  </Text>
                ))}
              </View>
            ) : null}

            <Pressable style={styles.rawToggle} onPress={() => setShowRaw((s) => !s)}>
              <Text style={styles.rawToggleText}>{showRaw ? "▾ hide raw artifact" : "▸ show raw artifact"}</Text>
            </Pressable>
            {showRaw ? (
              <View style={styles.rawCard}>
                <Text style={styles.rawBody} selectable>
                  {view.body}
                </Text>
              </View>
            ) : null}
          </>
        ) : null}
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
  back: { paddingVertical: 4 },
  backText: { color: C.info, fontSize: 15 },
  detailRef: { color: C.text, fontFamily: "Courier", fontSize: 13, marginTop: 10 },
  kindLine: { fontFamily: "Courier", fontSize: 14, fontWeight: "600", marginTop: 10 },
  schemaLine: { color: C.textSecondary, fontFamily: "Courier", fontSize: 11, marginTop: 4 },
  card: {
    backgroundColor: C.bgSecondary,
    borderColor: C.outline,
    borderRadius: 8,
    borderWidth: 1,
    marginTop: 20,
    padding: 18,
  },
  cardBody: { color: C.textSecondary, fontSize: 14, lineHeight: 20 },
  section: { marginTop: 22 },
  sectionTitle: { color: C.textSecondary, fontSize: 12, fontWeight: "700", letterSpacing: 1, marginBottom: 8 },
  fileRow: {
    alignItems: "center",
    backgroundColor: C.bgSecondary,
    borderColor: C.outline,
    borderRadius: 6,
    borderWidth: 1,
    flexDirection: "row",
    marginTop: 8,
    padding: 12,
  },
  fileMark: { fontFamily: "Courier", fontSize: 16, fontWeight: "700", marginRight: 12, width: 14 },
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
  rowStatus: { color: C.textSecondary, fontSize: 12, marginTop: 2 },
  rawToggle: { marginTop: 24, paddingVertical: 6 },
  rawToggleText: { color: C.info, fontSize: 14 },
  rawCard: {
    backgroundColor: C.bgSecondary,
    borderColor: C.outline,
    borderRadius: 8,
    borderWidth: 1,
    marginTop: 8,
    padding: 14,
  },
  rawBody: { color: C.text, fontFamily: "Courier", fontSize: 11, lineHeight: 16 },
})
