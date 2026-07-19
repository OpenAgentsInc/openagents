# Khala editor theme owner work packet

- Date: 2026-07-19
- Status: delivered to `main`
- Authority: current owner direction in the initiating Codex task
- Scope: OpenAgents Desktop IDE/workbench theme projection only

## Owner outcome

Make a new repository-owned Khala editor theme the Desktop IDE default. Keep
Tokyo Night as a built-in fallback. Preserve Tokyo Night's syntax separation
and contrast discipline while moving the editor, workbench, Pierre review,
terminal, status surfaces, native window background, and first HTML paint onto
the Khala blue-black product identity.

This packet does not add a user-facing theme marketplace, remote theme import,
unsafe CSS, executable theme contributions, or renderer authority. Theme data
remains schema-decoded and projection-only.

## CLAIM

```text
actor/session: codex-desktop-task/khala-editor-theme-owner-directive
base: 9bbe09766e1cccf32a38e39c2cce7308873eb817
worktree/branch: detached-clean-worktree/openagents-khala-editor-theme
scope: Add the Khala editor projection, retain Tokyo Night fallback, switch every default IDE/workbench consumer before first paint, and update theme contracts/tests/evidence.
paths: apps/openagents-desktop/src/ide/** theme files and adapters; apps/openagents-desktop/src/desktop-preferences-effects.ts; apps/openagents-desktop/src/renderer/theme.ts and theme bridges; apps/openagents-desktop/src/main.ts; apps/openagents-desktop/index.html; focused Desktop tests/contracts; packages/behavior-contracts/src/openagents-apps.ts; docs/ide/ROADMAP.md; this work packet.
hot files: apps/openagents-desktop/src/ide/tokyo-night-theme.ts; apps/openagents-desktop/src/main.ts; apps/openagents-desktop/src/contracts/ux-contracts.ts; packages/behavior-contracts/src/openagents-apps.ts; docs/ide/ROADMAP.md
hot contracts: DesktopThemeProjection schema; default editor theme registry; pre-paint background invariant; IDE package/Monaco/workbench/review behavior contracts.
verification: Desktop typecheck; theme/contrast/startup/Monaco/Pierre/preferences/design-contract tests; IDE boundary check; production build; visual inspection; full Desktop regression where practical; exact main receipt.
claimed_at: 2026-07-19T14:47:39Z
```

## Color decision

The default uses Khala's `#05070d` background, blue-black surface ladder,
`#3b82f6` energy accent, borders, selection, hover, and focus family. It keeps
Tokyo Night's green, magenta, orange, cyan, red, and yellow syntax roles plus
the adjusted `#8990ad` faint text that already clears the normal-text contrast
gate. Function blue moves onto Khala blue so code and product chrome share the
same energy signal.

Tokyo Night remains registered for Monaco and Pierre and remains available as
an owned fallback projection. Khala editor is the only default mounted theme;
theme choice is still not a mutable preference.

## Rollback

Revert the delivery commit. The retained Tokyo Night projection and adapters
remain independently schema-valid, so restoring it as the default requires no
theme re-import, model migration, or persisted-data migration.

## Verification receipt

- TypeScript typecheck: pass.
- Focused theme/Monaco/Pierre/preferences/startup/accessibility/design suites:
  8 files, 262 assertions passed, 11 intentional skips.
- Full Desktop regression before rebase: 273 files passed; 2,656 assertions passed; 39
  intentional skips. One unrelated 256 MiB history timing assertion first
  measured 55.53 ms against its 50 ms budget, then passed alone and in the
  complete rerun.
- Post-rebase full Desktop regression on the IDE-07/rc.25 main line: 274 files
  passed; 2,658 assertions passed; 39 intentional skips.
- Behavior-contract registry: 36 assertions passed.
- IDE schema/authority boundary: pass.
- Production workbench and lazy editor-island builds: pass.
- Real React Electron smoke: pass, including Files/editor route, offline
  private assets, navigation/reload, and zero active resources after teardown.
- Pixel inspection: Khala void/editor background, blue-black panel ladder, and
  energy-blue active/focus treatment mounted consistently in the real Files
  workbench.
- Contrast on `#05070d`: primary 18.13:1, muted 9.54:1, faint 6.38:1,
  energy blue 5.48:1; retained Tokyo syntax roles range from 7.61:1 to 11.74:1.
- `vp lint --quiet` and `git diff --check`: pass.

## CLAIM-RELEASE

```text
landed: 9d89d663c51affef94bb826f90610cf33bb49b85
verification: post-rebase typecheck; 274 Desktop files / 2,658 assertions passed with 39 intentional skips; 36 behavior-contract assertions; production and lazy-editor builds; compatibility and React Electron smokes; IDE boundary, lint, contrast, and visual gates all green.
residual: user-selectable theme switching, light/high-contrast/system modes, and external theme import remain IDE-18 work; Tokyo Night is retained as the local built-in fallback now.
```
