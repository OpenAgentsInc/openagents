# T3UI-11 responsive, accessibility, and performance receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@8342ad3942`
- Scope: responsive workbench, keyboard/touch access, contrast/motion, and bounded large-input behavior

## Implemented

- Desktop now admits a 480-pixel minimum width instead of preserving a hidden
  760-pixel renderer floor. Standard and minimum modes compact the header,
  stack dense settings/connection rows, wrap browser and terminal controls,
  and keep the primary transcript composer in view.
- Workbench, file, and terminal tablists use roving focus with ArrowLeft,
  ArrowRight, Home, and End alternatives while preserving the typed activation
  intents used by pointer input.
- Transcript metadata actions remain exposed for coarse pointers, forced-colors
  mode retains visible focus and status boundaries, and the existing OS plus
  explicit application reduced-motion selectors continue to disable
  non-essential transitions and animation.
- Existing bounded behavior remains authoritative: the transcript virtualizer
  limits mounted rows for a 500-item history, workspace projections cap large
  result sets, and rich diffs truncate oversized content instead of mounting an
  unbounded document.

## Proof

- Viewport-contract tests pin standard capture to 900 by 760 and minimum capture
  to 480 by 720; the production BrowserWindow boundary pins the same admitted
  minimum.
- Mounted interaction tests prove tab roles, roving `tabIndex`, and keyboard
  activation. CSS boundary tests prove removal of the legacy width floor plus
  coarse-pointer, forced-colors, and minimum-mode rules.
- The `responsive-standard` and `responsive-minimum` frames were manually
  inspected for clipping and composer reachability. All 24 committed visual
  frames pass the deterministic pixel gate.
- Desktop TypeScript, the full serial suite, production build, compatibility
  Electron smoke, React Electron smoke, and Sol document gates pass.

## Boundaries

This packet does not claim a signed installed release or complete T3 parity.
The exact pinned component census, mounted catalog disposition, packaged journey,
and signed-build evidence remain T3UI-12.
