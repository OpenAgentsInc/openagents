# OpenAgents Desktop parity ledger

- Updated: 2026-07-10
- Source: deprecated [`clients/khala-code-desktop`](../../clients/khala-code-desktop)
- Destination: `apps/openagents-desktop` on Electron + Effect Native
- Rule: extract behavior and typed data contracts. Never import the frozen
  desktop package or reproduce its legacy UI
- Lane: ready #8574 leaves only, under Sol's
  [`Terra execution-lane contract`](../sol/2026-07-10-terra-execution-lane.md)

## Meaning of parity

Parity is not “a button with the old label.” A capability is at parity only
when it has a bounded host service, typed renderer projection, user action,
failure/degraded state, and proportional verification. A capability that is
not connected stays absent or explicitly unavailable—it is never dressed up as
working product.

## Capability ledger

| Capability | Old Khala Code evidence | Destination | Status |
| --- | --- | --- | --- |
| Thread catalog, new/open chat, local persistence | `session-catalog`, Codex thread sidebar | Host-owned five-thread store + typed IPC | Landed local baseline. No Sync parity yet |
| Chat completion | Codex/Claude/Grok runtimes | Host-held OpenAgents gateway bridge | Landed gateway baseline. No Codex app-server parity yet |
| Composer focus and minimal conversation UX | `rich-composer`, transcript renderer | Effect Native composer/transcript | Landed baseline |
| Project/session home | `project-home-panel` | Effect Native workspace home + selected local root | Landed: real persisted conversation home. Folder selection starts the workspace flow |
| Local file explorer/editor | `editor-file-service`, `editor-panel` | Fixed typed Electron workspace service, read/edit/save state | In progress: user-chosen root, bounded root listing, and bounded read-only file preview are host-backed. Editing/saving remains next |
| Codex reconnect settings | provider/account readiness and local auth flows | Bounded readiness projection + isolated Pylon device-auth start | Landed baseline: headless smoke proves awaiting-browser state. Real owner browser completion remains an owner gate |
| Terminal workbench | `terminal-workbench`, `terminal-panel` | Bounded process/session projection and terminal host seam | Next. Must not expose arbitrary renderer command authority |
| Review/diff | `diff-review`, `review-panel` | Typed local git-status/diff projection, review view | Next |
| Inbox/approvals | `inbox`, Claude approvals, run evidence | Owner-safe approval/closeout projection | After durable Fleet/approval source is connected |
| Fleet supervision | fleet board/status/worker cards | Dedicated glass cockpit over authoritative Pylon/Sync state | Deferred from default chat. Source contract exists but current Desktop host only stages a brief |
| Gym/proof | `gym-pane`, proof loader | Read-only evidence projection | Depends on receipt source |
| Forum | `forum-panel` | Typed forum projection | Depends on current Forum API/auth seam |
| Settings/accounts/providers/skills/MCP | settings sections, provider catalog, permissions | Host-owned preferences and bounded status projections | Later. Credentials never enter renderer |
| Command palette/hotkeys | command registry/palette/thread hotkeys | Typed intent registry plus Electron menu/keyboard bridge | Later |
| Diagnostics/recovery/update | diagnostics, watchdog, updater | Dedicated support surface with public-safe bundle only | Later. Packaging identity/update policy remains owner-gated |

## Delivery order

1. Complete one owner Codex reconnect through Settings when the owner is
   available. Record account-ready evidence without exposing credentials. Do
   not idle other leaves on this proof gate.
2. Complete bounded editing/save and typed Git status/diff/review as one
   coherent local-workspace vertical slice.
3. Add terminal projection through a bounded host seam, never a generic
   renderer-to-shell bridge.
4. Add command palette/hotkeys to make the expanding capability set fast
   without reintroducing permanent header clutter.
5. Connect inbox, Fleet, Gym, Forum, and settings only to their current
   authoritative sources. Retain minimal chat as the everyday default.
6. Finish packaging/update/diagnostic parity under the separate owner gates.

## Non-negotiable extraction constraints

- No legacy `khala-code-desktop` UI imports.
- No raw filesystem, process, token, terminal, or IPC channel in the renderer.
- No default provider home or automatic provider-login behavior.
- No raw local paths, logs, credentials, or private repository content in
  public/evidence projections.
- No FleetRun, approval, provider, or update state manufactured from UI state.
- Every shared UI need becomes an Effect Native semantic component or renderer
  lowering before it becomes another app-local primitive.
