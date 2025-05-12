import { useEffect, useState, useRef } from 'react'
import init, { defaultConfig, connect, ReceiveAmount, BindingLiquidSdk } from '@breeztech/breez-sdk-liquid'
import * as bip39 from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english';

function App() {
  const [isInitialized, setIsInitialized] = useState(false)
  const [walletInfo, setWalletInfo] = useState({
    balanceSat: 0,
    pendingSendSat: 0,
    pendingReceiveSat: 0
  })
  const [lightningLimits, setLightningLimits] = useState({
    min: 0,
    max: 0
  })
  const [receiveAmount, setReceiveAmount] = useState(100)
  const [invoice, setInvoice] = useState('')
  const [fees, setFees] = useState(0)
  const sdkRef = useRef<BindingLiquidSdk | null>(null)
  const initializationRef = useRef(false)

  const connectToBreez = async () => {
    if (isInitialized || initializationRef.current) return;
    initializationRef.current = true;

    try {
      // Generate x random words. Uses Cryptographically-Secure Random Number Generator.
      const mn = bip39.generateMnemonic(wordlist);
      console.log(mn);

      // Call init when using the SDK in a web environment before calling any other SDK
      // methods. This is not needed when using the SDK in a Node.js/Deno environment.
      await init()

      // Create the default config, providing your Breez API key
      const config = defaultConfig('mainnet', import.meta.env.VITE_BREEZ_API_KEY)

      const sdk = await connect({ mnemonic: mn, config })
      sdkRef.current = sdk
      console.log(sdk)

      // Fetch wallet info
      const info = await sdk.getInfo()
      setWalletInfo({
        balanceSat: info.walletInfo.balanceSat,
        pendingSendSat: info.walletInfo.pendingSendSat,
        pendingReceiveSat: info.walletInfo.pendingReceiveSat
      })

      // Fetch lightning limits
      const limits = await sdk.fetchLightningLimits()
      setLightningLimits({
        min: limits.receive.minSat,
        max: limits.receive.maxSat
      })

      setIsInitialized(true)
    } catch (error) {
      console.error('Failed to initialize Breez SDK:', error)
      setIsInitialized(false)
      initializationRef.current = false; // Reset ref on error to allow retry
    }
  }

  const generateInvoice = async () => {
    if (!sdkRef.current) return;

    try {
      // First prepare the payment to check fees
      const optionalAmount = {
        type: 'bitcoin',
        payerAmountSat: receiveAmount
      } as ReceiveAmount

      // Step 1: Prepare the payment
      const prepareResponse = await sdkRef.current.prepareReceivePayment({
        paymentMethod: 'lightning',
        amount: optionalAmount
      })

      // Store the fees
      setFees(prepareResponse.feesSat)

      // Step 2: Generate the actual invoice using the prepare response
      const receiveResponse = await sdkRef.current.receivePayment({
        prepareResponse
      })

      // Store the invoice - it's directly in the destination field
      if (typeof receiveResponse === 'object' && receiveResponse !== null && receiveResponse.destination) {
        setInvoice(receiveResponse.destination)
      }
    } catch (error) {
      console.error('Failed to generate invoice:', error)
    }
  }

  useEffect(() => {
    connectToBreez()
  }, []) // Empty dependency array ensures this runs only once

  // Helper function to format satoshis to BTC
  const formatSatToBTC = (sats: number) => {
    return (sats / 100000000).toFixed(8)
  }

  return (
    <>
      <h1>Bitcoin Liquid Wallet</h1>

      <div className="wallet-info">
        <h2>Wallet Balance</h2>
        <div className="balance-grid">
          <div className="balance-item">
            <h3>Available Balance</h3>
            <p>{formatSatToBTC(walletInfo.balanceSat)} BTC</p>
            <small>{walletInfo.balanceSat} sats</small>
          </div>
          <div className="balance-item">
            <h3>Pending Send</h3>
            <p>{formatSatToBTC(walletInfo.pendingSendSat)} BTC</p>
            <small>{walletInfo.pendingSendSat} sats</small>
          </div>
          <div className="balance-item">
            <h3>Pending Receive</h3>
            <p>{formatSatToBTC(walletInfo.pendingReceiveSat)} BTC</p>
            <small>{walletInfo.pendingReceiveSat} sats</small>
          </div>
        </div>
      </div>

      <div className="receive-section">
        <h2>Receive Payment</h2>
        <div className="receive-content">
          <div className="amount-input">
            <label>Amount (sats)</label>
            <input
              type="number"
              value={receiveAmount}
              onChange={(e) => setReceiveAmount(Number(e.target.value))}
              min={lightningLimits.min}
              max={lightningLimits.max}
            />
            <small>Min: {lightningLimits.min} sats, Max: {lightningLimits.max} sats</small>
          </div>

          <button
            className="generate-button"
            onClick={generateInvoice}
            disabled={!isInitialized || receiveAmount < lightningLimits.min || receiveAmount > lightningLimits.max}
          >
            Generate Invoice
          </button>

          {fees > 0 && (
            <div className="fees-info">
              <p>Network Fees: {fees} sats</p>
            </div>
          )}

          {invoice && (
            <div className="invoice-display">
              <h3>Lightning Invoice</h3>
              <textarea
                readOnly
                value={invoice}
                onClick={(e) => {
                  const textarea = e.target as HTMLTextAreaElement;
                  textarea.select();
                  document.execCommand('copy');
                }}
                placeholder="Generated invoice will appear here..."
              />
              <small>Click to copy</small>
            </div>
          )}
        </div>
      </div>
    </>
  )
}

export default App
