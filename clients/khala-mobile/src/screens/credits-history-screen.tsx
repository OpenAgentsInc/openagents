import { useEffect, useState } from "react"
import { FlatList, View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { AppHeader } from "../components/app-header"
import { KhalaEmptyState } from "../components/khala-empty-state"
import { KhalaListItem } from "../components/khala-list-item"
import { KhalaScreen } from "../components/khala-screen"
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

const formatOccurredAt = (iso: string): string => {
  const parsed = new Date(iso)
  return Number.isNaN(parsed.getTime()) ? iso : parsed.toLocaleString()
}

const TransactionRow = ({ transaction }: { transaction: KhalaMobileCreditsTransaction }) => (
  <KhalaListItem
    accessibilityLabel={`${transactionKindLabel(transaction.kind)} ${signedAmountLabel(transaction.kind, transaction.amountUsdCents)}`}
    detail={formatOccurredAt(transaction.occurredAt)}
    meta={signedAmountLabel(transaction.kind, transaction.amountUsdCents)}
    title={transaction.description.trim().length > 0 ? transaction.description : transactionKindLabel(transaction.kind)}
  />
)

/**
 * MM-D3 (#8480): transaction history for the mobile credits balance. Reads
 * against the endpoint contract proposed in `khala-mobile-credits-api.ts`'s
 * header comment, which does not exist server-side yet — a 404/network
 * failure degrades to an honest "not yet available" screen rather than an
 * empty or fabricated list.
 */
export const CreditsHistoryScreen = () => {
  const { baseUrl, token } = useKhalaAuth()
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
    <KhalaScreen preset="fixed">
      <AppHeader showBack title="Credit history" />
      {state.status === "loading" ? (
        <View className="flex-1 justify-center px-4">
          <KhalaEmptyState loading title="Loading history" tone="accent" />
        </View>
      ) : state.status === "unavailable" ? (
        <View className="flex-1 justify-center px-4">
          <KhalaEmptyState
            detail="Transaction history isn't available yet — it's coming soon."
            title="History not yet available"
          />
        </View>
      ) : state.status === "error" ? (
        <View className="flex-1 justify-center px-4">
          <KhalaEmptyState detail="Could not load your credit history right now." title="History unavailable" tone="danger" />
        </View>
      ) : state.transactions.length === 0 ? (
        <View className="flex-1 justify-center px-4">
          <KhalaEmptyState title="No transactions yet" />
        </View>
      ) : (
        <FlatList
          ItemSeparatorComponent={() => <View className="mx-4 h-px bg-borderMuted" />}
          ListFooterComponent={
            state.nextCursor === null ? (
              <View className="h-8" />
            ) : (
              <KhalaListItem
                accessibilityLabel="Load more transactions"
                onPress={() => void loadPage(state.nextCursor ?? undefined, true)}
                title="Load more"
              />
            )
          }
          data={state.transactions}
          keyExtractor={transaction => transaction.id}
          renderItem={({ item: transaction }) => <TransactionRow transaction={transaction} />}
        />
      )}
    </KhalaScreen>
  )
}
