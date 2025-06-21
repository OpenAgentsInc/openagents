/**
 * Test page for Spark SDK Lightning wallet integration
 * Demonstrates wallet creation, balance checking, and Lightning payments
 */

import { html } from "@openagentsinc/psionic"

export default function testSpark() {
  return html`
    <div class="test-spark">
      <h1>Spark Lightning Wallet Test</h1>
      
      <div class="controls">
        <button id="create-wallet">Create New Wallet</button>
        <button id="restore-wallet">Restore Wallet</button>
        <button id="check-balance">Check Balance</button>
        <button id="create-invoice">Create Invoice</button>
        <button id="pay-invoice">Pay Invoice</button>
      </div>
      
      <div class="wallet-info" id="wallet-info">
        <h3>Wallet Info</h3>
        <div id="wallet-details">No wallet loaded</div>
      </div>
      
      <div class="invoice-section" id="invoice-section">
        <h3>Lightning Invoice</h3>
        <div id="invoice-display"></div>
        <input type="text" id="invoice-input" placeholder="Paste Lightning invoice to pay" />
      </div>
      
      <div class="activity-log" id="activity-log">
        <h3>Activity Log</h3>
        <div id="log-entries"></div>
      </div>
    </div>
    
    <script type="module">
      import { Effect, Runtime } from "https://esm.sh/effect@3.10.3"
      import * as SDK from "@openagentsinc/sdk/browser"
      
      let currentWallet = null
      let currentMnemonic = null
      
      const logActivity = (message, type = 'info') => {
        const logEntries = document.getElementById('log-entries')
        const entry = document.createElement('div')
        entry.className = \`log-entry log-\${type}\`
        entry.textContent = \`[\${new Date().toLocaleTimeString()}] \${message}\`
        logEntries.appendChild(entry)
        logEntries.scrollTop = logEntries.scrollHeight
      }
      
      const updateWalletDisplay = async () => {
        if (!currentWallet) {
          document.getElementById('wallet-details').innerHTML = 'No wallet loaded'
          return
        }
        
        try {
          const program = Effect.gen(function*() {
            const sparkService = yield* SDK.SparkService
            const info = yield* sparkService.getWalletInfo(currentWallet)
            
            document.getElementById('wallet-details').innerHTML = \`
              <div><strong>Address:</strong> \${info.address}</div>
              <div><strong>Public Key:</strong> \${info.publicKey.slice(0, 32)}...</div>
              <div><strong>Balance:</strong> \${info.balanceSats} sats</div>
              <div><strong>Network:</strong> \${info.network}</div>
            \`
          })
          
          const runtime = Runtime.defaultRuntime
          const layer = SDK.createBrowserServicesLayer()
          
          await Runtime.runPromise(runtime)(
            program.pipe(Effect.provide(layer))
          )
        } catch (error) {
          logActivity(\`Error updating wallet: \${error.message}\`, 'error')
        }
      }
      
      // Create new wallet
      document.getElementById('create-wallet').addEventListener('click', async () => {
        try {
          logActivity('Creating new Spark wallet...')
          
          const program = Effect.gen(function*() {
            const sparkService = yield* SDK.SparkService
            const { wallet, mnemonic } = yield* sparkService.createWallet()
            
            currentWallet = wallet
            currentMnemonic = mnemonic
            
            logActivity('Wallet created successfully!', 'success')
            logActivity(\`Mnemonic (SAVE THIS): \${mnemonic}\`, 'warning')
            
            yield* Effect.sync(() => updateWalletDisplay())
          })
          
          const runtime = Runtime.defaultRuntime
          const layer = SDK.createBrowserServicesLayer()
          
          await Runtime.runPromise(runtime)(
            program.pipe(Effect.provide(layer))
          )
        } catch (error) {
          logActivity(\`Error creating wallet: \${error.message}\`, 'error')
        }
      })
      
      // Restore wallet
      document.getElementById('restore-wallet').addEventListener('click', async () => {
        const mnemonic = prompt('Enter your 12/24 word mnemonic:')
        if (!mnemonic) return
        
        try {
          logActivity('Restoring wallet from mnemonic...')
          
          const program = Effect.gen(function*() {
            const sparkService = yield* SDK.SparkService
            const { wallet } = yield* sparkService.createWallet(mnemonic)
            
            currentWallet = wallet
            currentMnemonic = mnemonic
            
            logActivity('Wallet restored successfully!', 'success')
            
            yield* Effect.sync(() => updateWalletDisplay())
          })
          
          const runtime = Runtime.defaultRuntime
          const layer = SDK.createBrowserServicesLayer()
          
          await Runtime.runPromise(runtime)(
            program.pipe(Effect.provide(layer))
          )
        } catch (error) {
          logActivity(\`Error restoring wallet: \${error.message}\`, 'error')
        }
      })
      
      // Check balance
      document.getElementById('check-balance').addEventListener('click', async () => {
        if (!currentWallet) {
          logActivity('No wallet loaded', 'error')
          return
        }
        
        logActivity('Checking balance...')
        await updateWalletDisplay()
        logActivity('Balance updated', 'success')
      })
      
      // Create invoice
      document.getElementById('create-invoice').addEventListener('click', async () => {
        if (!currentWallet) {
          logActivity('No wallet loaded', 'error')
          return
        }
        
        const amountStr = prompt('Enter amount in sats:')
        if (!amountStr) return
        
        const amount = parseInt(amountStr)
        if (isNaN(amount) || amount <= 0) {
          logActivity('Invalid amount', 'error')
          return
        }
        
        try {
          logActivity(\`Creating invoice for \${amount} sats...\`)
          
          const program = Effect.gen(function*() {
            const sparkService = yield* SDK.SparkService
            const invoice = yield* sparkService.createInvoice(currentWallet, {
              amountSats: amount,
              memo: 'Test invoice from Spark SDK'
            })
            
            document.getElementById('invoice-display').innerHTML = \`
              <div><strong>Invoice ID:</strong> \${invoice.id}</div>
              <div class="invoice-text">\${invoice.invoice}</div>
              <div><button onclick="navigator.clipboard.writeText('\${invoice.invoice}'); alert('Copied!')">Copy Invoice</button></div>
            \`
            
            logActivity(\`Invoice created: \${invoice.id}\`, 'success')
            
            // Monitor invoice status
            yield* Effect.fork(
              Effect.gen(function*() {
                let attempts = 0
                while (attempts < 60) { // Monitor for 5 minutes
                  const status = yield* sparkService.getInvoiceStatus(currentWallet, invoice.id)
                  
                  if (status === 'paid') {
                    logActivity(\`Invoice \${invoice.id} PAID! 🎉\`, 'success')
                    yield* Effect.sync(() => updateWalletDisplay())
                    break
                  } else if (status === 'expired') {
                    logActivity(\`Invoice \${invoice.id} expired\`, 'warning')
                    break
                  }
                  
                  yield* Effect.sleep(5000) // Check every 5 seconds
                  attempts++
                }
              })
            )
          })
          
          const runtime = Runtime.defaultRuntime
          const layer = SDK.createBrowserServicesLayer()
          
          await Runtime.runPromise(runtime)(
            program.pipe(Effect.provide(layer))
          )
        } catch (error) {
          logActivity(\`Error creating invoice: \${error.message}\`, 'error')
        }
      })
      
      // Pay invoice
      document.getElementById('pay-invoice').addEventListener('click', async () => {
        if (!currentWallet) {
          logActivity('No wallet loaded', 'error')
          return
        }
        
        const invoice = document.getElementById('invoice-input').value.trim()
        if (!invoice || !invoice.startsWith('ln')) {
          logActivity('Invalid Lightning invoice', 'error')
          return
        }
        
        try {
          logActivity('Paying Lightning invoice...')
          
          const program = Effect.gen(function*() {
            const sparkService = yield* SDK.SparkService
            const payment = yield* sparkService.payInvoice(currentWallet, {
              invoice,
              maxFeeSats: 10
            })
            
            if (payment.status === 'success') {
              logActivity(\`Payment successful! Fee: \${payment.feeSats} sats\`, 'success')
              logActivity(\`Preimage: \${payment.preimage.slice(0, 32)}...\`, 'info')
              yield* Effect.sync(() => updateWalletDisplay())
            } else {
              logActivity('Payment failed', 'error')
            }
          })
          
          const runtime = Runtime.defaultRuntime
          const layer = SDK.createBrowserServicesLayer()
          
          await Runtime.runPromise(runtime)(
            program.pipe(Effect.provide(layer))
          )
        } catch (error) {
          logActivity(\`Error paying invoice: \${error.message}\`, 'error')
        }
      })
      
      logActivity('Spark Lightning wallet test interface ready')
    </script>
    
    <style>
      .test-spark {
        max-width: 800px;
        margin: 0 auto;
        padding: 2rem;
      }
      
      .controls {
        display: flex;
        gap: 1rem;
        margin-bottom: 2rem;
        flex-wrap: wrap;
      }
      
      .controls button {
        padding: 0.75rem 1.5rem;
        background: var(--foreground0);
        color: var(--background0);
        border: none;
        cursor: pointer;
        font-family: inherit;
      }
      
      .controls button:hover {
        opacity: 0.8;
      }
      
      .wallet-info, .invoice-section {
        background: var(--background1);
        padding: 1rem;
        margin-bottom: 2rem;
        border: 1px solid var(--background3);
      }
      
      .invoice-text {
        font-family: monospace;
        font-size: 0.75rem;
        word-break: break-all;
        background: var(--background0);
        padding: 0.5rem;
        margin: 0.5rem 0;
      }
      
      #invoice-input {
        width: 100%;
        padding: 0.5rem;
        background: var(--background0);
        color: var(--foreground0);
        border: 1px solid var(--background3);
        font-family: monospace;
        margin-top: 1rem;
      }
      
      .activity-log {
        background: var(--background0);
        border: 1px solid var(--background3);
        padding: 1rem;
        height: 300px;
        overflow-y: auto;
      }
      
      .log-entry {
        font-family: monospace;
        font-size: 0.875rem;
        padding: 0.25rem 0;
      }
      
      .log-info { color: var(--foreground1); }
      .log-success { color: var(--foreground0); }
      .log-warning { color: #ff9800; }
      .log-error { color: #f44336; }
      
      #log-entries {
        max-height: 250px;
        overflow-y: auto;
      }
    </style>
  `
}
