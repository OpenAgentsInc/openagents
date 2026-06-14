import { useState } from "react"
import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native"
import { useNavigation } from "@react-navigation/native"
import { CANONICAL_DARK } from "@openagentsinc/autopilot-control-protocol"
import type { ControlSessionRow } from "../src/control/control-client"
import { useConnection } from "../src/connection/ConnectionContext"

const C = CANONICAL_DARK

type Filter = "all" | "running" | "queued" | "completed" | "failed" | "cancelled"
const FILTERS: Filter[] = ["all", "running", "queued", "completed", "failed", "cancelled"]

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

export default function SessionsScreen() {
  const c = useConnection()
  const navigation = useNavigation<{ navigate: (route: string, params?: object) => void }>()
  const [selected, setSelected] = useState<Filter>("all")

  const sessions: ControlSessionRow[] = c.sessions
  const filtered =
    selected === "all" ? sessions : sessions.filter((s) => s.state === selected)

  const childrenOf = (ref: string) => filtered.filter((s) => s.parentRef === ref)
  const isTop = (s: ControlSessionRow) =>
    !s.parentRef || !sessions.some((p) => p.sessionRef === s.parentRef)

  const renderRow = (s: ControlSessionRow, child: boolean) => (
    <Pressable
      key={s.sessionRef}
      style={[styles.row, child ? styles.childRow : null]}
      onPress={() => navigation.navigate("SessionDetail", { sessionRef: s.sessionRef })}
    >
      <View style={[styles.dot, { backgroundColor: stateTone(s.state) }]} />
      <View style={styles.rowText}>
        <Text style={styles.rowLabel} numberOfLines={2}>
          {child ? "↳ " : ""}
          {s.latestActivity || s.state}
        </Text>
        <Text style={styles.rowStatus}>
          {s.agentKind ? `${s.agentKind} · ` : ""}
          {s.sessionRef.slice(-6)}
        </Text>
      </View>
      <Text style={styles.chevron}>›</Text>
    </Pressable>
  )

  const rows: React.ReactNode[] = []
  for (const s of filtered.filter(isTop)) {
    rows.push(renderRow(s, false))
    for (const child of childrenOf(s.sessionRef)) rows.push(renderRow(child, true))
  }

  return (
    <View style={styles.container}>
      <ScrollView contentContainerStyle={styles.scroll}>
        <Text style={styles.title}>Sessions</Text>

        {/* Filter chips */}
        <View style={styles.filterBar}>
          {FILTERS.map((f) => {
            const active = f === selected
            return (
              <Pressable
                key={f}
                style={[styles.chip, active ? styles.chipActive : null]}
                onPress={() => setSelected(f)}
              >
                <Text style={[styles.chipText, active ? styles.chipTextActive : null]}>
                  {f === "all" ? "All" : f}
                </Text>
              </Pressable>
            )
          })}
        </View>

        {/* Session rows */}
        {rows.length === 0 ? (
          <View style={styles.emptyCard}>
            <Text style={styles.emptyText}>
              {c.status === "connected"
                ? selected === "all"
                  ? "No sessions yet."
                  : `No ${selected} sessions.`
                : "Connecting…"}
            </Text>
          </View>
        ) : (
          rows
        )}
      </ScrollView>
    </View>
  )
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: C.bg,
  },
  scroll: {
    paddingTop: 50,
    paddingHorizontal: 16,
    paddingBottom: 32,
  },
  title: {
    fontSize: 22,
    fontWeight: "700",
    color: C.text,
    marginBottom: 12,
  },
  filterBar: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 6,
    marginBottom: 14,
  },
  chip: {
    paddingVertical: 4,
    paddingHorizontal: 10,
    borderRadius: 14,
    borderWidth: 1,
    borderColor: C.outline,
    backgroundColor: "transparent",
  },
  chipActive: {
    borderColor: C.primary,
    backgroundColor: C.bgSecondary,
  },
  chipText: {
    fontSize: 12,
    color: C.textSecondary,
  },
  chipTextActive: {
    color: C.primary,
  },
  row: {
    flexDirection: "row",
    alignItems: "center",
    paddingVertical: 10,
    paddingHorizontal: 12,
    borderRadius: 6,
    backgroundColor: C.bgSecondary,
    marginBottom: 6,
  },
  childRow: {
    marginLeft: 20,
  },
  dot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    marginRight: 10,
    flexShrink: 0,
  },
  rowText: {
    flex: 1,
  },
  rowLabel: {
    fontSize: 13,
    color: C.text,
    lineHeight: 18,
  },
  rowStatus: {
    fontSize: 11,
    color: C.textSecondary,
    marginTop: 2,
  },
  chevron: {
    fontSize: 18,
    color: C.textSecondary,
    marginLeft: 6,
  },
  emptyCard: {
    padding: 16,
    borderRadius: 6,
    backgroundColor: C.bgSecondary,
    alignItems: "center",
  },
  emptyText: {
    color: C.textSecondary,
    fontSize: 13,
  },
})
