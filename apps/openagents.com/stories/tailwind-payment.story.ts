export const title = "Tailwind - Payment Components"
export const component = "OpenAgents v1 Payments"

export const BalanceDisplay = {
  name: "Balance Display",
  html: `
    <div class="space-y-4">
      <div class="oa-balance">
        <svg class="oa-balance-icon-lightning" fill="currentColor" viewBox="0 0 24 24">
          <path d="M13 2L3 14h9l-1 8 10-12h-9z"/>
        </svg>
        <span class="oa-balance-amount">21,000<span class="oa-balance-unit">sats</span></span>
      </div>
      
      <div class="oa-balance oa-balance-large">
        <svg class="oa-balance-icon" fill="currentColor" viewBox="0 0 24 24">
          <path d="M11.8 10.9c-2.27-.59-3-1.2-3-2.15 0-1.09 1.01-1.85 2.7-1.85 1.42 0 2.13.54 2.39 1.4.12.4.45.7.87.7h.3c.66 0 1.13-.65.9-1.27-.42-1.18-1.4-2.16-2.96-2.54V4.5c0-.83-.67-1.5-1.5-1.5S10 3.67 10 4.5v.66c-1.94.42-3.5 1.68-3.5 3.61 0 2.31 1.91 3.46 4.7 4.13 2.5.6 3 1.48 3 2.41 0 .69-.49 1.79-2.7 1.79-1.65 0-2.5-.59-2.83-1.43-.15-.39-.49-.67-.9-.67h-.28c-.67 0-1.14.68-.89 1.3.57 1.39 1.9 2.21 3.4 2.53v.67c0 .83.67 1.5 1.5 1.5s1.5-.67 1.5-1.5v-.65c1.95-.37 3.5-1.5 3.5-3.55 0-2.84-2.43-3.81-4.7-4.4z"/>
        </svg>
        <span class="oa-balance-amount">0.00021000</span>
      </div>
    </div>
  `,
  description: "Balance display variations"
}

export const BalanceCard = {
  name: "Balance Card",
  html: `
    <div class="oa-balance-card max-w-sm">
      <p class="oa-balance-card-title">Lightning Balance</p>
      <div class="oa-balance-card-amount">
        ⚡ 150,000 <span class="text-base font-normal">sats</span>
      </div>
      <p class="oa-balance-card-value">≈ $45.00 USD</p>
      <div class="oa-balance-status mt-4">
        <span class="oa-balance-status-dot confirmed"></span>
        <span class="text-sm text-gray-400">Confirmed</span>
      </div>
    </div>
  `,
  description: "Full balance card display"
}

export const WalletBalances = {
  name: "Wallet Balances",
  html: `
    <div class="oa-wallet-balances max-w-2xl">
      <div class="oa-balance-card">
        <p class="oa-balance-card-title">Available Balance</p>
        <div class="oa-balance-card-amount">
          ⚡ 250,000 <span class="text-base font-normal">sats</span>
        </div>
        <p class="oa-balance-card-value">≈ $75.00 USD</p>
        <span class="oa-balance-change positive">+5,000 today</span>
      </div>
      
      <div class="oa-balance-card">
        <p class="oa-balance-card-title">Pending Balance</p>
        <div class="oa-balance-card-amount">
          ⚡ 10,000 <span class="text-base font-normal">sats</span>
        </div>
        <p class="oa-balance-card-value">≈ $3.00 USD</p>
        <div class="oa-balance-status mt-4">
          <span class="oa-balance-status-dot pending"></span>
          <span class="text-sm text-gray-400">Pending confirmation</span>
        </div>
      </div>
    </div>
  `,
  description: "Multiple balance cards"
}

export const LightningInvoice = {
  name: "Lightning Invoice",
  html: `
    <div class="oa-invoice max-w-sm">
      <div class="oa-invoice-qr">
        <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='192' height='192'%3E%3Crect width='192' height='192' fill='black'/%3E%3Crect x='20' y='20' width='20' height='20' fill='white'/%3E%3Crect x='40' y='20' width='20' height='20' fill='white'/%3E%3Crect x='60' y='20' width='20' height='20' fill='white'/%3E%3Crect x='80' y='20' width='20' height='20' fill='white'/%3E%3Crect x='100' y='20' width='20' height='20' fill='white'/%3E%3Crect x='120' y='20' width='20' height='20' fill='white'/%3E%3Crect x='140' y='20' width='20' height='20' fill='white'/%3E%3C/svg%3E" alt="QR Code">
      </div>
      
      <div class="oa-invoice-amount">
        <span class="oa-invoice-amount-sats">⚡</span> 50,000 sats
      </div>
      
      <div class="oa-invoice-timer oa-invoice-timer-urgent">
        Expires in 2:45
      </div>
      
      <div class="oa-invoice-text">
        lnbc500u1p3q8fznpp5qyur6y8vg5ej54gqp6n74q7kgf53g9skpy...
      </div>
      
      <div class="oa-invoice-actions">
        <button class="oa-button-secondary oa-button-sm">
          <svg class="w-4 h-4 mr-1" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 5H6a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2v-1M8 5a2 2 0 002 2h2a2 2 0 002-2M8 5a2 2 0 012-2h2a2 2 0 012 2m0 0h2a2 2 0 012 2v3m2 4H10m0 0l3-3m-3 3l3 3"></path>
          </svg>
          Copy Invoice
        </button>
        <button class="oa-button-primary oa-button-sm">Open Wallet</button>
      </div>
    </div>
  `,
  description: "Lightning invoice with QR code"
}

export const InvoiceStates = {
  name: "Invoice States",
  html: `
    <div class="space-y-6">
      <div class="oa-invoice-compact">
        <div class="oa-invoice-qr">
          <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='96' height='96' fill='black'%3E%3Crect width='96' height='96'/%3E%3C/svg%3E" alt="QR">
        </div>
        <div class="oa-invoice-compact-details">
          <div class="oa-invoice-status pending">
            <svg class="oa-invoice-status-icon" fill="currentColor" viewBox="0 0 24 24">
              <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
            </svg>
            <span class="oa-invoice-status-text">Waiting for payment...</span>
          </div>
          <p class="text-2xl font-mono text-white mt-2">10,000 sats</p>
          <p class="text-sm text-gray-500">Invoice expires in 10:00</p>
        </div>
      </div>
      
      <div class="oa-invoice-compact">
        <div class="oa-invoice-qr">
          <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='96' height='96' fill='%2310B981'%3E%3Crect width='96' height='96'/%3E%3C/svg%3E" alt="Paid">
        </div>
        <div class="oa-invoice-compact-details">
          <div class="oa-invoice-status paid">
            <svg class="oa-invoice-status-icon" fill="currentColor" viewBox="0 0 24 24">
              <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
            </svg>
            <span class="oa-invoice-status-text">Payment received!</span>
          </div>
          <p class="text-2xl font-mono text-white mt-2">25,000 sats</p>
          <p class="text-sm text-gray-500">Paid 2 minutes ago</p>
        </div>
      </div>
    </div>
  `,
  description: "Invoice payment states"
}

export const TransactionList = {
  name: "Transaction List",
  html: `
    <div class="oa-transaction-list max-w-2xl">
      <div class="oa-transaction">
        <div class="oa-transaction-icon incoming">
          <svg fill="currentColor" viewBox="0 0 24 24">
            <path d="M2 12l5 5 1.41-1.41L5.83 13H22v-2H5.83l2.58-2.59L7 7z"/>
          </svg>
        </div>
        <div class="oa-transaction-details">
          <h4 class="oa-transaction-title">Payment Received</h4>
          <p class="oa-transaction-description">From: satoshi@bitcoin.org</p>
        </div>
        <div class="oa-transaction-amount incoming">
          <div class="oa-transaction-amount-value">+50,000 sats</div>
          <div class="oa-transaction-amount-usd">$15.00</div>
          <div class="oa-transaction-time">2 hours ago</div>
        </div>
      </div>
      
      <div class="oa-transaction">
        <div class="oa-transaction-icon outgoing">
          <svg fill="currentColor" viewBox="0 0 24 24">
            <path d="M22 12l-5-5-1.41 1.41L18.17 11H2v2h16.17l-2.58 2.59L17 17z"/>
          </svg>
        </div>
        <div class="oa-transaction-details">
          <h4 class="oa-transaction-title">Agent Payment</h4>
          <p class="oa-transaction-description">To: Code Assistant Agent</p>
        </div>
        <div class="oa-transaction-amount outgoing">
          <div class="oa-transaction-amount-value">-1,000 sats</div>
          <div class="oa-transaction-amount-usd">$0.30</div>
          <div class="oa-transaction-time">5 hours ago</div>
        </div>
      </div>
      
      <div class="oa-transaction">
        <div class="oa-transaction-icon incoming">
          <svg fill="currentColor" viewBox="0 0 24 24">
            <path d="M2 12l5 5 1.41-1.41L5.83 13H22v-2H5.83l2.58-2.59L7 7z"/>
          </svg>
        </div>
        <div class="oa-transaction-details">
          <h4 class="oa-transaction-title">Deposit</h4>
          <p class="oa-transaction-description">Lightning Network</p>
          <div class="oa-transaction-status confirmed">
            <span class="oa-transaction-status-dot"></span>
            <span>Confirmed</span>
          </div>
        </div>
        <div class="oa-transaction-amount incoming">
          <div class="oa-transaction-amount-value">+100,000 sats</div>
          <div class="oa-transaction-amount-usd">$30.00</div>
          <div class="oa-transaction-time">Yesterday</div>
        </div>
      </div>
    </div>
  `,
  description: "Payment transaction history"
}

export const EmptyTransactions = {
  name: "Empty Transactions",
  html: `
    <div class="oa-transaction-empty">
      <svg class="w-16 h-16 mx-auto mb-4 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
      </svg>
      <p class="text-gray-500 mb-4">No transactions yet</p>
      <button class="oa-button-primary">Make a Deposit</button>
    </div>
  `,
  description: "Empty transaction state"
}