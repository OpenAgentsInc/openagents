import { useNavigation } from "@react-navigation/native"
import { useEffect, useState } from "react"
import { FlatList, View, type TextStyle, type ViewStyle } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { EmptyState, Header, ListItem, Screen, Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../theme/motion"
import {
  fetchKhalaMobileCreditsTransactions,
  type KhalaMobileCreditsTransaction,
} from "../sync/khala-mobile-credits-api"
import { signedAmountLabel, transactionKindLabel } from "../sync/khala-mobile-credits-format-core"

type LoadState =
  | Readonly<{ status: "loading" }>
  | Readonly<{ status: "unavailable" }>
  | Readonly<{ status: "error" }>
  | Readonly<{ status: "ready"; nextCursor: string | null; transactions: ReadonlyArray<KhalaMobileCreditsTransaction> }>

// Matches thread-messages-screen.tsx / thread-list-screen.tsx's stagger —
// arcade-fidelity audit (2026-07-06) §4.
const HISTORY_STAGGER_CAP = 8
const historyEntranceDelay = (index: number): number =>
  MOTION_STAGGER_MS * Math.min(index, HISTORY_STAGGER_CAP)

const formatOccurredAt = (iso: string): string => {
  const parsed = new Date(iso)
  return Number.isNaN(parsed.getTime()) ? iso : parsed.toLocaleString()
}

const TransactionRow = ({ transaction }: { transaction: KhalaMobileCreditsTransaction }) => {
  const { themed } = useAppTheme()
  const meta = signedAmountLabel(transaction.kind, transaction.amountUsdCents)
  const title =
    transaction.description.trim().length > 0 ? transaction.description : transactionKindLabel(transaction.kind)
  return (
    <ListItem
      accessibilityLabel={`${transactionKindLabel(transaction.kind)} ${meta}`}
      TextProps={{ weight: "medium", size: "sm" }}
      RightComponent={<Text size="xs" style={themed($meta)} text={meta} />}
    >
      {title}
      {"\n"}
      <Text size="xs" style={themed($dim)} text={formatOccurredAt(transaction.occurredAt)} />
    </ListItem>
  )
}

/**
 * MM-D3 (#8480): transaction history for the mobile credits balance, rebuilt on
 * the ported Infinite Red Ignite component kit (`../ignite`) so it shows the
 * real Ignite look. Behavior is unchanged: it reads against the endpoint
 * contract in `khala-mobile-credits-api.ts`'s header comment, and a
 * 404/network failure still degrades to an honest "not yet available" screen
 * rather than an empty or fabricated list, as defense in depth.
 */
export const CreditsHistoryScreen = () => {
  const { baseUrl, token } = useKhalaAuth()
  const { themed } = useAppTheme()
  const navigation = useNavigation()
  const [state, setState] = useState<LoadState>({ status: "loading" })

  const loadPage = async (cursor: string | undefined, append: boolean) => {
    if (token === "") {
      setState({ status: "error" })
      return
    }
    const result = await fetchKhalaMobileCreditsTransactions(baseUrl, token, { cursor, limit: 50 })
    if (!result.ok) {
      setState(result.kind === "unavailable" ? { status: "unavailable" } : { status: "error" })
      return
    }
    setState(previous => {
      const priorTransactions = append && previous.status === "ready" ? previous.transactions : []
      return {
        nextCursor: result.value.nextCursor,
        status: "ready",
        transactions: [...priorTransactions, ...result.value.transactions],
      }
    })
  }

  useEffect(() => {
    void loadPage(undefined, false)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <Screen preset="fixed" contentContainerStyle={themed($fill)}>
      <Header
        title="Credit history"
        leftIcon="‹"
        onLeftPress={() => {
          if (navigation.canGoBack()) navigation.goBack()
        }}
      />
      {state.status === "loading" ? (
        <View style={themed($centered)}>
          <EmptyState loading heading="Loading history" />
        </View>
      ) : state.status === "unavailable" ? (
        <View style={themed($centered)}>
          <EmptyState
            heading="History not yet available"
            content="Transaction history isn't available yet — it's coming soon."
          />
        </View>
      ) : state.status === "error" ? (
        <View style={themed($centered)}>
          <EmptyState status="error" heading="History unavailable" content="Could not load your credit history right now." />
        </View>
      ) : state.transactions.length === 0 ? (
        <View style={themed($centered)}>
          <EmptyState heading="No transactions yet" />
        </View>
      ) : (
        <FlatList
          ItemSeparatorComponent={() => <View style={themed($separator)} />}
          ListFooterComponent={
            state.nextCursor === null ? (
              <View style={themed($footerSpacer)} />
            ) : (
              <ListItem
                accessibilityLabel="Load more transactions"
                text="Load more"
                onPress={() => void loadPage(state.nextCursor ?? undefined, true)}
              />
            )
          }
          data={state.transactions}
          keyExtractor={transaction => transaction.id}
          renderItem={({ index, item: transaction }) => (
            <Animated.View entering={FadeIn.delay(historyEntranceDelay(index)).duration(MOTION_MEDIUM)}>
              <TransactionRow transaction={transaction} />
            </Animated.View>
          )}
        />
      )}
    </Screen>
  )
}

const $fill: ThemedStyle<ViewStyle> = () => ({ flex: 1 })

const $centered: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flex: 1,
  justifyContent: "center",
  paddingHorizontal: spacing.md,
})

const $separator: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  height: 1,
  marginHorizontal: spacing.md,
  backgroundColor: colors.separator,
})

const $footerSpacer: ThemedStyle<ViewStyle> = ({ spacing }) => ({ height: spacing.xl })

const $dim: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })
const $meta: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim, paddingTop: 2 })
