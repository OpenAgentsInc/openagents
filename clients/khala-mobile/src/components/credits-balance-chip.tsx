import { useEffect, useState } from "react"
import { View, type TextStyle, type ViewStyle } from "react-native"

import {
  CREDIT_BALANCE_ENTITY_TYPE,
  decodeCreditBalanceEntity,
  personalScope,
  type CreditBalanceEntity,
} from "@openagentsinc/khala-sync"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import { fetchKhalaMobileCreditsBalance } from "../sync/khala-mobile-credits-api"
import {
  formatUsdCents,
  isLowBalance,
  selectDisplayedBalanceUsdCents,
} from "../sync/khala-mobile-credits-format-core"
import { useKhalaMobileSyncPrimitives } from "../sync/khala-mobile-sync-runtime-context"
import { useKhalaSyncScopeEntities } from "../sync/use-khala-sync-scope-entities"

/**
 * MM-D3 (#8480) + #8505 Part 2: the "glanceable near the composer" half of
 * the acceptance criterion — a small, non-interactive balance readout, now on
 * the ported Infinite Red Ignite `Text` primitive (`../ignite`).
 *
 * Reads live from the synced `scope.user.<id>` `credit_balance` entity
 * (#8505 Part 2) through the SAME already-open Khala Sync subscription the
 * thread list uses (`useKhalaMobileSyncPrimitives` + `useKhalaSyncScopeEntities`)
 * — so the number updates the moment a charge/grant/clawback lands
 * server-side, no manual refresh. Layered with the Part-1 REST poll
 * (`/api/mobile/credits/balance`) as a fallback
 * (`selectDisplayedBalanceUsdCents`): before the synced entity has a value
 * (cold start, or a user whose projection hasn't been backfilled server-side
 * yet), the chip still shows the REST value; the synced value takes over the
 * moment it exists. Renders NOTHING while BOTH sources are unavailable —
 * never a fabricated balance. The Settings screen's Credits section is the
 * one place that always explains the honest "coming soon" state for an
 * unavailable read.
 */
export const CreditsBalanceChip = () => {
  const { baseUrl, ownerUserId, token } = useKhalaAuth()
  const { themed } = useAppTheme()
  const [restBalanceUsdCents, setRestBalanceUsdCents] = useState<number | null>(null)

  useEffect(() => {
    let cancelled = false
    void fetchKhalaMobileCreditsBalance(baseUrl, token).then(result => {
      if (cancelled) return
      setRestBalanceUsdCents(result.ok ? result.value : null)
    })
    return () => {
      cancelled = true
    }
  }, [baseUrl, token])

  const { overlay, session, store } = useKhalaMobileSyncPrimitives()
  const synced = useKhalaSyncScopeEntities<CreditBalanceEntity>({
    decode: decodeCreditBalanceEntity,
    entityType: CREDIT_BALANCE_ENTITY_TYPE,
    overlay,
    scope: ownerUserId === "" ? "" : String(personalScope(ownerUserId)),
    session,
    store
  })
  const syncedBalanceUsdCents = synced.items[0]?.balanceUsdCents ?? null

  const balanceUsdCents = selectDisplayedBalanceUsdCents({
    restBalanceUsdCents,
    syncedBalanceUsdCents
  })

  if (balanceUsdCents === null) return null

  return (
    <View style={themed($chipRow)}>
      <Text size="xxs" style={themed($faint)} text={`Balance: ${formatUsdCents(balanceUsdCents)}`} />
      {isLowBalance(balanceUsdCents) ? <Text size="xxs" style={themed($danger)} text="Low" /> : null}
    </View>
  )
}

const $chipRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.xs,
})

const $faint: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })
const $danger: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.error })
