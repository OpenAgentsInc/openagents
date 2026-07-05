import { useEffect, useRef, useState } from "react"
import { AppState } from "react-native"

import {
  checkKhalaCodeConnectivity,
  type KhalaCodeConnectivityStatus
} from "./khala-code-connectivity"

const POLL_INTERVAL_MS = 5_000

export const useKhalaCodeConnectivity = (): KhalaCodeConnectivityStatus | null => {
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

  return status
}
