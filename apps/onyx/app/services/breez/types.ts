export interface BalanceInfo {
  balanceSat: number
  pendingSendSat: number
  pendingReceiveSat: number
}

export interface BreezConfig {
  workingDir: string
  apiKey: string
  network: 'MAINNET' | 'TESTNET'
  mnemonic?: string // Add mnemonic as optional parameter
}

export interface Transaction {
  id: string
  amount: number
  timestamp: number
  type: 'send' | 'receive'
  status: 'pending' | 'complete' | 'failed'
  description?: string
  paymentHash?: string
  fee?: number
}

export interface BreezService {
  initialize(config: BreezConfig): Promise<void>
  disconnect(): Promise<void>
  getBalance(): Promise<BalanceInfo>
  sendPayment(bolt11: string, amount: number): Promise<Transaction>
  receivePayment(amount: number, description?: string): Promise<string> // Returns bolt11 invoice
  getTransactions(): Promise<Transaction[]>
  getMnemonic(): Promise<string>
  isInitialized(): boolean
}
