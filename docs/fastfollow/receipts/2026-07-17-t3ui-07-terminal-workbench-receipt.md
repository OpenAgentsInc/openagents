# T3UI-07 terminal workbench receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@0f00c016a9`
- Scope: generation-owned persistent terminal workbench and bounded attachment

## Implemented

- Terminal is now a persisted T3UI-05 workbench surface. The transcript remains
  mounted while terminal tabs activate, close, maximize, and resize.
- Existing main-owned PTY sessions render exact shell/cwd labels, running,
  exited, and recovered status, output-gap disclosure, replayed output, detected
  local previews, and an auto-following active output tail.
- New, select, input, submit, resize, interrupt, restart, refresh, close, and
  preview actions cross typed intents into the existing bridge. A bounded
  `ResizeObserver` projection keeps the active PTY geometry aligned with the
  panel. No process or shell capability was introduced into the renderer.
- “Add output” explicitly copies only the final 20,000 characters of the active
  session into one-turn composer state. The provider message marks the terminal
  output as untrusted data, and the chip can be removed before submission.

## Proof

- Terminal transition/registry tests cover session creation, output replay,
  bounded retention, input submission, interrupt/restart, close, preview, and
  exact active-session attachment.
- Mounted shell tests cover persistent terminal tabs, output, input binding,
  submit, interrupt, attachment, and preview intents. Shell tests cover the
  untrusted provider-context envelope.
- The visual lane now admits `terminal-workbench`; all 19 canonical frames were
  regenerated from the production shell and the terminal frame was inspected.
- Desktop TypeScript and all 210 test files pass: 2,034 tests passed and 39
  were skipped. The old 18-state manifest was the sole expected pre-admission
  failure; the final production and Electron gates run after this receipt.

## Boundaries

The renderer cannot spawn a process, choose an executable, read an absolute
working directory, or write directly to a PTY. Terminal context is explicit,
bounded, one-turn, removable, and labeled untrusted. Browser automation,
settings convergence, remote/mobile, installed signed evidence, and T3 parity
remain later packets.
