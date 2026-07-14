# UX-4 MVP visible-surface sweep receipts (#8790)

Owner statement (rc.10 review, 2026-07-14, verbatim): "This menu, when I click
the settings button, looks horrible. This folder thing looks horrible. I
thought we made a pass removing all screens that are not specifically called
for in the MVP. You need to clean all this up and make a pass to remove
everything from the sidebar and all UI that's not specifically called for in
our MVP spec."

Built-Electron smoke receipts (`OPENAGENTS_DESKTOP_SMOKE=1`, fixture
`~/.codex`), captured after the sweep. Derived allowed dock composition (each
with its ProductSpec citation in
`apps/openagents-desktop/src/renderer/mvp-visible-surfaces.ts`):
New chat · Chat · ProductSpec · AssuranceSpec · Project home · Settings.
Removed from the dock: `workspace-files` and the command-palette toggle (both
remain reachable only through their closed CW-AC-12 command identities). The
review and Files surfaces dropped every Git/filesystem mutation affordance to
the CW-AC-14 read-only boundary.

- `01-shell.png` — the cleaned six-icon sidebar (no Files/palette icons; the
  AssuranceSpec entry now carries the Check glyph, not the git-reading
  Compare glyph) with the chat workroom.
- `02-command-palette.png` — the design-passed palette: family-grouped rows
  with hairline dividers and keycap chord captions on the overlay recipe.
- `04-settings-current-codex-session.png` — the design-passed Settings
  column: unified raised-panel chrome, one centered 840px reading measure,
  title-scale headings (Codex session + MAINT-1 Coding harnesses + Desktop
  updates + Keyboard shortcuts).
- `04b-settings-harness-maintenance.png` — MAINT-1 rows resolved with live
  version/channel truth inside the unified panel.
- `04c-diagnostics-panel.png` — diagnostics on the same panel recipe.
- `05-files-workspace.png` — the Files workspace (command-routed; no dock
  icon), grant-bounded tree + bounded editor, no create/rename/delete/reveal
  affordance.
- `10-coding-catalog.png` — Project home.
- `12-git-review-panel.png` — the read-only review boundary: branch/status
  truth, per-file Review, exact diff, Add to composer/Close — and NO commit,
  push, stage, discard, branch, or issue/PR control.
- `14-product-spec.png` — ProductSpec workroom (retained, dock-cited).
- `15-assurance-spec.png` — AssuranceSpec document (owner-directed surface).
- `smoke-step-receipts.log` — the exact smoke step JSON lines, including
  `mvp-visible-surface-allowlist` with `dockExact:true` and the exact rendered
  dock ids, `workspace-files-relative-ui` with `mutationAffordance:null`, and
  `git-review-panel-real-status` with `mutationAffordance:null`.

Mechanical enforcement: `apps/openagents-desktop/src/renderer/
mvp-visible-surfaces.test.ts` walks the ACTUAL rendered shell tree for every
reachable workspace state and fails on any non-allowlisted or lost dock item
or forbidden surface key; its falsifier tests prove planted non-MVP surfaces
are rejected.

Contracts: `openagents_desktop.mvp.visible_surface_allowlist.v1` (amended),
`openagents_desktop.mvp.visible_surface_sweep.v1` (new, owner statement
verbatim) in `apps/openagents-desktop/src/contracts/ux-contracts.ts`.
