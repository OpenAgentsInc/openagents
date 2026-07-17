# T3UI-03 composer parity receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- Baseline: `OpenAgentsInc/openagents@780887ff5fe74dafb857caaf482c8a36f580b9a9`
- Scope: mounted composer context, discovery, responsive controls, inline
  decisions, attachments, and mode-state continuity

## Implemented

- Existing bounded review and editor-file authorities now render as removable
  composer chips. Review/file/image-only turns use the existing submission
  intent and remain subject to the Effect-owned admission check.
- One T3-shaped composer discovery surface searches only already-loaded typed
  projections. It exposes available canonical commands, loaded workspace files
  and folders, the active editor file attachment, and admitted local plugin
  skills. Selections dispatch the existing command, workspace, attachment, or
  input intent; there is no renderer filesystem crawl, shell capability, or
  keyword tool router.
- Skill selection inserts the modeled `/skill <plugin>/<skill> ` grammar used
  by the existing provider boundary. File/folder selection opens the typed
  workspace entry, while attaching content remains a separate bounded active-
  editor action.
- Wide windows retain direct provider/model/reasoning/access/Full Auto controls.
  At narrow widths the secondary controls move into one compact overflow
  surface, while context, attachment, provider, model, Queue/Steer, Stop, and
  submit remain reachable.
- Tool approvals, provider questions, and plan review now mount immediately
  above the composer instead of interrupting transcript reading with a modal.
  Plan review keeps the exact Accept / Request changes / Replan vocabulary;
  provider questions retain options, Other text, and explicit submission.
- Existing image acquisition, preview, removal, count/type rejection, drag
  target, provider capability gating, Queue/Steer, Stop, Full Auto durable
  state, background-running state, permission mode, model, reasoning, and
  provider selection remain connected to their prior typed intents.

## Proof

- Composer suite: **26 tests**, including image-only and context-only sends,
  removable context chips, loaded file/folder/skill/command discovery, active-
  file attachment, skill insertion, workspace entry selection, exact submit,
  IME and Shift+Enter handling, caret-safe hydration, Queue/Steer, durable Full
  Auto, provider capability projection, and every live decision family.
- Composer plus mounted shell-adapter focus: **51 passed**.
- Full Desktop sweep: **2,007 passed**, **39 skipped**, **206 passing files**.
- Desktop TypeScript: passed.
- Production renderer build: passed with **2,420 modules transformed**.
- Classic and mounted React Electron fixture smokes: passed, including image
  attachment, first-keystroke focus, inline decision reconciliation, Full Auto,
  navigation/reload recovery, workspace context attachment, and clean teardown.
- The 16-state Desktop visual lane was inspected, intentionally re-baselined
  for the inline composer/decision geometry, and then passed with zero drift.
- Sol guards passed; the publishing gate records final landed verification.

## Boundaries

This packet reuses current OpenAgents state and authority. It does not add
ambient filesystem access, a terminal bridge, provider credentials, plugin
installation, remote control, cross-machine operation, or release authority.
It does not claim arbitrary inline Lexical decorator-node persistence: rich
contexts remain in the existing typed attachment projections so Effect-owned
draft text stays authoritative. Preview annotations, terminal-context chips,
revert actions, project/worktree orchestration, the complete pinned component
census, installed signed-build evidence, and T3 parity remain later packets.
