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
- `Relay Connections`
  - Configured relay list with per-relay state (`connected`, `connecting`, `disconnected`, `error`), latency, last-seen, and last-error fields.
  - Inputs/actions: add relay (`wss://` validation), select row, retry selected, remove selected.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Sync Health`
  - Spacetime sync diagnostics for connection state, subscription state, cursor progress, stale detection, replay count, and duplicate-drop count.
  - Action: `Rebootstrap sync` for deterministic recovery lifecycle reset.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Earnings Scoreboard`
  - Canonical MVP earnings metrics sourced from wallet/runtime/history lanes.
  - Shows sats today, lifetime sats, jobs today, last job result, and current online uptime.
  - Actions: refresh metrics and stale-state visibility.
- `Job Inbox`
  - Deterministic intake pane for incoming NIP-90 requests with stable request IDs and replay-safe ordering.
  - Shows requester, capability, price, ttl, validation state, and decision state per request.
  - Actions: select request, accept selected (with reason), reject selected (with reason).
- `Active Job`
  - In-flight job lifecycle pane for one selected job (`received -> accepted -> running -> delivered -> paid`).
  - Shows append-only execution log events, invoice/payment linkage, and failure reason when present.
  - Actions: advance stage, abort job (disabled when runtime lane does not support cancel).
- `Job History`
  - Deterministic receipt/history pane for completed/failed jobs with immutable metadata.
  - Includes status/time filters, job-id search, and pagination.
  - Row model includes `job_id`, `status`, `completed timestamp`, `result hash`, and `payment pointer`.
- `Nostr Keys (NIP-06)`
  - Shows identity path, `npub`, masked `nsec`, masked mnemonic, and key controls.
  - Secrets are masked by default, reveal is timed, and copy emits explicit custody warning copy.
  - Explicit pane state machine: `loading`, `ready`, `error`.
  - Regenerate immediately triggers dependent wallet refresh.
  - Actions: regenerate keys, reveal/hide secrets, copy `nsec`.
- `Spark Lightning Wallet`
  - Shows wallet connectivity, balances, addresses, invoice creation, payment sending, and recent payment status.
  - Explicit pane state machine: `loading` (awaiting first refresh), `ready`, `error`.
  - Actions: refresh wallet, generate receive addresses, copy Spark address, create invoice, send payment.
- `Create Lightning Invoice`
  - Dedicated pane for creating receive invoices separate from pay flow.
  - Inputs: invoice sats (required), description (optional), expiry seconds (optional).
  - Outputs: generated invoice text, copy action, and QR payload field.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Pay Lightning Invoice`
  - Dedicated payment pane for paying a Lightning invoice/payment request.
  - Inputs: payment request (required), send sats (optional).
  - Explicit pane state machine: `loading`, `ready`, `error`.
  - Action: pay invoice (`Enter` submit and button submit are equivalent).

## Opening Panes

- Hotbar:
  - `2` opens `Nostr Keys (NIP-06)`.
  - `3` opens `Spark Lightning Wallet`.
  - `K` opens the command palette.
- Command Palette (`K`):
  - `Autopilot Chat` -> opens `Autopilot Chat`.
  - `Go Online` -> opens `Go Online`.
  - `Provider Status` -> opens `Provider Status`.
  - `Earnings Scoreboard` -> opens `Earnings Scoreboard`.
  - `Relay Connections` -> opens `Relay Connections`.
  - `Sync Health` -> opens `Sync Health`.
  - `Job Inbox` -> opens `Job Inbox`.
  - `Active Job` -> opens `Active Job`.
  - `Job History` -> opens `Job History`.
  - `Identity Keys` -> opens `Nostr Keys (NIP-06)`.
  - `Spark Wallet` -> opens `Spark Lightning Wallet`.
  - `Create Lightning Invoice` -> opens `Create Lightning Invoice`.
  - `Pay Lightning Invoice` -> opens `Pay Lightning Invoice`.

## Behavior Notes

- Chat, Go Online, Provider Status, Relay Connections, Sync Health, Earnings Scoreboard, Job Inbox, Active Job, Job History, identity, wallet, create-invoice, and pay-invoice panes are singletons: opening again brings the existing pane to front.
- Wallet worker updates are shared across wallet-related panes.
- When a new invoice is created in the wallet pane, that invoice is prefilled into send/payment request inputs.
