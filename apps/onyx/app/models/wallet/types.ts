import { IAnyModelType, Instance, IStateTreeNode } from "mobx-state-tree"

// Base store interface with just the properties
export interface IWalletStoreBase extends IStateTreeNode {
  isInitialized: boolean
  setupComplete: boolean
  error: string | null
  mnemonic: string | undefined
  setBalanceSat: (balanceSat: number) => void
  setPendingReceiveSat: (pendingReceiveSat: number) => void
  setPendingSendSat: (pendingSendSat: number) => void
  setMnemonic: (mnemonic: string | undefined) => void
  setError: (message: string | null) => void
  setInitialized: (isInitialized: boolean) => void
  setSetupComplete: (complete: boolean) => void
  setTransactions: (transactions: any[]) => void
  setNostrKeys: (nostrKeys: any) => void
}

// Balance related properties and actions
export interface IWalletStoreBalance extends IWalletStoreBase {
  balanceSat: number
  pendingSendSat: number
  pendingReceiveSat: number
  fetchBalanceInfo: () => Promise<void>
}

// Store with transactions
export interface IWalletStoreWithTransactions extends IWalletStoreBalance {
  transactions: {
    clear: () => void
    replace: (items: any[]) => void
    push: (item: any) => void
    toJSON: () => any[]
  }
}

// Full store interface
export interface IWalletStore extends IWalletStoreWithTransactions {
  setup: () => Promise<void>
  fetchTransactions: () => Promise<void>
  sendPayment: (bolt11: string, amount: number) => Promise<void>
  receivePayment: (amount: number, description?: string) => Promise<void>
  disconnect: () => Promise<void>
}

// Use a simple type alias to avoid circular references
export type WalletStore = Instance<IAnyModelType>
