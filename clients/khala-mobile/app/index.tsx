import { useEffect, useRef, useState } from "react"
import { AppState, Text, View } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import {
  checkKhalaCodeConnectivity,
  type KhalaCodeConnectivityStatus
} from "../src/status/khala-code-connectivity"
import { KhalaChatFeed } from "../src/sync/khala-chat-feed"

const POLL_INTERVAL_MS = 5_000

export default function HomeScreen() {
  const [status, setStatus] = useState<KhalaCodeConnectivityStatus | null>(null)
  const checkingRef = useRef(false)

  useEffect(() => {
    let cancelled = false
    let timer: ReturnType<typeof setInterval> | null = null

    const runCheck = async () => {
      if (checkingRef.current) return
      checkingRef.current = true
      const result = await checkKhalaCodeConnectivity()
      checkingRef.current = false
      if (!cancelled) setStatus(result)
    }

    void runCheck()
    timer = setInterval(() => void runCheck(), POLL_INTERVAL_MS)

    const subscription = AppState.addEventListener("change", state => {
      if (state === "active") void runCheck()
    })

    return () => {
      cancelled = true
      if (timer !== null) clearInterval(timer)
      subscription.remove()
    }
  }, [])

  const reachable = status?.reachable === true
  const checking = status === null
  const dotColor = checking ? "#7e8a98" : reachable ? "#22c55e" : "#ef4444"

  return (
    <SafeAreaView className="flex-1 items-center bg-bg">
      <View className="items-center pt-10">
        <View
          className="h-24 w-24 rounded-full"
          style={{ backgroundColor: dotColor }}
        />
        <Text className="mt-6 font-sans text-base text-textMuted">
          {checking
            ? "checking…"
            : reachable
              ? `connected${status?.hostname ? ` — ${status.hostname}` : ""}`
              : "no khala code instance found"}
        </Text>
        {status?.target !== null && status?.target !== undefined ? (
          <Text className="mt-1 font-mono text-xs text-textFaint">
            {status.target}
          </Text>
        ) : null}
      </View>
      <KhalaChatFeed />
    </SafeAreaView>
  )
}
