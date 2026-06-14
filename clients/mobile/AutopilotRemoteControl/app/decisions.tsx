// CL-56: "Decisions" screen — cross-session approvals/decisions queue.
//
// Renders the full pending-approvals queue from the shared ConnectionContext.
// Each card shows the approval prompt (or kind fallback) with Approve/Deny
// buttons. Resolving removes the item optimistically via c.resolveApproval.

import { Pressable, ScrollView, StyleSheet, Text, View } from "react-native"
import { CANONICAL_DARK } from "@openagentsinc/autopilot-control-protocol"
import { useConnection } from "../src/connection/ConnectionContext"

const C = CANONICAL_DARK

export default function DecisionsScreen() {
  const c = useConnection()
  const { approvals } = c

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.title}>Decisions</Text>

      {approvals.length === 0 ? (
        <View style={styles.emptyCard}>
          <Text style={styles.emptyText}>Nothing needs you right now.</Text>
        </View>
      ) : (
        approvals.map((a) => {
          const label = a.prompt.trim() !== "" ? a.prompt : a.kind
          const resolve = (decision: "approve" | "deny") => {
            void c.resolveApproval({ approvalRef: a.approvalRef, decision })
          }
          return (
            <View key={a.approvalRef} style={[styles.card, styles.approvalCard]}>
              <Text style={styles.acctText} numberOfLines={2}>
                {label}
              </Text>
              <View style={styles.approvalButtons}>
                <Pressable style={[styles.approvalBtn, styles.approveBtn]} onPress={() => resolve("approve")}>
                  <Text style={styles.approvalBtnText}>Approve</Text>
                </Pressable>
                <Pressable style={[styles.approvalBtn, styles.denyBtn]} onPress={() => resolve("deny")}>
                  <Text style={styles.approvalBtnText}>Deny</Text>
                </Pressable>
              </View>
            </View>
          )
        })
      )}
    </ScrollView>
  )
}

const styles = StyleSheet.create({
  container: { backgroundColor: C.bg, flex: 1 },
  content: { padding: 24, paddingTop: 50 },
  title: { color: C.primary, fontSize: 22, fontWeight: "700" },
  emptyCard: { backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 8, borderWidth: 1, marginTop: 24, padding: 18 },
  emptyText: { color: C.textSecondary, fontSize: 14 },
  card: { backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 8, borderWidth: 1, marginTop: 24, padding: 18 },
  approvalCard: { borderColor: C.warning, borderWidth: 1 },
  acctText: { color: C.text, fontFamily: "Courier", fontSize: 13 },
  approvalButtons: { flexDirection: "row", gap: 10, marginTop: 8 },
  approvalBtn: { borderRadius: 6, paddingHorizontal: 16, paddingVertical: 8 },
  approveBtn: { backgroundColor: C.success },
  denyBtn: { backgroundColor: C.danger },
  approvalBtnText: { color: C.bg, fontWeight: "700" },
})
