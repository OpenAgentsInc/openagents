import { useEffect, useState, useRef } from 'react'
import reactLogo from './assets/react.svg'
import viteLogo from '/vite.svg'
import './App.css'
import init, { defaultConfig, connect } from '@breeztech/breez-sdk-liquid'
import * as bip39 from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english';

function App() {
  const [count, setCount] = useState(0)
  const [isInitialized, setIsInitialized] = useState(false)
  const [walletInfo, setWalletInfo] = useState({
    balanceSat: 0,
    pendingSendSat: 0,
    pendingReceiveSat: 0
  })
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
      console.log(sdk)

      // Fetch wallet info
      const info = await sdk.getInfo()
      setWalletInfo({
        balanceSat: info.walletInfo.balanceSat,
        pendingSendSat: info.walletInfo.pendingSendSat,
        pendingReceiveSat: info.walletInfo.pendingReceiveSat
      })

      setIsInitialized(true)
    } catch (error) {
      console.error('Failed to initialize Breez SDK:', error)
      setIsInitialized(false)
      initializationRef.current = false; // Reset ref on error to allow retry
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
      <div>
        <a href="https://vite.dev" target="_blank">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
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

      <div className="card">
        <button onClick={() => setCount((count) => count + 1)}>
          count is {count}
        </button>
      </div>
    </>
  )
}

export default App
