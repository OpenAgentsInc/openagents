# T3UI-09 settings and primitives receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@7fe8b322ff`
- Scope: routed settings and primitive-family convergence

## Implemented

- Settings now routes in place across General, Codex CLI, Extensions, Source
  control, Keybindings, Diagnostics, and Account while preserving one shell and
  one OpenAgents settings frame.
- General preserves Codex-only maintenance/account truth, default-off local
  usage consent, update outcomes, and click-to-reveal sensitive values.
- Extensions projects exact MCP and local-plugin rows. Source control exposes an
  exact read-only repository summary. Keybindings exposes rows, reset, edit,
  default restoration, and conflicts. Diagnostics exposes health rows, refresh,
  and redacted export. Every enabled action uses an existing typed intent.
- Loading, unavailable, empty, loaded, redacted, conflict, and explicit
  authority-boundary states share the mounted OpenAgents design language.
  Existing buttons, inputs, badges, tables, dialogs, menus, popovers, selectors,
  tooltips, sheets, and notices remain the primitive authority; no second UI
  architecture was admitted.

## Proof

- Mounted tests cover all seven navigation destinations and exact typed source-
  control refresh dispatch while preserving existing Codex-only, consent,
  redaction, and Khala-frame contracts.
- The visual lane admits and was manually inspected at `settings-routed`; all
  21 canonical frames pass with zero pixel drift.
- Desktop TypeScript, 212 serial test files (2,050 passing, 39 skipped), the
  production build, compatibility Electron smoke, React Electron smoke, and the
  visual gate pass.

## Boundaries

Settings does not gain provider credentials, Git mutation, raw shell access, or
ambient filesystem authority. Account values remain blurred until explicit
reveal. Remote/mobile management, responsive and accessibility closure,
installed signed evidence, and the final pinned component census remain later
packets; this is not a T3 parity claim.
