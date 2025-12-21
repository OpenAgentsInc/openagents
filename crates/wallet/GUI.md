# Wallet GUI

The wallet now includes a native desktop GUI built with wry/tao + Actix + Maud/HTMX.

## Launch

```bash
# Start the GUI
cargo wallet gui
```

## Requirements

You must first initialize a wallet:

```bash
cargo wallet init
```

This stores the mnemonic securely in your OS keychain.

## Features

### Dashboard
- View your Nostr identity (npub)
- Display name and profile info
- Total balance across all payment methods
  - Spark L2 balance
  - Lightning balance
  - On-chain balance

### Send Payment
- Send to Bitcoin address, Lightning invoice, or Spark address
- Specify amount in sats
- Form validation

### Receive Payment
- Display your Spark address for receiving
- Generate Lightning invoices with custom amounts
- Optional description field

### Transaction History
- View all past transactions
- Filter by type, date, status
- Shows amounts, timestamps, and status

### Settings
- Manage Nostr relay connections
- Add/remove relays
- View current network configuration

## Architecture

```
┌─────────────────────────────────────┐
│        wry/tao Window               │
│    (Native Desktop Window)          │
├─────────────────────────────────────┤
│                                     │
│      WebView (localhost:PORT)       │
│                                     │
│  ┌───────────────────────────────┐  │
│  │   Actix Web Server            │  │
│  │   (Rust Backend)              │  │
│  ├───────────────────────────────┤  │
│  │   Maud Templates + HTMX       │  │
│  │   (Server-Rendered HTML)      │  │
│  └───────────────────────────────┘  │
│                                     │
└─────────────────────────────────────┘
           │
           ├─ UnifiedIdentity (Nostr + Bitcoin)
           ├─ SecureKeychain (OS native)
           └─ NostrClient (relay communication)
```

## UI Conventions

Following OpenAgents desktop UI standards:

- **Inline-first CSS** with CSS custom properties
- **No border radius** (sharp corners everywhere)
- **Server-rendered** (no SPA, minimal JS)
- **HTMX** for dynamic updates
- **Dark theme** by default
- **Monospace fonts** for code/addresses

## Implementation

- `crates/wallet/src/gui/mod.rs` - Module exports
- `crates/wallet/src/gui/app.rs` - Window and lifecycle management
- `crates/wallet/src/gui/server.rs` - Actix web server and routes
- `crates/wallet/src/gui/views.rs` - Maud templates for all pages

## TODO

- [ ] Integrate actual Spark wallet operations
- [ ] Add WebSocket for real-time balance updates
- [ ] Fetch and display actual transaction history
- [ ] Implement profile fetching from Nostr relays
- [ ] Add QR code generation for receiving addresses
- [ ] Add invoice parsing and validation
- [ ] Implement relay configuration persistence
- [ ] Add NIP-05 verification display
- [ ] Contact list integration
- [ ] Zap functionality from GUI
- [ ] Multi-account support
