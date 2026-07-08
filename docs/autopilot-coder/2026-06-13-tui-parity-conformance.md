# TUI Parity Conformance Checklist

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-13
Scope: M3 CL-24, openagents #4930

This checklist maps Pylon TUI capabilities to the shared UI parity components
exported from `packages/autopilot-ui/src/index.ts`.

## Matrix

| Pylon TUI capability | Shared component coverage | Status | Notes |
| --- | --- | --- | --- |
| Session spawn/list/events/cancel | `SessionList`, `SessionRow`, `SessionDetail`, `SessionActions`, `EventTimeline` | Covered | Covers session inventory, per-session status/detail, event stream display, and spawn/cancel action affordances. |
| Approvals/decision queue | `DecisionCard`, `DecisionActions` | Covered | Covers pending/resolved decision display and approve/deny/answer action affordances. |
| Steer/interrupt/pause/resume | `SteerControls` | Covered | Covers the four runtime control affordances with read-only and terminal-state disabling. |
| Accounts/usage/quota | `AccountList` | Covered | Covers provider account rows, readiness state, and known/unknown quota display. |
| Dev-loop verify status | `VerifyStatus` | Covered | Covers verify command status and required artifact presence. |
| Node/provider online/offline | `NodeStatusBadge`, `ProviderStatusList` | Covered | Covers node heartbeat status and provider online/offline list status. |
| Artifacts/closeout/receipts | `ArtifactList`, `ReceiptList` | Partial | Covers artifact and receipt visibility. Closeout is represented by artifact/receipt evidence, but there is no dedicated closeout action component in the shared UI yet. |
| Assignments poll/accept/progress | `AssignmentList` | Covered | Covers assignment inventory, accept affordance for available work, and progress state. Polling remains a data-source concern outside the presentational component. |
| Wallet/balance read | `EarningsPanel` | Partial | Covers read-only sats balance and earnings history. Wallet send/withdrawal controls are not part of this parity surface. |

## Export Gate

`packages/autopilot-ui/test/parity-conformance.test.ts` imports the shared
barrel (`../src/index`) and asserts every component named above is exported as a
function. The gate is intentionally narrow: if the checklist claims a component
covers a capability, the package barrel must keep exporting it.

Shared tokens are also exported from the barrel for visual consistency, but they
are not counted as capability components in this conformance matrix.
