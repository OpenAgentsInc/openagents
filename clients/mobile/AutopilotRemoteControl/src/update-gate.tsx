import { useEffect, useRef, useState } from "react"
import { ActivityIndicator, StyleSheet, Text, View } from "react-native"

// Polls the EAS update server every few seconds; when a new update is found it
// downloads it and auto-reloads, showing a progress overlay. JS-only (OTA-safe).
// expo-updates is only present in release/dev-client builds — guarded so web /
// Expo Go don't crash (there it stays idle).

type Phase = "idle" | "checking" | "downloading" | "reloading"

const POLL_MS = 5000

type ExpoUpdates = {
  checkForUpdateAsync: () => Promise<{ isAvailable: boolean }>
  fetchUpdateAsync: () => Promise<{ isNew: boolean }>
  reloadAsync: () => Promise<void>
  isEmbeddedLaunch?: boolean
}

function loadUpdates(): ExpoUpdates | null {
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const U = require("expo-updates") as ExpoUpdates & { isEnabled?: boolean }
    if (U.isEnabled === false) return null
    return U
  } catch {
    return null
  }
}

export function UpdateGate() {
  const [phase, setPhase] = useState<Phase>("idle")
  const busy = useRef(false)

  useEffect(() => {
    const U = loadUpdates()
    if (U === null) return

    let cancelled = false

    const tick = async (): Promise<void> => {
      if (busy.current || cancelled) return
      busy.current = true
      try {
        setPhase("checking")
        const check = await U.checkForUpdateAsync()
        if (cancelled) return
        if (check.isAvailable) {
          setPhase("downloading")
          const fetched = await U.fetchUpdateAsync()
          if (cancelled) return
          if (fetched.isNew) {
            setPhase("reloading")
            await U.reloadAsync() // app restarts into the new bundle
            return
          }
        }
        setPhase("idle")
      } catch {
        setPhase("idle")
      } finally {
        busy.current = false
      }
    }

    void tick()
    const id = setInterval(() => void tick(), POLL_MS)
    return () => {
      cancelled = true
      clearInterval(id)
    }
  }, [])

  if (phase === "idle" || phase === "checking") return null

  return (
    <View style={styles.overlay} pointerEvents="auto">
      <View style={styles.card}>
        <ActivityIndicator color="#00c853" />
        <Text style={styles.text}>
          {phase === "downloading" ? "Downloading update…" : "Reloading…"}
        </Text>
      </View>
    </View>
  )
}

const styles = StyleSheet.create({
  overlay: {
    alignItems: "center",
    backgroundColor: "rgba(0,0,0,0.72)",
    bottom: 0,
    justifyContent: "center",
    left: 0,
    position: "absolute",
    right: 0,
    top: 0,
    zIndex: 1000,
  },
  card: {
    alignItems: "center",
    backgroundColor: "#151515",
    borderColor: "#525458",
    borderRadius: 10,
    borderWidth: 1,
    flexDirection: "row",
    gap: 12,
    paddingHorizontal: 20,
    paddingVertical: 16,
  },
  text: { color: "#d7d8e5", fontSize: 15 },
})
