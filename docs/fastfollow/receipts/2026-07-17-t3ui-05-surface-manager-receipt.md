# T3UI-05 surface manager and tab strip receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@6b8ca94e0b`
- Scope: transcript-preserving capability surface manager and tab lifecycle

## Implemented

- The mounted React workbench keeps the transcript and composer alive while a
  right-side workbench panel opens. Its first-open width is deliberately 440px
  so the transcript remains the primary surface.
- Files and read-only Review are the only admitted tabs. Activating them emits
  the existing typed `DesktopWorkspaceSelected` intent, so their host refresh,
  grant, and Git boundaries remain authoritative. Terminal and Browser are not
  fabricated ahead of T3UI-07 and T3UI-08.
- The T3 tab lifecycle is mounted: add, activate, close, close others, close to
  the right, close all, panel close, maximize/restore, pointer resize, keyboard
  resize, and double-click width reset.
- A total bounded v1 decoder persists tab order, active tab, width, and
  maximized state per exact coding-session ref in renderer presentation
  storage. Unknown tab kinds, corrupt documents, duplicate tabs, and unbounded
  widths normalize to safe defaults.
- At narrow widths the panel becomes an overlay; at standard widths header
  controls compact to icons while the transcript remains mounted underneath.

## Proof

- The pure surface-layout suite covers lifecycle, activation fallback,
  close-others/right/all, maximize, bounded resize, corrupt recovery, and
  refusal of unknown surfaces.
- Mounted component proof covers transcript retention, exact Files/Review/Chat
  intent dispatch, tab addition, maximization, per-session persistence, and
  panel closure. The focused layout/adapter set passes 30 tests.
- The Desktop visual lane now includes a dedicated `surface-tabs` capture; all
  17 states pass at zero pixel drift after deliberate baseline admission.
- Desktop TypeScript passed. The full serial suite passed 209 files and 2,024
  tests with 39 skipped; the production build and both Electron fixture
  journeys passed. Sol guards and the publishing gate are recorded by the
  landed commit's final verification.

## Boundaries

This packet adds presentation state only. It grants no filesystem, Git
mutation, shell/PTTY, browser, path, credential, remote-placement, or worktree
authority. Files and Review currently expose honest loading/unavailable/summary
states; their full tree, tabs, rich diff, annotation, edit, and conflict UX is
T3UI-06. Terminal, preview, settings convergence, remote/mobile, closure, and
installed evidence remain later packets. This is not T3 parity.
