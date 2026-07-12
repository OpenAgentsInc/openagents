# CUT-23 local plugin registry receipt

Date: 2026-07-12  
Issue: [#8703](https://github.com/OpenAgentsInc/openagents/issues/8703)

This tranche closes CUT-23 residual R2 without claiming the remaining
skills/slash or permission-mode work complete.

## Landed contract

- Electron main owns an owner-only local plugin registry. Absolute directories
  never cross preload; the renderer receives opaque refs and bounded name,
  provider, provenance, scope, readiness, enablement, restart, next-turn-use,
  and capability metadata.
- Add uses a native main-process directory chooser. Toggle and remove require
  the exact opaque ref already projected to the renderer. Duplicate, invalid,
  missing, disabled, unknown-ref, over-cap, and corrupt rows fail closed.
- A plugin is runnable only when its directory contains
  `.claude-plugin/plugin.json`. Enabled valid paths are read fresh per turn and
  passed as the pinned Claude Agent SDK's typed `plugins: [{type: "local",
  path}]` option. No provider-default plugin discovery is enabled.
- Effect Native Settings shows readiness and app/next-turn scope and exposes
  typed add, enable/disable, and remove controls.

Codex currently has no equivalent local-plugin SDK contract. The registry says
`provider: claude_agent` rather than pretending provider parity; provider
disagreement is explicit, not silently emulated.

## Verification

- Desktop typecheck passed.
- Desktop full suite: 904 passed, 5 pre-existing capability skips, 0 failed,
  4,709 assertions.
- Desktop build passed.
- Built Electron smoke passed every required stage and ended with lifecycle
  teardown `active=0`.

## Remaining CUT-23 scope

- R1: skills/slash invocation and lifecycle (I3).
- R3: local permission-mode UI and runtime enforcement (J3).
- One real plugin/skill workflow on each supported provider remains an exit
  receipt; unsupported provider capability must stay explicit.
