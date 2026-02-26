# Autopilot Desktop Panes

This document defines the active pane surfaces in `apps/autopilot-desktop` and how they are opened.

## Pane Inventory

- `Nostr Keys (NIP-06)`
  - Shows identity path, `npub`, masked `nsec`, masked mnemonic, and key controls.
  - Actions: regenerate keys, reveal/hide secrets, copy `nsec`.
- `Spark Lightning Wallet`
  - Shows wallet connectivity, balances, addresses, invoice creation, payment sending, and recent payment status.
  - Actions: refresh wallet, generate receive addresses, create invoice, send payment.
- `Pay Lightning Invoice`
  - Dedicated payment pane for paying a Lightning invoice/payment request.
  - Inputs: payment request (required), send sats (optional).
  - Action: pay invoice.

## Opening Panes

- Hotbar:
  - `2` opens `Nostr Keys (NIP-06)`.
  - `3` opens `Spark Lightning Wallet`.
- Command Palette (`Cmd/Ctrl+K`):
  - `Open Identity Keys Pane` -> opens `Nostr Keys (NIP-06)`.
  - `Open Spark Wallet Pane` -> opens `Spark Lightning Wallet`.
  - `Open Pay Lightning Invoice Pane` -> opens `Pay Lightning Invoice`.

## Behavior Notes

- Identity, wallet, and pay-invoice panes are singletons: opening again brings the existing pane to front.
- Wallet worker updates are shared across wallet-related panes.
- When a new invoice is created in the wallet pane, that invoice is prefilled into send/payment request inputs.
