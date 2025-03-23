import { WalletStore } from "./types"
import { Transaction } from "@/services/breez/types"

export const createViews = (self: WalletStore) => ({
  get totalBalance() {
    return self.balanceSat
  },
  get hasPendingTransactions() {
    return self.pendingSendSat > 0 || self.pendingReceiveSat > 0
  },
  get recentTransactions() {
    return self.transactions.slice().sort((a: Transaction, b: Transaction) => b.timestamp - a.timestamp)
  },
  get pendingTransactions() {
    return self.transactions.filter((tx: Transaction) => tx.status === "pending")
  },
})
