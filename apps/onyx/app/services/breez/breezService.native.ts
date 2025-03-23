import * as FileSystem from "expo-file-system"
import { log } from "@/utils/log"
import {
  connect, defaultConfig, disconnect, getInfo, InputTypeVariant,
  LiquidNetwork, listPayments, lnurlPay, LnUrlPayResultVariant, parse,
  PaymentDetailsVariant, PaymentMethod, prepareLnurlPay,
  prepareReceivePayment, receivePayment, sendPayment, prepareSendPayment,
  SendDestinationVariant, GetInfoResponse, PrepareLnUrlPayRequest, PrepareReceiveRequest,
  ReceiveAmount, ReceiveAmountVariant, fetchLightningLimits, WalletInfo, PayAmountVariant
} from "@breeztech/react-native-breez-sdk-liquid"
import { BalanceInfo, BreezConfig, BreezService, Transaction } from "./types"

// Helper function to convert file:// URL to path
const fileUrlToPath = (fileUrl: string) => {
  return decodeURIComponent(fileUrl.replace('file://', ''))
}

// Helper to generate a random id if none is provided
const generateId = () => Math.random().toString(36).substring(2, 15)

class BreezServiceImpl implements BreezService {
  private sdk: any = null
  private mnemonic: string | undefined | null = undefined
  private isInitializedFlag = false
  private initializationPromise: Promise<void> | null = null

  async initialize(config: BreezConfig): Promise<void> {
    // If already initializing, wait for that to complete
    if (this.initializationPromise) {
      return this.initializationPromise
    }

    // If already initialized, return immediately
    if (this.isInitializedFlag && this.sdk) {
      return Promise.resolve()
    }

    // Create a new initialization promise
    this.initializationPromise = (async () => {
      try {
        // Use Expo's document directory which is guaranteed to be writable
        const workingDirUrl = `${FileSystem.documentDirectory}breez`
        const workingDir = fileUrlToPath(workingDirUrl)

        // Create working directory if it doesn't exist
        const dirInfo = await FileSystem.getInfoAsync(workingDirUrl)
        if (!dirInfo.exists) {
          await FileSystem.makeDirectoryAsync(workingDirUrl, { intermediates: true })
        }

        // Test directory write permissions
        try {
          const testFile = `${workingDirUrl}/test.txt`
          await FileSystem.writeAsStringAsync(testFile, 'test')
          await FileSystem.deleteAsync(testFile, { idempotent: true })
        } catch (err: any) {
          throw new Error(`Working directory is not writable: ${err.message}`)
        }

        // Use provided mnemonic
        if (!config.mnemonic) {
          throw new Error("Mnemonic is required for initialization")
        }
        this.mnemonic = config.mnemonic

        // Initialize SDK with proper working directory
        const sdkConfig = await defaultConfig(
          config.network === 'MAINNET' ? LiquidNetwork.MAINNET : LiquidNetwork.TESTNET,
          config.apiKey
        )

        sdkConfig.workingDir = workingDir

        // Connect to the SDK and store the instance
        this.sdk = await connect({
          mnemonic: this.mnemonic,
          config: sdkConfig
        })

        // Only set initialized after successful connect
        this.isInitializedFlag = true

        console.log('Breez SDK initialized successfully')
      } catch (err) {
        console.error('Breez initialization error:', err)
        this.isInitializedFlag = false
        this.sdk = null
        this.mnemonic = null
        throw err
      } finally {
        this.initializationPromise = null
      }
    })()

    return this.initializationPromise
  }

  private ensureInitialized() {
    if (!this.isInitializedFlag || !this.sdk) {
      throw new Error('Breez SDK not initialized')
    }
  }

  async disconnect(): Promise<void> {
    try {
      if (this.sdk) {
        await disconnect()
        this.sdk = null
        this.isInitializedFlag = false
        this.mnemonic = null
      }
    } catch (err) {
      console.error('Error disconnecting from Breez:', err)
      throw err
    }
  }

  async getBalance(): Promise<BalanceInfo> {
    this.ensureInitialized()

    try {
      const info = await getInfo()
      return {
        balanceSat: Number(info.walletInfo.balanceSat || 0),
        pendingSendSat: Number(info.walletInfo.pendingSendSat || 0),
        pendingReceiveSat: Number(info.walletInfo.pendingReceiveSat || 0),
      }
    } catch (err) {
      console.error('Error fetching balance:', err)
      throw err
    }
  }

  async sendPayment(input: string, amount: number): Promise<Transaction> {
    this.ensureInitialized()

    if (amount < 1000) {
      throw new Error("Minimum send amount is 1000 sats")
    }

    try {
      // Try to parse the input as a Lightning Address or LNURL
      const parsedInput = await parse(input)

      if (parsedInput.type === InputTypeVariant.LN_URL_PAY) {
        // Handle Lightning Address payment
        const amountMsat = amount * 1000 // Convert sats to msats
        const prepareRequest: PrepareLnUrlPayRequest = {
          data: parsedInput.data,
          amount: {
            type: PayAmountVariant.BITCOIN,
            receiverAmountSat: amount
          }
        }

        const prepareResponse = await prepareLnurlPay(prepareRequest)

        // Execute the LNURL payment
        const result = await lnurlPay({
          prepareResponse
        })

        log({
          name: "BreezServiceImpl.sendPayment",
          preview: "LNURL payment result",
          value: result
        })

        if (result.type === LnUrlPayResultVariant.ENDPOINT_SUCCESS) {
          const details = result.data.payment.details
          const paymentHash = details.type === PaymentDetailsVariant.LIGHTNING
            ? details.paymentHash
            : generateId()

          return {
            id: paymentHash || generateId(),
            amount: amount,
            timestamp: Date.now(),
            type: 'send',
            status: 'complete',
            paymentHash: paymentHash,
            fee: prepareResponse.feesSat,
          }
        } else if (result.type === LnUrlPayResultVariant.PAY_ERROR) {
          return {
            id: result.data.paymentHash || generateId(),
            amount: amount,
            timestamp: Date.now(),
            type: 'send',
            status: 'failed',
            paymentHash: result.data.paymentHash,
            fee: prepareResponse.feesSat,
          }
        } else {
          // Handle ENDPOINT_ERROR
          throw new Error(result.data.reason)
        }

      } else {
        // Handle regular BOLT11 invoice
        const prepareResponse = await prepareSendPayment({
          destination: input
        })

        const result = await sendPayment({
          prepareResponse
        })

        return {
          id: result.payment.txId || generateId(),
          amount: result.payment.amountSat,
          timestamp: result.payment.timestamp,
          type: 'send',
          status: result.payment.status === 'complete' ? 'complete' : 'pending',
          paymentHash: result.payment.details.type === PaymentDetailsVariant.LIGHTNING
            ? result.payment.details.paymentHash
            : undefined,
          fee: result.payment.feesSat,
        }
      }
    } catch (err) {
      console.error('Error sending payment:', err)
      throw err
    }
  }

  async receivePayment(amount: number, description?: string): Promise<string> {
    this.ensureInitialized()

    try {
      const receiveAmount: ReceiveAmount = {
        type: ReceiveAmountVariant.BITCOIN,
        payerAmountSat: amount
      }

      const prepareResponse = await prepareReceivePayment({
        paymentMethod: PaymentMethod.LIGHTNING,
        amount: receiveAmount
      })

      const result = await receivePayment({
        prepareResponse,
        description: description || "Payment request"
      })

      return result.destination
    } catch (err) {
      console.error('Error preparing receive payment:', err)
      throw err
    }
  }

  async getTransactions(): Promise<Transaction[]> {
    this.ensureInitialized()

    try {
      const txs = await listPayments({})
      return txs.map((tx: any) => ({
        id: tx.txId || generateId(),
        amount: tx.amountSat,
        timestamp: tx.timestamp,
        type: tx.paymentType === 'send' ? 'send' : 'receive',
        status: tx.status,
        description: tx.details?.description,
        paymentHash: tx.details?.paymentHash,
        fee: tx.feesSat,
      }))
    } catch (err) {
      console.error('Error fetching transactions:', err)
      throw err
    }
  }

  async getMnemonic(): Promise<string> {
    this.ensureInitialized()
    if (!this.mnemonic) {
      throw new Error('Mnemonic not available')
    }
    return this.mnemonic
  }

  isInitialized(): boolean {
    return this.isInitializedFlag && this.sdk !== null
  }
}

// Export a singleton instance
export const breezService = new BreezServiceImpl()
