# Wallet GUI

The wallet includes a native WGPUI desktop GUI backed by a winit + wgpu event
loop. It runs in-process (no local web server).

## Launch

```bash
openagents wallet gui
```

## Requirements

Initialize a wallet first:

```bash
openagents wallet init
```

## Features

### Header
- Balance summary across Spark L2, Lightning, and on-chain sats

### Send
- Send to Lightning invoice, Spark address, or on-chain address
- Transaction limit enforcement and large-send confirmation
- Inline success/error notices

### Receive
- Generate a receive payload for a specific amount
- QR code rendering for quick scanning
- Copyable payload string

### History
- Infinite scroll transaction list
- Click-to-select detail panel
- Balance trend chart derived from history

## Architecture

```
┌─────────────────────────────────────┐
│       winit + wgpu Window           │
├─────────────────────────────────────┤
│             WGPUI Scene             │
│  (WalletView layout + components)   │
└─────────────────────────────────────┘
           │
           ├─ Wallet backend (async worker)
           ├─ SparkWallet (Breez SDK)
           └─ Wallet config + storage
```

## UI Conventions

- WGPUI-only rendering, no web stack
- Square721 Std Roman default; Vera Mono embedded for monospace via WGPUI TextSystem
- Sharp corners (no border radius)
- Inline-first layout styling

## Implementation

- `crates/wallet/src/gui/app.rs` - Window, renderer, event loop
- `crates/wallet/src/gui/backend.rs` - Async backend worker + channels
- `crates/wallet/src/gui/view.rs` - Layout, rendering, input handling
- `crates/wallet/src/gui/types.rs` - Commands/updates shared types

## Tests

- `crates/wallet/src/gui/view.rs` - GUI interaction + layout unit tests
