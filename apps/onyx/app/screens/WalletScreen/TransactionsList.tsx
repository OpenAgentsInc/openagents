import { formatDistanceToNow } from "date-fns"
import { observer } from "mobx-react-lite"
import { FC } from "react"
import {
  ScrollView, TextStyle, TouchableOpacity, View, ViewStyle
} from "react-native"
import { Text } from "@/components"
import { useStores } from "@/models"
import { typography } from "@/theme"
import { colors } from "@/theme/colorsDark"
import { Transaction } from "@/services/breez/types"

export const TransactionsList: FC = observer(function TransactionsList() {
  const { walletStore } = useStores()
  const { isInitialized, recentTransactions } = walletStore

  if (!isInitialized) return null

  return (
    <View style={$transactionsContainer}>
      <Text text="Recent Transactions" style={$sectionHeader} />
      <ScrollView style={$transactionsList}>
        {recentTransactions.length === 0 ? (
          <Text text="No transactions yet" style={$emptyText} />
        ) : (
          recentTransactions.map((tx: Transaction) => (
            <TouchableOpacity key={tx.id} style={$transactionItem} activeOpacity={1}>
              <View style={$transactionLeft}>
                <Text
                  text={tx.type === "send" ? "Sent" : "Received"}
                  style={[$transactionType, tx.type === "send" ? $sendText : $receiveText]}
                />
                <Text
                  text={formatDistanceToNow(new Date(tx.timestamp * 1000), { addSuffix: true })}
                  style={$transactionDate}
                />
              </View>
              <View style={$transactionRight}>
                <Text
                  text={`${tx.type === "send" ? "-" : "+"}${tx.amount} ₿`}
                  style={[$transactionAmount, tx.type === "send" ? $sendText : $receiveText]}
                />
                {tx.status === "pending" && (
                  <Text text="Pending" style={$pendingText} />
                )}
              </View>
            </TouchableOpacity>
          ))
        )}
      </ScrollView>
    </View>
  )
})

const $transactionsContainer: ViewStyle = {
  flex: 1,
  width: "100%",
  paddingHorizontal: 16,
  marginTop: 20,
}

const $sectionHeader: TextStyle = {
  color: colors.palette.accent100,
  fontSize: 16,
  marginTop: 24,
  marginBottom: 12,
  fontFamily: typography.primary.medium
}

const $transactionsList: ViewStyle = {
  flex: 1,
}

const $transactionItem: ViewStyle = {
  flexDirection: "row",
  justifyContent: "space-between",
  alignItems: "center",
  paddingVertical: 12,
  borderBottomWidth: 1,
  borderBottomColor: "#333",
}

const $transactionLeft: ViewStyle = {
  flex: 1,
}

const $transactionRight: ViewStyle = {
  alignItems: "flex-end",
}

// And update these style constants:
const $transactionType: TextStyle = {
  fontSize: 16,
  marginBottom: 4,
  fontFamily: typography.primary.normal,
  color: "white", // Added this
}

const $transactionDate: TextStyle = {
  color: "#888",
  fontSize: 14,
  fontFamily: typography.primary.normal,
}

const $transactionAmount: TextStyle = {
  fontSize: 16,
  fontFamily: typography.primary.medium,
  color: "white", // Added this
}

const $sendText: TextStyle = {
  color: 'white',
}

const $receiveText: TextStyle = {
  color: "white", // Changed from green to white
}

const $pendingText: TextStyle = {
  color: "#888",
  fontSize: 12,
  marginTop: 4,
  fontFamily: typography.primary.normal,
}

const $emptyText: TextStyle = {
  color: "#888",
  fontSize: 16,
  textAlign: "center" as const,
  marginTop: 20,
  fontFamily: typography.primary.normal,
}
