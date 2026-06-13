import { useMemo } from "react"
import { ScrollView, StyleSheet, Text, View } from "react-native"

import {
  type NodeStatus,
  type NodeStatusRowTone,
  nodeStatusRowsViewModel,
} from "../src/parity/node-status-view-model"

// Dark-mode tokens mirrored from the openagents.com website (CL-42).
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

const toneColor = (tone: NodeStatusRowTone): string =>
  tone === "success"
    ? C.success
    : tone === "danger"
      ? C.danger
      : tone === "warning"
        ? C.warning
        : tone === "info"
          ? C.info
          : C.outline

// expo-updates is only present in release/dev-client builds; guard so web/Expo Go
// don't crash. Shows whether this screen is running an embedded build or an OTA.
function useUpdateBadge(): string {
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const Updates = require("expo-updates") as {
      isEmbeddedLaunch: boolean
      updateId: string | null
      channel: string | null
    }
    if (Updates.isEmbeddedLaunch) return "embedded build (no OTA yet)"
    const id = Updates.updateId ? Updates.updateId.slice(0, 8) : "unknown"
    return `OTA update ${id}${Updates.channel ? ` · ${Updates.channel}` : ""}`
  } catch {
    return "dev"
  }
}

export default function NodesScreen() {
  const updateBadge = useMemo(useUpdateBadge, [])

  // No bridge/pairing yet, so there is no live node to show. Honest empty state.
  // (When a node pairs, feed real NodeStatus[] here and the list renders.)
  const nodes: NodeStatus[] = []
  const rows = useMemo(() => nodeStatusRowsViewModel(nodes), [nodes])

  return (
    <View style={styles.container}>
      <ScrollView contentContainerStyle={styles.content}>
        <Text style={styles.h1}>Autopilot Remote Control</Text>
        <Text style={styles.subtitle}>Nodes</Text>

        {rows.length === 0 ? (
          <View style={styles.card}>
            <Text style={styles.cardTitle}>No node paired yet</Text>
            <Text style={styles.cardBody}>
              Pair this device with a Pylon node to see live sessions, decisions,
              and status here. Secure bridge pairing is coming next.
            </Text>
          </View>
        ) : (
          rows.map((row) => (
            <View key={row.nodeRef} style={styles.row}>
              <View style={[styles.dot, { backgroundColor: toneColor(row.tone) }]} />
              <View style={styles.rowText}>
                <Text style={styles.rowLabel}>{row.label}</Text>
                <Text style={styles.rowStatus}>{row.statusLabel}</Text>
              </View>
            </View>
          ))
        )}
      </ScrollView>

      <View style={styles.footer}>
        <Text style={styles.footerText}>{updateBadge}</Text>
      </View>
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
  rowLabel: { color: C.text, fontFamily: "Courier", fontSize: 14 },
  rowStatus: { color: C.textSecondary, fontSize: 12, marginTop: 2 },
  footer: {
    borderTopColor: C.outline,
    borderTopWidth: 1,
    padding: 14,
    paddingBottom: 28,
  },
  footerText: {
    color: C.textSecondary,
    fontFamily: "Courier",
    fontSize: 12,
    textAlign: "center",
  },
})
