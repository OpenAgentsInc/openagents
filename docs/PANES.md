# Autopilot Desktop Panes

This document defines the active pane surfaces in `apps/autopilot-desktop` and how they are opened.

## Pane Inventory

- `Autopilot Chat`
  - Chat-first pane with thread rail, transcript, composer input, and per-message status (`queued`, `running`, `done`, `error`).
  - Action: send prompt to local Autopilot lane.
- `Go Online`
  - Provider mode toggle pane with explicit state machine (`offline`, `connecting`, `online`, `degraded`) and preflight blockers.
  - Action: toggle online/offline.
- `Provider Status`
  - Runtime status pane for heartbeat freshness, uptime, queue depth, and dependency state.
  - Action: read-only operational visibility.
- `Nostr Keys (NIP-06)`
  - Shows identity path, `npub`, masked `nsec`, masked mnemonic, and key controls.
  - Actions: regenerate keys, reveal/hide secrets, copy `nsec`.
- `Spark Lightning Wallet`
  - Shows wallet connectivity, balances, addresses, invoice creation, payment sending, and recent payment status.
  - Actions: refresh wallet, generate receive addresses, copy Spark address, create invoice, send payment.
- `Pay Lightning Invoice`
  - Dedicated payment pane for paying a Lightning invoice/payment request.
  - Inputs: payment request (required), send sats (optional).
  - Action: pay invoice.

## Opening Panes

- Hotbar:
  - `2` opens `Nostr Keys (NIP-06)`.
  - `3` opens `Spark Lightning Wallet`.
  - `K` opens the command palette.
- Command Palette (`K`):
  - `Autopilot Chat` -> opens `Autopilot Chat`.
  - `Go Online` -> opens `Go Online`.
  - `Provider Status` -> opens `Provider Status`.
  - `Identity Keys` -> opens `Nostr Keys (NIP-06)`.
  - `Spark Wallet` -> opens `Spark Lightning Wallet`.
  - `Pay Lightning Invoice` -> opens `Pay Lightning Invoice`.

## Behavior Notes

- Chat, Go Online, Provider Status, identity, wallet, and pay-invoice panes are singletons: opening again brings the existing pane to front.
- Wallet worker updates are shared across wallet-related panes.
- When a new invoice is created in the wallet pane, that invoice is prefilled into send/payment request inputs.
