import type { LiveAgentGraphEntity } from "@openagentsinc/khala-sync"
import {
  newestLiveAgentGraph,
  projectLiveAgentGraphPresentation,
  resolveLiveAgentGraphSelection,
  type LiveAgentGraphPresentationRow,
  type LiveAgentGraphTone,
} from "@openagentsinc/khala-sync-client"
import { useEffect, useMemo, useState } from "react"
import { Pressable, StyleSheet, type TextStyle, View, type ViewStyle } from "react-native"

import { khalaMobileTheme } from "../theme/tokens"
import { KhalaText } from "./khala-text"

export type LiveAgentGraphPanelAction = Readonly<{
  kind: "focus_agent" | "inspect_agent"
  graphRef: string
  agentRef: string
}>

export type LiveAgentGraphPanelProps = Readonly<{
  graphs: ReadonlyArray<LiveAgentGraphEntity>
  phase: "loading" | "ready" | "error"
  authority?: "live" | "historical"
  nowMs?: number
  onAction?: (action: LiveAgentGraphPanelAction) => void
}>

const toneColor = (tone: LiveAgentGraphTone): string => {
  if (tone === "active") return khalaMobileTheme.accent
  if (tone === "attention") return khalaMobileTheme.warning
  if (tone === "success") return khalaMobileTheme.success
  if (tone === "danger") return khalaMobileTheme.danger
  return khalaMobileTheme.textMuted
}

const phaseLabel = (phase: LiveAgentGraphPanelProps["phase"]): string =>
  phase === "ready" ? "Confirmed" : phase === "loading" ? "Loading" : "Unavailable"

const detailLines = (row: LiveAgentGraphPresentationRow): ReadonlyArray<string> => [
  `${row.providerLabel} · ${row.runtimeLabel}`,
  `${row.sessionLabel} · ${row.worktreeLabel}`,
  row.elapsedLabel,
  ...(row.toolLabel === null ? [] : [row.toolLabel]),
  ...(row.attentionLabel === null ? [] : [row.attentionLabel]),
  ...(row.terminalLabel === null ? [] : [row.terminalLabel]),
]

export const LiveAgentGraphPanel = ({
  authority = "live",
  graphs,
  nowMs,
  onAction,
  phase,
}: LiveAgentGraphPanelProps) => {
  const graph = useMemo(() => newestLiveAgentGraph(graphs), [graphs])
  const presentation = useMemo(
    () => graph === null
      ? null
      : projectLiveAgentGraphPresentation(graph, {
          authority,
          maxRows: 40,
          ...(nowMs === undefined ? {} : { nowMs }),
        }),
    [authority, graph, nowMs],
  )
  const [expanded, setExpanded] = useState(() => (graph?.nodes.length ?? 0) <= 4)
  const [selectedAgentRef, setSelectedAgentRef] = useState<string | null>(null)

  useEffect(() => {
    if (presentation === null) {
      setSelectedAgentRef(null)
      return
    }
    setSelectedAgentRef(current => resolveLiveAgentGraphSelection(presentation, current))
    if (presentation.attentionCount > 0) setExpanded(true)
  }, [presentation])

  if (presentation === null) return null

  const summary = `${presentation.totalCount} agent${presentation.totalCount === 1 ? "" : "s"} · ${presentation.activeCount} active` +
    (presentation.attentionCount === 0 ? "" : ` · ${presentation.attentionCount} need attention`)
  const authorityLabel = authority === "historical"
    ? "Historical import · controls unavailable"
    : phaseLabel(phase)

  return (
    <View accessibilityLabel="Live agent graph" style={styles.container}>
      <Pressable
        accessibilityLabel={`${authorityLabel} agent graph. ${summary}. ${expanded ? "Collapse" : "Expand"}.`}
        accessibilityRole="button"
        accessibilityState={{ expanded }}
        onPress={() => setExpanded(value => !value)}
        style={({ pressed }) => [styles.summaryButton, pressed ? styles.pressed : null]}
      >
        <View style={styles.summaryCopy}>
          <View style={styles.summaryTitleRow}>
            <View style={[styles.liveDot, { backgroundColor: authority === "live" ? khalaMobileTheme.accent : khalaMobileTheme.textMuted }]} />
            <KhalaText className="font-medium" variant="body">Agent stack</KhalaText>
            <KhalaText style={styles.authorityLabel} variant="muted">{authorityLabel}</KhalaText>
          </View>
          <KhalaText numberOfLines={1} style={styles.summaryMeta} variant="muted">{summary}</KhalaText>
        </View>
        <KhalaText accessibilityElementsHidden style={styles.disclosure}>{expanded ? "−" : "+"}</KhalaText>
      </Pressable>

      {expanded ? (
        <View accessibilityRole="list" style={styles.rows}>
          {presentation.rows.map(row => {
            const selected = row.agentRef === selectedAgentRef
            return (
              <View key={row.agentRef}>
                <Pressable
                  accessibilityLabel={`${row.label}. ${row.statusLabel}. ${detailLines(row).join(". ")}. Tap to inspect.`}
                  accessibilityRole="button"
                  accessibilityState={{ selected }}
                  onPress={() => {
                    setSelectedAgentRef(row.agentRef)
                    onAction?.({ kind: "inspect_agent", graphRef: row.graphRef, agentRef: row.agentRef })
                  }}
                  style={({ pressed }) => [
                    styles.agentRow,
                    { marginLeft: Math.min(row.depth, 5) * 16 },
                    selected ? styles.agentRowSelected : null,
                    pressed ? styles.pressed : null,
                  ]}
                >
                  <View style={[styles.statusDot, { backgroundColor: toneColor(row.tone) }]} />
                  <View style={styles.agentCopy}>
                    <KhalaText className="font-medium" numberOfLines={1} variant="body">{row.label}</KhalaText>
                    <KhalaText numberOfLines={1} style={styles.agentMeta} variant="muted">
                      {`${row.statusLabel} · ${row.toolLabel ?? row.elapsedLabel}`}
                    </KhalaText>
                  </View>
                </Pressable>
                {selected ? (
                  <View style={[styles.inspector, { marginLeft: Math.min(row.depth, 5) * 16 + 16 }]}>
                    {detailLines(row).map(line => (
                      <KhalaText key={line} style={styles.detailLine} variant="muted">{line}</KhalaText>
                    ))}
                    {row.canControl && onAction !== undefined ? (
                      <Pressable
                        accessibilityLabel={`Focus ${row.label}`}
                        accessibilityRole="button"
                        onPress={() => onAction({ kind: "focus_agent", graphRef: row.graphRef, agentRef: row.agentRef })}
                        style={({ pressed }) => [styles.focusButton, pressed ? styles.pressed : null]}
                      >
                        <KhalaText className="font-medium" style={styles.focusLabel}>Focus agent</KhalaText>
                      </Pressable>
                    ) : null}
                  </View>
                ) : null}
              </View>
            )
          })}
          {presentation.hiddenCount > 0 ? (
            <KhalaText accessibilityLiveRegion="polite" style={styles.overflowLabel} variant="muted">
              {`${presentation.hiddenCount} more agents hidden by the mobile safety bound`}
            </KhalaText>
          ) : null}
        </View>
      ) : null}
    </View>
  )
}

const styles = StyleSheet.create({
  agentCopy: { flex: 1, minWidth: 0 } satisfies ViewStyle,
  agentMeta: { color: khalaMobileTheme.textMuted, marginTop: 2 } satisfies TextStyle,
  agentRow: {
    alignItems: "center",
    borderRadius: 8,
    flexDirection: "row",
    gap: 10,
    minHeight: 48,
    paddingHorizontal: 10,
    paddingVertical: 7,
  } satisfies ViewStyle,
  agentRowSelected: { backgroundColor: khalaMobileTheme.surfaceActive } satisfies ViewStyle,
  authorityLabel: { color: khalaMobileTheme.textSoft, marginLeft: 4 } satisfies TextStyle,
  container: {
    backgroundColor: khalaMobileTheme.surface,
    borderColor: khalaMobileTheme.border,
    borderRadius: 12,
    borderWidth: 1,
    marginBottom: 8,
    marginHorizontal: 16,
    overflow: "hidden",
  } satisfies ViewStyle,
  detailLine: { color: khalaMobileTheme.textSoft } satisfies TextStyle,
  disclosure: { color: khalaMobileTheme.accent, fontSize: 22, lineHeight: 24 } satisfies TextStyle,
  focusButton: {
    alignSelf: "flex-start",
    borderColor: khalaMobileTheme.accent,
    borderRadius: 8,
    borderWidth: 1,
    marginTop: 4,
    minHeight: 38,
    justifyContent: "center",
    paddingHorizontal: 12,
  } satisfies ViewStyle,
  focusLabel: { color: khalaMobileTheme.accent } satisfies TextStyle,
  inspector: {
    borderColor: khalaMobileTheme.borderMuted,
    borderRadius: 8,
    borderWidth: 1,
    gap: 3,
    marginBottom: 6,
    marginRight: 10,
    padding: 10,
  } satisfies ViewStyle,
  liveDot: { borderRadius: 4, height: 8, width: 8 } satisfies ViewStyle,
  overflowLabel: { color: khalaMobileTheme.textMuted, padding: 10, textAlign: "center" } satisfies TextStyle,
  pressed: { opacity: 0.72 } satisfies ViewStyle,
  rows: { borderTopColor: khalaMobileTheme.borderMuted, borderTopWidth: 1, padding: 6 } satisfies ViewStyle,
  statusDot: { borderRadius: 5, height: 10, width: 10 } satisfies ViewStyle,
  summaryButton: {
    alignItems: "center",
    flexDirection: "row",
    minHeight: 60,
    paddingHorizontal: 12,
    paddingVertical: 9,
  } satisfies ViewStyle,
  summaryCopy: { flex: 1, minWidth: 0 } satisfies ViewStyle,
  summaryMeta: { color: khalaMobileTheme.textMuted, marginTop: 3 } satisfies TextStyle,
  summaryTitleRow: { alignItems: "center", flexDirection: "row", gap: 7 } satisfies ViewStyle,
})
