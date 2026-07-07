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
 * Live credit-balance readout pinned to the BOTTOM of the drawer flyout menu
 * (owner request, 2026-07-07: move the balance out of the chat header into the
 * drawer). Reads from the SAME live source as the old header chip
 * (`credits-balance-chip.tsx`): the synced `credit_balance` entity on the
 * personal scope (updates the moment a charge/grant/clawback lands), layered
 * over the Part-1 REST poll as a cold-start fallback
 * (`selectDisplayedBalanceUsdCents`). Renders a neutral "Balance —" line while
 * both sources are unavailable rather than a fabricated figure.
 */
export const DrawerCreditsBalance = () => {
  const { baseUrl, ownerUserId, token } = useKhalaAuth()
  const { theme, themed } = useAppTheme()
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
    store,
  })
  const syncedBalanceUsdCents = synced.items[0]?.balanceUsdCents ?? null

  const balanceUsdCents = selectDisplayedBalanceUsdCents({
    restBalanceUsdCents,
    syncedBalanceUsdCents,
  })

  const low = balanceUsdCents !== null && isLowBalance(balanceUsdCents)

  return (
    <View style={themed($container)}>
      <View style={themed($row)}>
        <Text
          size="xxs"
          weight="medium"
          style={{ color: theme.colors.textDim, textTransform: "uppercase", letterSpacing: 0.5 }}
          text="Balance"
        />
        <Text
          size="sm"
          weight="medium"
          style={{ color: low ? theme.colors.error : theme.colors.text }}
          text={balanceUsdCents === null ? "—" : formatUsdCents(balanceUsdCents)}
        />
      </View>
      {low ? <Text size="xxs" style={themed($low)} text="Low balance" /> : null}
    </View>
  )
}

const $container: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  borderTopWidth: 1,
  borderTopColor: colors.palette.neutral400,
  paddingHorizontal: spacing.md,
  paddingTop: spacing.sm,
  paddingBottom: spacing.xs,
  gap: spacing.xxs,
})

const $row: ThemedStyle<ViewStyle> = () => ({
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
})

const $low: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.error })
