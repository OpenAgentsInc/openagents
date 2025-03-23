import { BalanceInfo, BreezConfig, BreezService, Transaction } from "./types"

// Helper to generate a random id
const generateId = () => Math.random().toString(36).substring(2, 15)

// Mock implementation for web
class BreezServiceWebImpl implements BreezService {
  private mnemonic: string | undefined | null = undefined
  private isInitializedFlag = false
  private mockBalance: BalanceInfo = {
    balanceSat: 0,
    pendingSendSat: 0,
    pendingReceiveSat: 0
  }
  private mockTransactions: Transaction[] = []

  async initialize(config: BreezConfig): Promise<void> {
    console.log('Web mock: Initializing Breez service')
    this.mnemonic = config.mnemonic
    this.isInitializedFlag = true
  }

  async disconnect(): Promise<void> {
    console.log('Web mock: Disconnecting Breez service')
    this.isInitializedFlag = false
    this.mnemonic = null
  }

  async getBalance(): Promise<BalanceInfo> {
    this.ensureInitialized()
    console.log('Web mock: Getting balance')
    return this.mockBalance
  }

  async sendPayment(bolt11: string, amount: number): Promise<Transaction> {
    this.ensureInitialized()
    console.log('Web mock: Sending payment', { bolt11, amount })
    
    if (amount < 1000) {
      throw new Error("Minimum send amount is 1000 sats")
    }

    const tx: Transaction = {
      id: generateId(),
      amount: amount,
      timestamp: Date.now(),
      type: 'send',
      status: 'complete',
      paymentHash: generateId(),
      fee: Math.floor(amount * 0.01) // Mock 1% fee
    }

    this.mockTransactions.push(tx)
    this.mockBalance.balanceSat -= (amount + (tx.fee || 0))

    return tx
  }

  async receivePayment(amount: number, description?: string): Promise<string> {
    this.ensureInitialized()
    console.log('Web mock: Creating invoice', { amount, description })

    if (amount < 1000) {
      throw new Error("Minimum receive amount is 1000 sats")
    }

    const mockInvoice = `lnbc${amount}${generateId()}`
    
    // Simulate received payment immediately in mock
    const tx: Transaction = {
      id: generateId(),
      amount: amount,
      timestamp: Date.now(),
      type: 'receive',
      status: 'complete',
      description,
      paymentHash: generateId()
    }

    this.mockTransactions.push(tx)
    this.mockBalance.balanceSat += amount

    return mockInvoice
  }

  async getTransactions(): Promise<Transaction[]> {
    this.ensureInitialized()
    console.log('Web mock: Getting transactions')
    return this.mockTransactions
  }

  async getMnemonic(): Promise<string> {
    this.ensureInitialized()
    if (!this.mnemonic) {
      throw new Error('Mnemonic not available')
    }
    return this.mnemonic
  }

  isInitialized(): boolean {
    return this.isInitializedFlag
  }

  private ensureInitialized() {
    if (!this.isInitializedFlag) {
      throw new Error('Breez service not initialized')
    }
  }
}

// Export a singleton instance
export const breezService = new BreezServiceWebImpl()