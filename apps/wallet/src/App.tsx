import { useEffect, useState } from 'react'
import reactLogo from './assets/react.svg'
import viteLogo from '/vite.svg'
import './App.css'
import init, { defaultConfig, connect } from '@breeztech/breez-sdk-liquid'

function App() {
  const [count, setCount] = useState(0)

  const connectToBreez = async () => {
    const mnemonic = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon'

    // Call init when using the SDK in a web environment before calling any other SDK
    // methods. This is not needed when using the SDK in a Node.js/Deno environment.
    //
    // import init, { defaultConfig, connect } from '@breeztech/breez-sdk-liquid'
    await init()

    // Create the default config, providing your Breez API key
    const config = defaultConfig('mainnet', '<your-Breez-API-key>')

    // The `workingDir` does not need to be set in a web environment
    config.workingDir = 'path to writable directory'

    const sdk = await connect({ mnemonic, config })

    console.log(sdk)
  }

  useEffect(() => {
    connectToBreez()
  }, [])

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
      <h1>Vite + React</h1>
      <div className="card">
        <button onClick={() => setCount((count) => count + 1)}>
          count is {count}
        </button>
        <p>
          Edit <code>src/App.tsx</code> and save to test HMR
        </p>
      </div>
      <p className="read-the-docs">
        Click on the Vite and React logos to learn more
      </p>
    </>
  )
}

export default App
