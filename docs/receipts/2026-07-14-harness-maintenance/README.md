# MAINT-1 (#8785) — one-click harness maintenance pixel proof

Captured 2026-07-14 by the built-Electron smoke journey
(`OPENAGENTS_DESKTOP_SMOKE=1` + `OPENAGENTS_DESKTOP_SMOKE_SHOTS`), on a real
machine with Codex CLI (npm-global), Claude Code (native), and OpenCode
(native) installed.

- `04-settings-current-codex-session.png` — Settings open, current-Codex-session
  MVP surface intact.
- `04b-settings-harness-maintenance.png` — the "Coding harnesses" section
  resolved from LIVE detection through the typed Runtime Gateway query:
  per-harness installed version + install channel + latest advisory, with the
  one-click update affordance (`Update to <latest>` for the behind-latest
  harness, `Check & update` for current ones) driving the typed
  `maintenance.harness_update` command.

The smoke step `settings-harness-maintenance` fails the run if the section is
stuck loading or renders no rows/unavailable state — the capture is gated on
the real detection resolving, not on a fixture.

Contract: `openagents_desktop.settings.harness_maintenance_one_click.v1`
(`apps/openagents-desktop/src/contracts/ux-contracts.ts`), guarantee text in
`apps/openagents-desktop/GUARANTEES.md`.
