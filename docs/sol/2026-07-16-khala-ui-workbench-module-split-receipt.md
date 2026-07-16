# Khala UI workbench module-split receipt

Issue: [#8860](https://github.com/OpenAgentsInc/openagents/issues/8860)  
Implementation: `f0f1b65451`

## Result

The public `@openagentsinc/ui/desktop-workbench` entry point is now a stable
barrel over per-component modules in `packages/ui/src/workbench/`. Desktop and
web consumers retain the complete pre-split export surface while later typed
workbench lanes can extend a focused module and one dispatch branch instead of
editing a 694-line component file.

The new `dispatchWorkbenchItem` boundary covers the typed workbench item union
and records the intended component lane for every variant. Existing DOM and
`data-*` hooks remain stable.

## Theme boundary

The restyle uses Autopilot only as a donor grammar inside Khala: dense mono
instrumentation, compact labels, hairline geometry, status hierarchy, and
compatible color relationships resolve through shared semantic variables.
`khalaTheme` remains the sole mounted palette, and Khala's deep-blue
background/surface hierarchy wins every conflict. The workbench modules add no
raw color literals, competing `autopilotTheme` mount, or non-token radius.

## Verification

- Public export-name parity against the pre-split barrel: exact.
- Raw hex, hardcoded non-token radius, and `autopilotTheme` scan across the
  workbench source: zero findings.
- `packages/ui` TypeScript check: passed.
- Desktop workbench/theme consumer suites: 157 passed, 11 skipped.
- Web `/components` and `/splash` consumer suites: 5 passed.
- Desktop TypeScript check: passed.

The package's unfiltered `vp test --run` command expands to the entire
monorepo. That unrelated sweep was stopped after reporting existing failures in
API-worker packaging, Pylon timing, assurance/evidence guards, and repository
policy scans; no failure referenced `packages/ui` or this change.
