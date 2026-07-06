import { useEffect, useState } from "react"
import { View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { fetchKhalaMobileCreditsBalance } from "../sync/khala-mobile-credits-api"
import { formatUsdCents, isLowBalance } from "../sync/khala-mobile-credits-format-core"
import { KhalaText } from "./khala-text"

/**
 * MM-D3 (#8480): the "glanceable near the composer" half of the acceptance
 * criterion — a small, non-interactive balance readout. Renders NOTHING
 * while the balance endpoint is unavailable (rather than a permanent "not
 * available" banner on every thread screen); the `/api/mobile/credits/balance`
 * route (`khala-mobile-credits-api.ts`) is now live server-side (#8505 Part 1),
 * so this degrade path is defense in depth rather than the steady state. The
 * Settings screen's Credits section is the one place that always explains the
 * honest "coming soon" state for an unavailable read.
 */
export const CreditsBalanceChip = () => {
  const { baseUrl, token } = useKhalaAuth()
  const [balanceUsdCents, setBalanceUsdCents] = useState<number | null>(null)

  useEffect(() => {
    let cancelled = false
    void fetchKhalaMobileCreditsBalance(baseUrl, token).then(result => {
      if (cancelled) return
      setBalanceUsdCents(result.ok ? result.value : null)
    })
    return () => {
      cancelled = true
    }
  }, [baseUrl, token])

  if (balanceUsdCents === null) return null

  return (
    <View className="flex-row items-center gap-2">
      <KhalaText variant="faint">Balance: {formatUsdCents(balanceUsdCents)}</KhalaText>
      {isLowBalance(balanceUsdCents) ? <KhalaText variant="danger">Low</KhalaText> : null}
    </View>
  )
}
