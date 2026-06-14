import { useState } from "react"
import { Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native"
import { useNavigation } from "@react-navigation/native"

import { CANONICAL_DARK, validateSpawnRequest } from "@openagentsinc/autopilot-control-protocol"
import { useConnection } from "../src/connection/ConnectionContext"

const C = CANONICAL_DARK

type Adapter = "codex" | "claude_agent"

export default function SpawnScreen() {
  const c = useConnection()
  const navigation = useNavigation<{ navigate: (route: string, params?: Record<string, unknown>) => void }>()

  const [adapter, setAdapter] = useState<Adapter>("codex")
  const [objective, setObjective] = useState("")
  const [verifyText, setVerifyText] = useState("")
  const [submitStatus, setSubmitStatus] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)

  const handleSubmit = () => {
    setSubmitStatus(null)

    const v = validateSpawnRequest({ adapter, objective })
    if (!v.ok) {
      setSubmitStatus(v.errors[0] ?? "invalid request")
      return
    }

    const verify = verifyText
      .split("\n")
      .map((l) => l.trim())
      .filter((l) => l.length > 0)

    setIsSubmitting(true)
    setSubmitStatus("spawning…")

    void c
      .spawnSession({
        adapter: v.adapter as Adapter,
        objective: v.objective,
        verify: verify.length > 0 ? verify : undefined,
      })
      .then((sessionRef) => {
        setIsSubmitting(false)
        setSubmitStatus(null)
        setObjective("")
        setVerifyText("")
        navigation.navigate("SessionDetail", { sessionRef })
      })
      .catch((e: unknown) => {
        setIsSubmitting(false)
        setSubmitStatus(e instanceof Error ? e.message : "spawn failed")
      })
  }

  const disconnected = c.status !== "connected"

  return (
    <ScrollView style={styles.container} contentContainerStyle={styles.content}>
      <Text style={styles.h1}>Spawn</Text>
      <Text style={styles.subtitle}>New bounded session</Text>

      <View style={styles.card}>
        <Text style={styles.cardTitle}>Adapter</Text>
        <View style={styles.toggleRow}>
          <Pressable
            style={[styles.toggleBtn, adapter === "codex" && styles.toggleBtnActive]}
            onPress={() => setAdapter("codex")}
          >
            <Text style={[styles.toggleText, adapter === "codex" && styles.toggleTextActive]}>
              codex
            </Text>
          </Pressable>
          <Pressable
            style={[styles.toggleBtn, adapter === "claude_agent" && styles.toggleBtnActive]}
            onPress={() => setAdapter("claude_agent")}
          >
            <Text style={[styles.toggleText, adapter === "claude_agent" && styles.toggleTextActive]}>
              claude_agent
            </Text>
          </Pressable>
        </View>

        <Text style={styles.cardTitle}>Objective</Text>
        <TextInput
          style={[styles.input, styles.inputMultiline]}
          placeholder="Describe the session objective…"
          placeholderTextColor={C.textSecondary}
          value={objective}
          onChangeText={setObjective}
          multiline
          editable={!isSubmitting}
        />

        <Text style={styles.cardTitle}>Verify commands (optional)</Text>
        <Text style={styles.hint}>One shell command per line — run after the session completes.</Text>
        <TextInput
          style={[styles.input, styles.inputMultiline]}
          placeholder={"bun test\nbun run typecheck"}
          placeholderTextColor={C.textSecondary}
          value={verifyText}
          onChangeText={setVerifyText}
          multiline
          editable={!isSubmitting}
          autoCapitalize="none"
          autoCorrect={false}
        />

        {disconnected && !isSubmitting ? (
          <Text style={styles.warnLine}>Not connected to a node.</Text>
        ) : null}

        {submitStatus ? (
          <Text style={[styles.statusLine, isSubmitting ? styles.statusPending : styles.statusError]}>
            {submitStatus}
          </Text>
        ) : null}

        <Pressable
          style={[styles.button, (isSubmitting || disconnected) && styles.buttonDisabled]}
          onPress={handleSubmit}
          disabled={isSubmitting || disconnected}
        >
          <Text style={styles.buttonText}>
            {isSubmitting ? "Spawning…" : "Spawn session"}
          </Text>
        </Pressable>
      </View>
    </ScrollView>
  )
}

const styles = StyleSheet.create({
  container: { backgroundColor: C.bg, flex: 1 },
  content: { padding: 24, paddingTop: 50 },
  h1: { color: C.primary, fontSize: 22, fontWeight: "700" },
  subtitle: { color: C.textSecondary, fontSize: 13, letterSpacing: 1, marginTop: 4, textTransform: "uppercase" },
  card: { backgroundColor: C.bgSecondary, borderColor: C.outline, borderRadius: 8, borderWidth: 1, marginTop: 24, padding: 18 },
  cardTitle: { color: C.primary, fontSize: 14, fontWeight: "600", marginBottom: 8, marginTop: 14 },
  hint: { color: C.textSecondary, fontSize: 12, marginBottom: 6 },
  input: {
    backgroundColor: C.bg,
    borderColor: C.outline,
    borderRadius: 6,
    borderWidth: 1,
    color: C.text,
    fontFamily: "Courier",
    fontSize: 13,
    marginBottom: 6,
    padding: 12,
  },
  inputMultiline: { minHeight: 80, textAlignVertical: "top" },
  toggleRow: { flexDirection: "row", gap: 10, marginBottom: 6 },
  toggleBtn: {
    borderColor: C.outline,
    borderRadius: 6,
    borderWidth: 1,
    paddingHorizontal: 16,
    paddingVertical: 8,
  },
  toggleBtnActive: { borderColor: C.primary, backgroundColor: C.primary },
  toggleText: { color: C.textSecondary, fontFamily: "Courier", fontSize: 13 },
  toggleTextActive: { color: C.bg, fontWeight: "700" },
  button: {
    alignItems: "center",
    backgroundColor: C.primary,
    borderRadius: 6,
    marginTop: 16,
    padding: 14,
  },
  buttonDisabled: { opacity: 0.4 },
  buttonText: { color: C.bg, fontSize: 15, fontWeight: "700" },
  statusLine: { fontSize: 13, marginTop: 10 },
  statusError: { color: C.danger },
  statusPending: { color: C.textSecondary },
  warnLine: { color: C.warning, fontFamily: "Courier", fontSize: 12, marginTop: 8 },
})
