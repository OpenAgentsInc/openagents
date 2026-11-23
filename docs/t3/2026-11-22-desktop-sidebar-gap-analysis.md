# OpenAgents Desktop vs. T3 Code Desktop — Sidebar Gap Analysis

- **Date:** 2026-11-22 *(updated post-sidebar overhaul)*
- **OpenAgents repository:** `/Users/christopherdavid/work/openagents` (`origin/main` @ `5e9dc70b08` + local `main` `6ca74c39e3`)
- **T3 Code repository:** `/Users/christopherdavid/work/projects/repos/t3code` (`pingdotgg/t3code`, `main` @ [`18850a5eeb`](https://github.com/pingdotgg/t3code/commit/18850a5eeb388148fc2b8fa3499bf8e624a1154c), `v0.1.12-nightly.20261123.5124`)
- **Hub & policy context:** See [`docs/t3/README.md`](README.md) for the corpus index and [`docs/t3/2026-11-22-t3code-borrowing-theory.md`](2026-11-22-t3code-borrowing-theory.md) for the selective-borrowing policy governing what belongs in `--minimal` vs. what we deliberately reject.
- **Method:** Direct source-level inspection of sidebar layout, interaction, state, host-bridge, styling, and theme/color files in both desktop applications.

---

## 1. Executive Summary & Post-Correction Status

The morning 2026-11-22 audit (`b0fa86d99f`, baseline `3faa6c3efb`) triggered an immediate, empirically tested sidebar overhaul across OpenAgents Desktop (`344fe2d25a`..`5e9dc70b08`, plus `6ca74c39e3`). What happened in practice separated genuine daily-driver upgrades from T3 Code chrome that degraded OpenAgents' clarity:

### What Landed and Stuck in OpenAgents Desktop (`--minimal`)
1. **Default-on collapsible `Working` shelf (`344fe2d25a`):** Active runs lift to the top until they finish or need user attention, clearing the main list.
2. **Calm 2-line `54pt` chronological chat cards (`2f843c40c7` → `56090029c8`, `6c47efc473`, `872c454440`, `d54d78af31`, `1a20e8c28e`, `10b3d427b8`):** Line 1 shows title + icon-only busy spinner (no `"Working"` prose) or relative age (`app_side_age`); Line 2 shows `11pt` folder icon + repo + real branch (suppressing `coder/<uuid>` auto-worktree UUIDs) + persisted dimmed harness icon (Antigravity logo / Psionic mark).
3. **Pure `#000000` canvas & sidebar with exact 1-physical-pixel `#262626` border (`9f6182eb55`):** Replaced both the old `#050505`/`#0a0a0a` split and T3's `#0a0a0a` gray canvas with OLED-black calm (`cp_side_border`).
4. **Flat chronological recency with 5-chat initial cap + `Load more` (`411a000f15`, `05c942c687`):** Deliberately **deleted** project-folder grouping (`−459` LOC) after living with it; sorted active chats strictly by most recent message with a clean `Load more` row and `⌘ N` keycap hint on `New chat`.
5. **Icon-only sidebar footer & reachable Archived view (`dcd9efeb7d`, `6ca74c39e3`):** Bottom utility bar (`cp_side_utility_bar`) shows quiet `28×28pt` icons (`Settings`, plus flag-gated `PRs & Issues` / `Gym`). Archived view is now reachable via `Settings > General → Archived chats` (with a `Back to chats` row at the top of Archived view, fixing the earlier unreachable-view bug).
6. **Faster `Cmd`-hold jump hints & phone-chat sync (`cb96e8196e`, `8da287e936`):** Halved `Cmd`-hold delay for `1..9` jump badges (`1000ms` → `500ms`) and synced phone-originated daemon runs (`ID.phone`) live into the Desktop sidebar.

### What T3 Code Changed Since `b568fcbaa6` (`b568fcbaa6` → `18850a5eeb`)
- **Environment instead of branch (`#40736`):** T3 itself stopped rendering git branch names inline on sidebar thread rows (moving branch + worktree path into `SidebarThreadTooltip`) and now shows the **environment** label instead.
- **Multi-repo projects (`#35925`):** Projects can span multiple tracked repositories (`threadRepositories.ts`).
- **Sidebar folder button & workflows (`#33057`, `#38884`, `#26661`):** Header folder button picks or adds a project; Workflows entered web/mobile navigation; settling a thread no longer jolts scroll position.

### Remaining OpenAgents UX Bugs / High-Value Gaps
1. **Unreachable Snoozed chats (real bug):** Context menus still offer Snooze (`Snooze 1 Hour` / `4 Hours` / `Until Tomorrow` / `Custom…`), hiding the chat via `cp_sb_is_visible_active` with **no Snoozed shelf** to view or wake it early. Fix by either adding a collapsible `Snoozed (N)` shelf or removing Snooze from the `--minimal` context menu.
2. **No narrow-viewport drawer fallback (`< 1080pt`):** Resizing Desktop below `CP_SIDE_FROM` (`1080pt`) hides the sidebar with no slide-over drawer (`drawer_share = 0.0f32`), even though `components/app/sidebar.coax` already has drawer code for iOS.
3. **Calm semantic status differentiation & misclick safety:** Busy/unread states still don't distinguish `Approval` / `Input` / `Failed` on 2-line cards, and there is no `ShiftGuard` (500ms click swallow on involuntary row shifts) or focus-out-of-collapsed-sidebar handling.

### Sidebar Scorecard Across 14 Domains

| # | Domain | OpenAgents Desktop (`5e9dc70b08` + `6ca74c39e3`) | T3 Code Desktop (`18850a5eeb`) | Disposition |
| --- | --- | --- | --- | --- |
| 1 | Shell Geometry, Resize & Responsive Mode | `272pt` default (`242–400pt`), collapses at `<1080pt` (no narrow drawer), `1px` physical hairline (`cp_side_border`) | `256px` default (`max(208px, brandWidth)` min, `viewport - 640px` max), double-click rail reset, `<768px` mobile sheet | **Close narrow drawer + rail reset** |
| 2 | Header Chrome & Top Controls | Transparent macOS titlebar + collapsed island (`toggle` + `search`), `New chat` (`⌘ N`) + flag-gated `Foreman` | Wordmark, Back/Forward, Dev/Nightly artwork, inline Search + Scope + Folder/Project picker (`#33057`) + New Thread | **Keep OpenAgents minimal** |
| 3 | Sidebar Modes & Attention Shelves | Flat chronological: `Working` (default-on collapsible) + `Pinned` + `Recents` (5-chat cap + `Load more`); `Archived` via `Settings > General` + `Back to chats`. Project folders **intentionally removed**. Snoozed shelf missing. | Dual mode: Inbox (`Pinned`, `Active`, `Working`, `Snoozed`, `Settled`) + Project-Folders (multi-repo `#35925`) + Project Scope Filter | **Fix Snoozed gap; reject folder/5-shelf sprawl** |
| 4 | Row Anatomy & Metadata Density | 2-line `54pt` card (`APP_SIDE_CARD_H`): title + icon-only spinner / age / unread dot; Line 2 `11pt` folder + repo + real branch (auto-UUID hidden) + persisted Antigravity/Psionic icon | 4 variants (`78px` card, `36px` slim, `folder`, `folder-multiline`): favicon, pin, 9 statuses + live timer, environment label (`#40736`), PR, Issue, terminal, `+N −M` diff, provider stack | **Keep calm 2-line card** |
| 5 | Status System & Visual Receding | `current`, `busy` (icon-only spinner), `unread` dot, `selected` (`✓`) | 9 statuses (`approval`, `input`, `request`, `preparing`, `working`, `waiting`, `limited`, `failed`, `ready`) + `Woke` + `Done` + visual receding | **Add calm `Approval`/`Input`/`Failed` cues** |
| 6 | Inline Search & Filtering | Titlebar button opens `Cmd+K` modal palette (title-only in `--minimal`; full-text behind `--full`) | Inline `combobox` search input with full-text excerpts + Project Scope popover | **Keep `Cmd+K`; share full-text index later** |
| 7 | Unsent Drafts | Per-chat draft persisted in state; no dedicated sidebar drafts block | Dedicated `SidebarDraftBlock` above threads (`#25272` script-only drafts) | **Non-goal for `--minimal`** |
| 8 | Drag-and-Drop | Pinned-row vertical reorder (macOS only; missing Linux/Win) | `@dnd-kit` cross-shelf drag (`Pinned` ↔ `Active` ↔ `Settled`), drag-to-composer, drop-files-on-row, OS folder drop | **Bring pinned reorder to Linux/Win; skip multi-shelf DnD** |
| 9 | Multi-Select & Bulk Operations | `Cmd`/`Shift`-click range + `52pt` bulk bar (`Clear`, `Pin`, `Archive`/`Restore`, `Delete`) | `Cmd`/`Shift`-click + bulk context menu + pointer action-sweep | **Sufficient** |
| 10 | Context Menus | Chat row menu (Pin, Archive, Snooze, Rename, Fork, Delete) + Foreman menu | Thread (16+ items), Multi-select, Project folder, Draft menus | **Clean up Snooze; keep lean** |
| 11 | Hover Quick-Actions, Rename & Hover Cards | Hover wash; rename via modal dialog; no hover card | Hover swaps timestamp for Snooze/Settle; `F2`/double-click inline rename; rich `ThreadHoverCard` (branch/worktree `#40736`, PRs, issues) | **Optional inline rename; skip hover card** |
| 12 | List Motion & ShiftGuard | Static reorder (no FLIP, no misclick guard) | 150ms FLIP + 500ms `SidebarShiftGuard` anti-misclick pointer swallow (`#26661` settle jolt fix) | **Borrow `ShiftGuard` invariant** |
| 13 | Footer Chrome & Undo Notice | Icon-only utility bar (`Settings` + flag-gated `PRs & Issues` / `Gym`) + Update pill + floating 6s Undo toast | Utility icon bar (`Settings`, `PRs`, `Issues`, `Usage`, `Workflows` `#38884`) + inline `Mod+Z` Undo notice + Desktop & Provider Update pills | **Keep ultra-quiet footer; consider `Cmd+Z` undo** |
| 14 | Visual Design, Colors & Theming | Pure `#000000` canvas & sidebar + `1px` `#262626` border (`--minimal`); 5-skin engine & VS Code JSON gated OFF (`DC_FLAG_CUSTOMIZE`) | 61 theme roles incl. 8 sidebar tokens, 5 dual-appearance OKLCH palettes + OpenVSX + 100 KB custom CSS | **Keep OLED `#000000` native restraint; add distinct selected-row & focus-ring tokens** |

---

## 2. Layout Geometry & Responsive Shell

- **OpenAgents (`apps/openagents-desktop/page/layout/layout.coax`, `sidebar.coax`, `titlebar.coax`, `input.coax`):**
  - Coax → Metal (macOS) / Vulkan (Linux, Windows), 120 Hz ProMotion, zero DOM.
  - Default width `272pt` (`242–400pt`), `4pt` drag resize edge, `Cmd+B` animated slide collapse, pure `#000000` background with crisp 1-physical-pixel `#262626` border (`cp_side_border`), collapsed transparent titlebar island (`toggle` + `search`).
  - **Gap:** Below `1080pt` (`CP_SIDE_FROM`), sidebar vanishes with no overlay drawer fallback; resize rail lacks double-click-to-reset; collapsing sidebar does not explicitly shift keyboard focus out of the hidden sidebar.
- **T3 Code (`AppSidebarLayout.tsx`, `threadSidebarWidth.ts`, `SidebarChrome.tsx`):**
  - Default `256px` (`max(208px, brandWidth)` min, `viewport - 640px` max), double-click `<SidebarRail>` resets to `256px`, `<768px` switches to a `<Sheet>` overlay drawer, `moveFocusOutOfCollapsingSidebar()` prevents trapped focus.

---

## 3. Organization, Row Anatomy & Status Semantics

### OpenAgents Desktop (`page/layout/sidebar.coax`, `components/app/nav.coax`)
- **Top action:** `New chat` (`ICON_COMPOSE`, `⌘ N` keycap chips via `cp_side_action_keys`) + flag-gated `Foreman`.
- **Sections (Active view):**
  1. **`Working` (collapsible, default-on when `nworking > 0`):** Active running chats move here with count badge (`EL_S_BUSY`) until they finish or need user input.
  2. **`Pinned` (when `npinned > 0`):** Pinned active chats, drag-reorderable on macOS.
  3. **`Recents`:** Flat chronological list sorted by most recent message (including synced phone daemon runs from `~/.openagents/coder/host/runs/*.phone`). Capped at **5 visible chats** initially with a `Load more (N more)` row (`CP_SB_LOAD_MORE_ACT`).
- **Archived view:** Entered from `Settings > General → Archived chats` (`6ca74c39e3`), starting with a `Back to chats` (`ICON_SET_BACK`) row above the `Archived` list.
- **Card anatomy (`APP_SIDE_CARD_H = 54pt`):**
  - **Line 1:** Truncated title (`Geist Medium 13pt`) + right-side status slot: `"✓"` when multi-selected, `15pt` icon-only spinner when busy, `6pt` unread dot, or relative age (`app_side_age`), plus floating `1..9` keycap badges after `500ms` `Cmd` hold (`cb96e8196e`).
  - **Line 2:** `11pt` folder icon (`ICON_FOLDER_11`) + repo basename + real git branch (auto-generated `coder/<uuid>` worktree branches hidden) + dimmed persisted harness icon (Antigravity logo or `ICON_PSIONIC`).
- **Footer (`cp_side_utility_bar`):** Optional `Update` capsule pill, `1px` hairline, then quiet `28×28pt` icon-only `Settings` button (plus flag-gated `PRs & Issues` and `Gym` in `--full`).

### T3 Code Desktop (`Sidebar.tsx`, `SidebarThreadRow.tsx`, `Sidebar.logic.ts`)
- **Two modes:** Inbox/Shelves (`Unsent Drafts` + `Pinned` + `Active` + collapsible `Working` / `Snoozed` / `Settled`) and Boring/Project-Folders (multi-repo `#35925`), plus searchable Project Scope popover.
- **5 row variants:** `78px` cards, `36px` slim rows (Snoozed/Settled), `36px` folder rows, `54px` multi-line folder rows, and `78px` draft rows. As of `#40736`, thread rows display the **environment** label rather than the git branch (branch + worktree path live inside `ThreadHoverCard`).
- **9 semantic statuses** (`approval`, `input`, `request`, `preparing`, `working`/`goal`, `waiting`, `limited`, `failed`, `ready`) + `Woke` + `Done`, `shouldRecedeSidebarThread()` dimming on quiet/working rows, `500ms` `SidebarShiftGuard`, `150ms` FLIP motion (`#26661` settle jolt fix), and `200ms` `Mod+1..9` hints.

---

## 4. Visual Design, Colors & Accessibility

- **OpenAgents `--minimal` default:** Pure `#000000` canvas and sidebar, `1px` `#262626` right border, monochrome `Geist` typography (`13pt` titles, `12pt` metadata), `11pt` folder + dimmed (`0.46`) harness icon, native macOS VoiceOver semantics (`NSAccessibilityElement`). The 5-skin theme engine + VS Code JSON import (`page/customize.coax`) remains flag-gated OFF (`DC_FLAG_CUSTOMIZE`).
  - **Remaining visual gaps:** Active (`s.raised`), hovered (`s.raised @ 75%`), and multi-selected rows still share the same base token; sidebar rows lack a visible keyboard focus ring; Linux AT-SPI and Windows UIA accessibility lag macOS VoiceOver.
- **T3 Code:** 61 semantic roles (8 dedicated sidebar tokens), 5 dual-appearance OKLCH palettes + T3 Dark/Light (`#000000` sidebar / `#0a0a0a` canvas), continuous contrast boost, OpenVSX marketplace, 100 KB custom CSS, full cross-platform ARIA.

---

## 5. Updated Action Plan (Aligned with Borrowing Theory)

Per [`2026-11-22-t3code-borrowing-theory.md`](2026-11-22-t3code-borrowing-theory.md), only high-signal bugs, safety invariants, and calm daily-driver ergonomics belong in `--minimal`.

### Already Closed Since Morning 2026-11-22
- ~~Wire Archived view toggle~~ → **Done** (`Settings > General → Archived chats` + `Back to chats`, `6ca74c39e3`).
- ~~Add `Working` shelf~~ → **Done** (`344fe2d25a`).
- ~~Upgrade single-line rows to show repo, branch, relative age, harness icon~~ → **Done** as calm 2-line `54pt` cards (`2f843c40c7` → `56090029c8`, `6c47efc473`, `1a20e8c28e`, `10b3d427b8`).
- ~~Quiet icon-only sidebar footer~~ → **Done** (`dcd9efeb7d`, `6ca74c39e3`).
- ~~Reduce `1000ms` `Cmd`-hold jump-badge delay~~ → **Done** (`500ms`, `cb96e8196e`).
- ~~Pure black sidebar + crisp `1px` border~~ → **Done** (`9f6182eb55`).
- ~~Sync phone-originated daemon chats into sidebar~~ → **Done** (`8da287e936`).

### P0 — Fix Remaining Broken / Missing Mechanics
1. **Resolve unreachable Snoozed chats (`sidebar.coax`, `Window+Menus.swift`):** Either add a collapsible `Snoozed (N)` shelf (shown only when `cp_sb_snoozed_count > 0`) with wake times and Wake action, **or** remove the Snooze items from the `--minimal` context menu so users cannot hide chats into an invisible state.
2. **Narrow-viewport overlay drawer fallback (`layout.coax`):** Below `1080pt` (`CP_SIDE_FROM`), let `Cmd+B` / titlebar toggle open the existing `app_sidebar` overlay drawer with scrim instead of making chats unreachable.

### P1 — Calm Signal & Safety Invariants (No Card Bloat)
3. **Calm semantic status cues on 2-line cards (`nav.coax`, `sidebar.coax`):** Distinguish `Approval` (amber), `Input` (indigo/cyan), and `Failed` (red) with subtle icon/dot cues and quiet receding on already-read rows — without expanding cards back to 3 lines or adding status prose.
4. **Safety & keyboard invariants (`input.coax`, `layout.coax`, `skin.coax`):**
   - Port T3's `500ms` `ShiftGuard` involuntary-row-shift click swallow (especially important as chats move into/out of `Working`).
   - Move keyboard focus out of the sidebar when collapsed (`Cmd+B`), paint a crisp focus ring on keyboard-focused sidebar rows, separate multi-selected row fill from active row fill, and add double-click-to-reset on the sidebar resize rail.
5. **Cross-platform parity:** Bring pinned-chat drag reorder and consistent keyboard/accessibility semantics to Linux (Wayland/AT-SPI) and Windows (Win32/UIA).

### Deliberately Rejected (Do Not Port)
- Project-folder sidebar trees and multi-repo project trees (tested and intentionally deleted in `411a000f15`).
- 3-line `78px` status-prose cards with live ticking timers, PR/Issue/terminal badges, and hover-action swaps.
- 5-shelf inbox choreography (`Settled`, custom snooze presets, cross-shelf DnD, FLIP clone pools, hover cards, unsent-drafts block).
- Inline sidebar search combobox (keep `Cmd+K`), 61-token OpenVSX theme marketplace, and 100 KB custom CSS.
