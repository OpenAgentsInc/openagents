# OpenAgents Desktop vs. T3 Code Desktop — 100% Complete Sidebar Gap Analysis

- **Date:** 2026-11-22
- **OpenAgents repository:** `/Users/christopherdavid/work/openagents` (`main` @ `3faa6c3efb`)
- **T3 Code repository:** `/Users/christopherdavid/work/projects/repos/t3code` (`pingdotgg/t3code`, `main` @ `b568fcbaa6`, `v0.1.12-nightly`)
- **Prior T3 Code audits reviewed in this repo:**
  - `docs/research/t3code/README.md`, `01-apps-and-connections.md`, `02-orchestration.md`, `07-openagents-path.md`, `08-source-map.md`, `09-current-gap-analysis.md` (pinned to T3 Code `0.1.9` `f313d0a6cb`)
  - `docs/audits/2026-11-20-t3code-comprehensive-gap-analysis.md` (T3 Code `8cfd2b5211` vs OpenAgents `855654fc81`)
  - `docs/audits/2026-11-20-openagents-desktop-vs-t3code-desktop-gap-analysis.md` (T3 Code `b568fcbaa6` vs OpenAgents `90e3f9361a`)
  - `docs/audits/2026-11-20-t3code-antigravity-v0-1-11-audit.md` (T3 Code `d15123668e`)
- **Method:** Direct source-level inspection of every sidebar layout, interaction, state, host-bridge, styling, and theme/color file in both desktop applications.

---

## 1. Executive Summary

Since the 2026-11-20 desktop audit, **OpenAgents Desktop** has closed several foundational sidebar gaps (`#13875` and `#13887`): it lifted the 50-chat cap to **512**, added **project-folder grouping** with collapsible headers and tree guide rails (`page/sidebar_groups.coax`), added **Cmd/Shift multi-select** with a bulk action bar, added **Archive/Snooze context menu items** and a **6-second Undo toast**, and added a **5-skin theme engine** (`Coder Noir`, `Daylight`, `Grove`, `Ocean`, `Ember`) plus high-contrast mode and VS Code JSON import (`page/customize.coax`).

However, a line-by-line comparison against **T3 Code Desktop** (`apps/web/src/components/Sidebar.tsx`, `SidebarThreadRow.tsx`, `SidebarChrome.tsx`, `AppSidebarLayout.tsx`, `themePalettes.ts`) reveals a **deep structural, functional, and visual gap** — plus **two immediate UX bugs** in OpenAgents Desktop's newly wired archive/snooze code:

1. **Immediate OpenAgents UX Bugs Found in Source:**
   - **Archived chats are unreachable in the UI:** `sidebar_groups.coax` implements `CP_SB_VIEW_ARCHIVED` / `CP_SB_VIEW_ARCHIVED_ACT` and `host.c` exports `cd_sb_set_view`, but **no button, tab, menu item, or shortcut anywhere in `sidebar.coax`, `Window+Menus.swift`, or `AppDelegate.swift` switches `view_mode` to `CP_SB_VIEW_ARCHIVED`**. Once the 6-second Undo toast expires, an archived chat cannot be viewed or restored in the app.
   - **Snoozed chats are invisible until wake:** Snoozing hides a chat via `cp_sb_is_visible_active`, and there is **no Snoozed shelf** in the sidebar to see snoozed chats or wake them early.

2. **Fundamental Architectural Difference:**
   - **OpenAgents Desktop** renders a **compact, single-line, Antigravity/Claude-Desktop-inspired list** (`32pt` rows, title + busy spinner or unread dot, monochrome Coder Noir `#050505` surface, native Metal/Vulkan GPU at 120 Hz). It is fast and clean, but information-sparse.
   - **T3 Code Desktop** renders a **rich multi-mode developer command center** (`78px` 3-line cards in Inbox/Shelf mode, or `36px` slim/folder rows in Project-Folder mode) with **5 attention shelves** (Pinned, Active, Working, Snoozed, Settled), **9 semantic thread statuses** with live ticking timers, **inline full-text search**, **unsent drafts block**, **project scope filtering**, **custom project favicons/icons**, **branch + worktree + PR + Issue + terminal + `+ins −del` diff + provider stack metadata**, **hover quick-actions**, **rich hover cards**, **cross-shelf drag-and-drop**, **500ms ShiftGuard misclick protection**, **150ms FLIP list motion**, **8 dedicated sidebar color tokens** per theme, and **61 total theme roles** with OKLCH perceptual palettes and 100 KB custom CSS.

### Sidebar Scorecard Across 14 Domains

| # | Domain | OpenAgents Desktop (`3faa6c3efb`) | T3 Code Desktop (`b568fcbaa6`) | Gap Severity |
| --- | --- | --- | --- | --- |
| 1 | Shell Geometry, Resize & Responsive Mode | `272pt` default (`242–400pt`), collapses at `<1080pt` (no narrow drawer on desktop), smooth slide animation | `256px` default (`208px` / brand-width min, dynamic max `viewport - 640px`), double-click rail reset, `<768px` mobile sheet | **Medium** |
| 2 | Header Chrome & Top Controls | Transparent macOS titlebar + collapsed island (`toggle` + `search`), `New chat` + gated `Foreman`/`Gym`/`PRs & Issues` | `T3 Code` wordmark, Back/Forward history buttons, Dev/Nightly SVG stage artwork/pill, inline Search + Scope + Add Project + New Thread | **Large** |
| 3 | Sidebar Modes & Attention Shelves | Single mode: `Pinned` + `Projects` (collapsed by default) + `General`/`Recents`. No Working/Snoozed/Settled shelves; Archived view unreachable | Dual mode: **Inbox/Shelves** (`Pinned`, `Active`, `Working`, `Snoozed`, `Settled`) + **Boring/Project-Folders** mode + **Project Scope Filter** | **Critical** |
| 4 | Row Anatomy & Metadata Density | Single-line `32pt` row: title + busy spinner OR unread dot + `· F` tag | 4 variants (`78px` card, `36px` slim, `folder`, `folder-multiline`): favicon, pin, 9 statuses + live timer, branch/worktree, PR, Issue, terminal, `+N −M` diff, provider stack, remote machine icon | **Critical** |
| 5 | Status System & Visual Receding | 3 states (`current`, `busy`, `unread` dot) | 9 statuses (`approval`, `input`, `request`, `preparing`, `working`, `waiting`, `limited`, `failed`, `ready`) + `Woke` + `Done` + `shouldRecedeSidebarThread` dimming + 4-tier closed-folder dot | **Critical** |
| 6 | Inline Search & Filtering | Titlebar button opens `Cmd+K` modal palette (title-only); no inline sidebar search, no project scope filter | Inline `combobox` search input in sidebar header: ranked title + branch + PR + full-text message search with highlighted excerpts; searchable Project Scope popover | **Large** |
| 7 | Unsent Drafts | No sidebar representation for unsent drafts | Dedicated `SidebarDraftBlock` above threads with prompt/attachment preview, project favicon, discard button, context menu, frozen active row | **Medium** |
| 8 | Drag-and-Drop | Pinned-row vertical reorder (macOS only; missing Linux/Win) + OS folder drop to add project | `@dnd-kit` cross-shelf drag (`Pinned` ↔ `Active` ↔ `Settled`) with projected layout, dashed placeholders, boundary labels, drag-thread-to-composer, drop-files-on-thread, OS folder drop | **Large** |
| 9 | Multi-Select & Bulk Operations | `Cmd`-click / `Shift`-click range + top `52pt` bulk bar (`Clear`, `Pin`, `Archive`/`Restore`, `Delete`) | `Cmd`-click / `Shift`-click range + bulk context menu (`Unpin`, `Settle`, `Snooze` submenu + custom, `Regenerate title`, `Mark unread`, `Delete`) + drag-sweep actions | **Medium** |
| 10 | Context Menus | Chat row menu (9 items + Foreman row menu); no project-folder context menu | 4 menus: Thread (16+ items incl. Copy submenu, Auto-settle, Regenerate title, Guest share, Turn into project, Filter by project), Multi-select, Project folder, Draft | **Large** |
| 11 | Hover Quick-Actions, Rename & Hover Cards | Hover wash only; rename via modal dialog; no hover card | Hover swaps timestamp/status for Snooze menu + Settle/Unsettle/Wake + Discard draft; `F2`/double-click inline `<input>` rename; rich `ThreadHoverCard` | **Large** |
| 12 | List Motion & ShiftGuard | Static list reorder (no row FLIP animation, no misclick guard) | `SidebarListMotion` (150ms Web Animations FLIP + cloned fade-out) + `SidebarShiftGuard` (500ms anti-misclick pointer swallow on involuntary row shifts) | **Medium** |
| 13 | Footer Chrome & Undo Notice | Bottom-left `Settings` row + `Update` pill + bottom-right floating 6s Undo toast | Utility icon bar (`Settings`, `PRs`, `Issues`, `Usage`, `Back`) + inline non-overlapping Undo notice (`Mod+Z` capture) + Desktop & Provider Update pills + Arch warning | **Medium** |
| 14 | Visual Design, Colors & Theming | 5 built-in themes + high contrast + VS Code JSON (6 tokens); `1` sidebar surface token (`#050505` in Noir); shared `raised` (`#191919`) for hover/active; monochrome accent `#ededed` | 61 theme roles incl. **8 dedicated sidebar tokens** (`sidebar`, `sidebarForeground`, `sidebarMutedForeground`, `sidebarControlSurface`, `sidebarRowHover`, `sidebarRowActive`, `sidebarRowSelected`, `sidebarBorder`); 5 OKLCH light+dark built-ins + T3 Dark/Light + contrast boost + VS Code/OpenVSX + 100KB CSS | **Large** |

---

## 2. Architecture, Layout Geometry & Responsive Shell

### Source Files Compared
- **OpenAgents:** `apps/openagents-desktop/page/layout/layout.coax`, `sidebar.coax`, `titlebar.coax`, `state.coax`, `input.coax`, `components/app/sidebar.coax`
- **T3 Code:** `apps/web/src/components/AppSidebarLayout.tsx`, `threadSidebarWidth.ts`, `components/ui/sidebar.tsx`, `components/sidebar/SidebarChrome.tsx`

| Property | OpenAgents Desktop | T3 Code Desktop | Gap / Notes |
| --- | --- | --- | --- |
| **Rendering Stack** | Coax → Metal (macOS) / Vulkan (Linux, Windows), 120 Hz ProMotion, zero DOM | Electron + React 19 + Tailwind CSS v4 + `@base-ui/react` + `@dnd-kit` | OpenAgents has vastly lower RAM/CPU overhead and native 120 Hz rendering. |
| **Default Width** | `272pt` (`CP_SIDE :: 272`) | `256px` (`16rem`, `THREAD_SIDEBAR_DEFAULT_WIDTH = 16 * 16`) | Comparable; OpenAgents is `16pt` wider by default. |
| **Minimum Width** | `242pt` (`CP_SIDE_LEAST :: 242`) | `max(208px, ceil(brandWidth))` (`13rem`, dynamically measured via `SidebarBrandWidthProbe`) | T3 prevents brand/history controls from ever clipping on resize. |
| **Maximum Width** | Fixed `400pt` (`CP_SIDE_MOST :: 400`), clamped by `win_w - pane_w - 456` | Dynamic `viewportWidth - 640px` (`THREAD_MAIN_CONTENT_MIN_WIDTH = 40 * 16`) | T3 lets users widen the sidebar arbitrarily on ultrawide monitors while protecting `640px` main content. |
| **Resize Handle** | `4pt` hit zone (`CP_EDGE :: 4`) along right edge (`cp_edge` in `input.coax`); persists width via host | `<SidebarRail>` full-height drag handle (`useResizeDrag`); **double-click resets width to `256px`**; persists in `localStorage` (`chat_thread_sidebar_width`) | OpenAgents lacks double-click-to-reset on the divider rail. |
| **Collapse Behavior** | `Cmd+B` or titlebar toggle; slides out left via `app_presence` keyed animation (`app_slide`) | `Mod+B` or fixed top-left `SidebarTrigger`; CSS `offcanvas` slide with `--ease-drawer: cubic-bezier(0.32, 0.72, 0, 1)`, configurable `panelAnimationDurationMs`, focus moved out of collapsing `inert` sidebar | T3's `moveFocusOutOfCollapsingSidebar()` prevents trapped keyboard focus and announces collapse to screen readers. |
| **Narrow Viewport Breakpoint** | `< 1080pt` (`CP_SIDE_FROM :: 1080`): **sidebar disappears completely** (`drawer_share = 0.0f32` passed to `app_shell_layout`); no drawer fallback on desktop | `< 768px` (`(max-width: 767px)`): switches to a slide-over `<Sheet>` drawer (`calc(100vw - 12px)` wide) | **Gap:** Resizing an OpenAgents Desktop window below `1080pt` hides the sidebar with no slide-over drawer to access chats (even though `components/app/sidebar.coax` already has `app_sidebar` drawer code for iOS). |
| **macOS Traffic Lights & Titlebar** | Transparent inset titlebar (`38pt` tall, line at `20pt`); traffic lights at `14–74pt`; toggle at `88pt`, search at `116pt`; collapsed state paints an opaque `12pt`-radius **island** (`th.menu` + `shadow_sm`) behind lights + toggle + search | `--workspace-topbar-height: 52px` (`40px` in Electron WCO); `--desktop-window-controls-inset: 90px` on macOS (clears in fullscreen via `getWindowFullscreenState` IPC); fixed `SidebarTrigger` (`z-50`) stays anchored at `--workspace-controls-left` | Both handle macOS fullscreen traffic-light clearance cleanly; OpenAgents' collapsed titlebar island is a strong native macOS touch. |
| **Right Border** | `1pt` vertical hairline (`k.menu_divider` = `white @ 7%`) | `1px` `border-sidebar-border` (`#141414` dark / `#e4e4e7` light) | T3's border is a dedicated per-theme token (`sidebarBorder`). |

---

## 3. Sidebar Modes, Organization & Information Architecture

### OpenAgents Desktop (`page/layout/sidebar.coax`, `page/sidebar_groups.coax`)
OpenAgents has **one layout mode** (with an unreachable Archived view):
1. **Top Action Stack:**
   - `New chat` (`ICON_COMPOSE`, `CP_NEW`)
   - Feature-flagged destinations (OFF in Inkling v1 release): `Foreman` (`ICON_BOT` + tier/busy badge), `Gym` (`ICON_WIDGET`), `PRs & Issues` (`ICON_SHARE`)
2. **Conditional Bulk Action Bar** (`52pt` height when `sel_count > 0`): `"N selected"`, `Clear`, `Pin`, `Archive`/`Restore`, `Delete`.
3. **Scrollable List (`CP_CHATS :: 512`):**
   - **Pinned** section (if `npinned > 0`): flat list of pinned active chats.
   - **Projects** section (always shown in Active mode): heading `"Projects"` with trailing `ICON_FOLDER_ADD` (`+`) button.
     - If `has_projects`: renders up to `64` (`CP_SB_MAX_GROUPS`) collapsible project folder rows (collapsed by default, `collapsed = true`), showing basename, `ICON_FOLDER`, chat count, and disclosure chevron (`ICON_CHEVRON_RIGHT` / `ICON_CHEVRON`). Expanded projects show up to `10` (`CP_SB_GROUP_LIMIT`) indented chats with a `1pt` vertical tree rail (`white @ 8%`), plus a `"See all"` row if `> 10`.
     - Trailing **General** section for non-pinned active chats with no folder.
     - If `!has_projects`: flat **Recents** section.
   - **Tail hint:** `"Your chats show here."`, `"No archived chats."`, or `"N older chats are not shown."`
   - **Missing / Unwired:**
     - `CP_SB_VIEW_ARCHIVED` exists in `sidebar_groups.coax` but has **no UI trigger**.
     - Snoozed chats are filtered out by `cp_sb_is_visible_active`, with **no Snoozed shelf** to inspect or wake them.
     - No **Working** or **Settled** shelves.
     - No **Project Scope Filter** to focus the sidebar on one or more repos.

### T3 Code Desktop (`Sidebar.tsx`, `Sidebar.logic.ts`)
T3 Code has **two full sidebar modes** (toggleable in Settings → Threads) plus **multi-project scope filtering**:
1. **Mode A — Inbox / Attention-Shelves Mode (Default):**
   - **Unsent Drafts Block (`SidebarDraftBlock`):** Always pinned above threads with a subtle divider (`bg-sidebar-border/60`), listing unsent drafts newest-first.
   - **Pinned Section:** User-ordered pinned threads (full `78px` cards), separated from Active by a drag boundary (`pinned-divider`).
   - **Active Section:** Threads needing attention or recently active (full `78px` cards), sorted by return/activity order.
   - **Working Shelf (`working-header`, collapsible):** Threads currently running in the background (when working shelf is enabled), showing count badge; rows use `78px` cards.
   - **Snoozed Shelf (`snoozed-header`, collapsible, persisted in `localStorage`):** Snoozed threads as `36px` slim rows showing `<AlarmClockIcon />` + wake time (e.g., `"Tomorrow 9am"`). Uses an exact deadline clock (`useSnoozeWakeClock`) so threads automatically wake and jump back to Active with a `"Woke"` status badge at the exact second their wake time passes (even across laptop sleep).
   - **Settled Shelf (`settled-header`, collapsible, persisted in `localStorage`):** Completed/settled threads as `36px` slim rows, paginated on demand (`olderSettledShelf` + "Show more"), with active route thread always kept visible even when collapsed.
2. **Mode B — Boring / Project-Folders Mode:**
   - Groups active threads into collapsible project folders (`SidebarProjectFolderHeader.tsx`), preserving folder open/closed state per project key.
   - Supports **multi-environment project grouping** (same repo across local + SSH/remote machines grouped or separated per `selectProjectGroupingSettings`).
   - Closed folders display a **priority status dot** (`bg-warning` attention > `bg-error` failed > `bg-success` unread/done > `bg-info` working) + hidden thread count (`tabular-nums`), which swaps on hover for a `New thread in {project}` (`SquarePenIcon`) button.
   - Folder child rows support both single-line (`folder`, `36px`) and multi-line (`folder-multiline`, `54px`) layouts.
3. **Project Scope Filter (`projectScope` in `SidebarThreadHeader.tsx`):**
   - Searchable popover in the sidebar header to filter the entire sidebar (drafts, shelves, search) to `"All projects"` or any subset of projects, with per-project pin/settings actions.

---

## 4. Header Chrome & Top Action Area

| Element | OpenAgents Desktop | T3 Code Desktop |
| --- | --- | --- |
| **Brand / Wordmark** | None in sidebar header | `T3` SVG wordmark + `"Code"` (`text-sm font-medium tracking-tight`, clickable link to `/`) |
| **Back / Forward Navigation** | None (`titlebar.coax` explicitly notes: *"No back or forward"*) | Electron desktop Back (`ArrowLeftIcon`) & Forward (`ArrowRightIcon`) history buttons with keyboard shortcut tooltips + trackpad swipe (`HistorySwipeNavigation`) |
| **Environment Stage Identification** | None | `SidebarStageBackdrop`: full-bleed `80px`-tall SVG artwork behind header for `nightly` (starry night sky) and `dev` (blueprint grid), or compact `Dev`/`Nightly` badge pill (`EnvironmentIdentificationMode`) |
| **Sidebar Toggle** | `24×24pt` quiet icon button (`ICON_SIDEBAR`) in titlebar at `x = 88pt` (`12pt` fullscreen) | `28×28px` `SidebarTrigger` (`PanelLeftCloseIcon` / `PanelLeftIcon`) fixed at `--workspace-controls-left` with shortcut tooltip |
| **Search Control** | `24×24pt` magnifying glass icon in titlebar — opens global `Cmd+K` modal palette | **Inline `32px` search combobox** right inside the sidebar header (`SearchIcon`, native `<input type="search">`, `XIcon` clear button, `aria-activedescendant` keyboard navigation) |
| **Add Project Button** | Trailing `22×22pt` `ICON_FOLDER_ADD` button on the `"Projects"` section heading | `28×28px` `FolderPlusIcon` button in the sidebar header toolbar (`SidebarThreadHeader`) |
| **New Thread / Chat Button** | Full-width `32pt` action row (`ICON_COMPOSE` + `"New chat"`) at top of sidebar | `28×28px` `SquarePenIcon` icon button in header toolbar; **Shift+click** creates a new thread in the current project (with multi-line tooltip showing both shortcuts) |
| **Special Destination Rows** | Flag-gated `Foreman` (with live managed-busy count + tier label + `` ` `` keycap), `Gym`, `PRs & Issues` | None in top stack — `Pull requests`, `Issues`, `Usage`, `Settings` live in the **footer utility icon bar** |

---

## 5. Row Anatomy, Variants & Metadata Density

This is the **single largest functional and visual gap** between the two sidebars.

### OpenAgents Desktop Row (`components/app/nav.coax`, `page/layout/sidebar.coax`)
- **Variants:** 1 chat row variant (`32pt` single-line; `48pt` two-line exists in `nav.coax` but is **never used** by Desktop — `detail` is always `""`).
- **Left side:** Optional indent (`23pt`) + `1pt` vertical tree line (`white @ 8%`) when inside an expanded project folder. No project favicon, no pin icon, no draft icon.
- **Center:** Single-line truncated chat title (`Geist Medium 13pt / 18pt line`), optionally suffixed with `" · F"` if Foreman-managed.
- **Right side:**
  - Multi-select: `"✓"` text + accent dot (`6pt` circle).
  - Running: `15pt` animated monochrome 3×3 dot spinner (`activity()`).
  - Unread (not current, not busy): `6pt` solid accent dot (`#ededed` in Coder Noir).
  - Holding `Cmd` (`>1s`): floating `18pt` keycap badge (`1`..`9`, `` ` ``) painted in `over` layer.
  - **Missing on rows:** No timestamp/age (even though `app_side_age` exists in `nav.coax`!), no git branch, no worktree indicator, no diff stats, no PR/Issue status, no terminal indicator, no provider/model icon, no hover quick-actions, no hover card, no inline rename.

### T3 Code Desktop Row (`components/sidebar/SidebarThreadRow.tsx`)
T3 Code renders **5 distinct row layouts**, all with `[content-visibility:auto]` virtualization hints and viewport-leased live subscriptions (`useSidebarRowSubscriptionLease`, `160px` overscan):

1. **Card Row (`variant="card"`, `78px` / `4.875rem`, or `60px` when meta line is empty) — Pinned, Active, Working:**
   - **Line 1 (`h-5`, `20px`):**
     - Unsent draft pen icon (`SquarePenIcon`) if thread has an unsent reply draft
     - `16×16px` `ProjectFavicon` (auto-detected repo favicon, custom Lucide icon in 20 colors, emoji, or monogram)
     - `PinIcon` (`12px`, clickable to unpin on hover → `PinOffIcon`)
     - Project display name (`12px`, `text-secondary-label`, truncated)
     - **Right status slot:** Shows either **Top Status** (icon + label + live ticking duration `1m 24s` for Working/Preparing/Waiting) OR **Relative Time** (`now`, `5m`, `3h`, `2d`, updated via shared minute clock `useNowMinute`). On hover (or keyboard focus), smoothly swaps to **Quick Actions**: Discard Draft (`XIcon`), Snooze Menu (`AlarmClockIcon` dropdown with presets + Custom), and Settle (`CheckIcon`). While `Cmd`/`Ctrl` is held (`200ms` delay), swaps to `<Kbd>` jump hint badge (`1`..`9`).
   - **Line 2 (Title):** `14px` (`text-sm font-medium`), `dir="auto"`, inline `<input>` when renaming (`F2` or double-click), shimmer overlay (`ActivityShimmerOverlay`) when regenerating title.
   - **Line 3 (Metadata strip, `12px`, `text-xs text-secondary-label`, omitted if empty):**
     - `ThreadWorktreeIndicator` (worktree vs local branch icon) + `<MiddleTruncate>` git branch name (or remote scratch machine icon + label)
     - Running terminal process indicator (`TerminalIcon` + count tooltip)
     - Linked Pull Request badge (`ThreadPullRequestBadgeControl` — open/draft/merged/closed/CI status, clickable to open in right panel)
     - Linked Issue badge (`IssueGlyph` + state color)
     - Latest run diff stats in monospace: `<span class="text-diff-addition-foreground">+142</span> <span class="text-diff-deletion-foreground">−38</span>`
     - Right-aligned `EnvironmentMachineIcon` (if remote SSH/Tailscale/WSL environment) + `SidebarProviderStack` (current provider icon + grayscale history icons if thread switched providers, with multi-account accent badge).
2. **Slim Row (`variant="slim"`, `36px` / `h-9`) — Snoozed & Settled shelves:**
   - Single flex row: grayscale-until-hover `16×16px` `ProjectFavicon`, draft icon, title, pin, terminal icon, issue badge, right-aligned wake time (`<AlarmClockIcon />` in `text-info-foreground`) or settled age, swapping on hover to Wake (`AlarmClockOffIcon`) or Un-settle (`Undo2Icon`).
3. **Folder Row (`variant="folder"`, `36px`, `ps-9`) — Boring / Project-Folders compact:**
   - Indented `36px` to align with parent folder name; shows status icon+label in the time slot when active/needs attention.
4. **Folder Multi-line Row (`variant="folder-multiline"`, `~54px`, `ps-9`) — Boring / Project-Folders detailed:**
   - Line 1: title + status/time/hover-actions; Line 2: full branch/worktree/PR/issue/diff/provider metadata line.
5. **Unsent Draft Row (`SidebarDraftRow`, `78px`):**
   - `SquarePenIcon` + `ProjectFavicon` + project name + hover `XIcon` discard button + prompt/attachment preview (`"2 attachments"` when prompt text is empty).

### Rich Hover Card (`SidebarThreadTooltip` → `ThreadHoverCard`)
Hovering any T3 Code thread row opens a structured right-anchored card (`side="right"`, `sideOffset={4}`) displaying:
- Full untruncated thread title
- Project favicon + name
- Remote environment machine icon + name
- Full git branch name + **warning banner if local checkout is on a different branch** (`"You're currently checked out on another branch."`)
- Provider icon + instance badge + model name + previous providers used in handoffs
- Running terminal process count
- Linked issues list (`ThreadDetailsIssueRows`)
- Footer with all linked Pull Requests (`ThreadPullRequestsMiniList`)

OpenAgents Desktop has **no hover card or tooltip** on sidebar rows.

---

## 6. Status System, Attention Semantics & Visual Receding

| Status / State | T3 Code Desktop (`Sidebar.logic.ts`, `SidebarThreadRow.tsx`) | OpenAgents Desktop (`sidebar.coax`, `nav.coax`) |
| --- | --- | --- |
| **Approval Needed** | `approval` — `ShieldQuestionIcon` + `"Approval"` in `text-warning-foreground` (`#ffb900` dark / `#bb4d00` light); priority #1 | **None** (shows generic busy spinner or nothing) |
| **User Input Needed** | `input` — `MessageCircleQuestionIcon` + `"Input"` in `text-indigo-600 dark:text-indigo-300`; never recedes | **None** |
| **Guest Request** | `request` — `UserRoundIcon` + `"Request"` in `text-guest-foreground`; never recedes | **None** |
| **Setting Up** | `preparing` — spinner + `"Setting up"` + ticking timer in `text-info-foreground`; recedes to `opacity-70` | **None** |
| **Working / Goal** | `working` — spinner + `"Working"` (or `"Goal"` when goal active) + live ticking `1m 12s` timer in `text-info-foreground`; recedes to `opacity-70` | `EL_S_BUSY` — `15pt` monochrome 3×3 dot spinner (no label, no elapsed timer, no Goal distinction) |
| **Waiting (Child/Idle)** | `waiting` — `ClockIcon` + `"Waiting"` + elapsed timer in `text-muted-foreground`; recedes | **None** |
| **Usage Limited** | `limited` — `CircleAlertIcon` + `"Limited"` in `text-warning-foreground` | **None** |
| **Failed (Unseen)** | `failed` — `CircleAlertIcon` + `"Failed"` in `text-error-foreground` (`#ff6467` dark); settles to `ready` once visited | **None** |
| **Woke from Snooze** | `woke` — clickable `AlarmClockIcon` + `"Woke"` button in `text-warning-foreground` (click dismisses Woke badge) | **None** (silently reappears in list when `now_ms >= snoozed_until_ms`) |
| **Done (Unread)** | `done` — `CircleCheckIcon` + `"Done"` in `text-success-foreground` | `unread` — `6pt` solid accent dot (`#ededed` in Coder Noir) |
| **Visual Receding** | `shouldRecedeSidebarThread()`: quiet/working/already-read rows dim (`text-sidebar-muted-foreground/75`, working `opacity-70`) so **Done / Woke / Input / Approval / Failed** rows pop visually | All non-current rows render at `95%` full ink (`opacity(s.ink, 0.95)`) — unread and read rows have nearly identical contrast |
| **Closed Folder Dot** | 4-tier priority dot on collapsed project folder: `warning` (attention) > `error` (failed) > `success` (unread) > `info` (working) | **None** (collapsed project folder shows only total chat count, hiding busy/unread/needs-you status) |

---

## 7. Drag-and-Drop, Multi-Select, ShiftGuard & List Motion

| Interaction | OpenAgents Desktop | T3 Code Desktop |
| --- | --- | --- |
| **Pinned Reorder** | Custom pointer drag in `input.coax` (`pin_drag`, `4pt` threshold, `2pt` drop indicator line in `k.ink`) — **macOS only** (missing on Linux/Windows) | `@dnd-kit/sortable` with `createSidebarSortingStrategy` & `createSidebarCollisionDetection` across all platforms |
| **Cross-Shelf Drag** | **None** (cannot drag a chat into Pinned to pin it, or out of Pinned to unpin, or into Settled) | Drag any thread between **Pinned ↔ Active ↔ Settled**; live projected layout shifts peer rows, displays dashed drop placeholders (`SidebarSectionPlaceholder`) and boundary labels (`SidebarDragBoundary`: `"Pinned"` / `"Active"`), and displays a live drop-verb badge on the dragged card |
| **Drag Thread to Composer** | **None** | Drag any sidebar thread (including Working threads) directly into the composer to insert a `@thread` context chip |
| **Drop Files onto Thread Row** | **None** (dropping files attaches only to the currently open chat) | `makeWorkspaceFileDropHandlers`: drop OS files directly onto **any** sidebar thread row (highlights with `ring-1 ring-inset ring-primary/70`) to attach them to that thread's composer |
| **Drop Folders onto Sidebar** | Supported (`coder_page_sb_drop_folder` → adds project group + toast) | Supported (`SidebarFolderDrop`: full overlay `"Drop folders to add projects"` with `ring-1 ring-primary/70`, rejects non-folder drops with helpful toast) |
| **Multi-Select** | `Cmd`-click toggle + `Shift`-click contiguous range along `vis_order`; shows `52pt` top bulk bar (`Clear`, `Pin`, `Archive`/`Restore`, `Delete`) | `Cmd`-click toggle + `Shift`-click range (`threadSelectionStore`); right-click opens bulk context menu; also supports **pointer action-sweep** (`onActionSweepStart`) to sweep-settle/unsettle multiple rows in one drag |
| **ShiftGuard (Anti-Misclick)** | **None** | `Sidebar.shiftGuard.ts`: tracks every row's `offsetTop`; if an involuntary update (thread waking, arriving, settling) moves a row by `≥ 50%` of its height outside a `500ms` user-input window, swallows pointer presses on that row for `500ms` so the user never clicks the wrong thread |
| **Row Animation (FLIP)** | **None** (rows snap instantly on insert/remove/reorder) | `Sidebar.motion.ts`: 150ms `ease-out` Web Animations FLIP for moved rows, slide+fade entry, and cloned `inert` fade-out for exiting rows (capped at 40 clones, respects `prefers-reduced-motion`) |

---

## 8. Context Menus Comparison

### Single Thread / Chat Row Context Menu

| Action | OpenAgents Desktop (`Window+Menus.swift`) | T3 Code Desktop (`threadActionMenu.logic.ts`) |
| --- | --- | --- |
| **Find in thread** | — | `Find in thread` |
| **New thread on branch** | — | `New thread on {branch}` |
| **Pin / Unpin** | `Pin Chat` / `Unpin Chat` | `Pin thread` / `Unpin thread` |
| **Settle / Un-settle** | — | `Settle thread` / `Un-settle thread` |
| **Auto-settle behavior** | — | Submenu: `Follow settings` / `Never` |
| **Archive / Unarchive** | `Archive Chat` / `Unarchive Chat` | *(T3 uses Settle + Settings → Archived / Delete)* |
| **Snooze / Wake** | Flat items: `Snooze 1 Hour`, `Snooze 4 Hours`, `Snooze Until Tomorrow`, `Snooze Custom…` / `Unsnooze Chat` | Nested submenu `Snooze ▸` with **configurable environment presets** + wake-time detail labels + `Custom…` / `Wake thread` |
| **Rename** | `Rename Chat…` (modal dialog) | `Rename thread` (inline `<input>` edit, also `F2` / double-click) |
| **Regenerate title (AI)** | — | `Regenerate title` (with live shimmer state) |
| **Fork / Side Pane** | `Fork Chat`, `Open in Side Pane` (flag-gated) | *(Fork is in thread header/composer; Side Pane via linked/delegated pane)* |
| **Mark unread** | — | `Mark unread` |
| **Share with guest** | — | `Share with guest` (HMAC guest link) |
| **Filter by project** | — | `Filter by {project}` / `Show all projects` |
| **Turn into project** | — | `Turn into project` (for scratch threads) |
| **Copy submenu** | — | `Copy ▸` `Path`, `Branch`, `Thread`, `Thread ID` |
| **Delete** | `Delete Chat…` (destructive) | `Delete` (destructive, optional confirm dialog) |

### Other Context Menus
- **Project Folder Header:** OpenAgents has **no context menu** on project folder rows. T3 Code provides: `New thread`, `Copy path`, `Pin project` / `Unpin project`, `Project settings` (favicon, icon/color, setup scripts, default model, archive/delete project).
- **Multi-Select Bulk Menu:** OpenAgents uses a `52pt` top bar (`Pin`, `Archive`/`Restore`, `Delete`, `Clear`). T3 Code provides a bulk context menu (`Unpin`, `Settle (N)`, `Snooze (N) ▸`, `Regenerate title (N)`, `Mark unread (N)`, `Delete (N)`).
- **Unsent Draft Row:** OpenAgents has no draft rows. T3 Code provides: `Project settings`, `Copy path`, `Copy branch`, `Discard draft`.
- **Foreman Row (OpenAgents-unique):** OpenAgents has a rich 10-item Foreman context menu (`Open Foreman`, Tier 0/1/2 selector, `Let Programs Raise the Tier`, `View Audit Log…`, `Stop All Running Chats`, `Clear Foreman History…`).

---

## 9. Footer Chrome, Undo Notice & Update Pills

| Element | OpenAgents Desktop (`sidebar.coax`, `sidebar_groups.coax`) | T3 Code Desktop (`SidebarChrome.tsx`, `SidebarThreadUndoNotice.tsx`) |
| --- | --- | --- |
| **Footer Layout** | Stacked: optional `Update` pill (`28pt` chip + `20pt` vertical padding + `1pt` divider), then full-width `30pt` `Settings` row (`ICON_SETTINGS` + `"Settings"`) | Stacked: `SidebarProviderUpdatePill` + `SidebarUpdateArchitectureWarning`, then horizontal `SidebarUtilityMenu` icon row (`Settings`, `Pull requests`, `Issues`, `Usage` — or `Back` when on a utility page) + inline `SidebarThreadUndoNotice` + `SidebarUpdatePill` |
| **Update Pill** | Single desktop app updater pill (`Available`, `Updating…`, `Restart to update`, `Update failed`) with accent border when highlighted | Desktop app update pill (`SidebarUpdatePill` with release notes popover & `"Update Everything"` dialog) **+** separate **Provider CLI update pill** (`SidebarProviderUpdatePill`) **+** macOS Rosetta/architecture mismatch warning |
| **Undo Notice** | Floating `340pt` bottom-right overlay toast (`cp_sb_undo_paint`, `6s` auto-dismiss, `Undo` + `×` buttons); covers Archive and Delete; **no `Cmd+Z` shortcut capture** | **Inline inside sidebar footer** (`SidebarThreadUndoNotice`): never moves or covers the list; `@container` responsive layout collapses text to icon on narrow sidebars; **captures `Mod+Z` globally** (`registerThreadUndoCommands`) before composer undo while notice is active; covers Settled, Snoozed, Unpinned, Archived, Discarded |

---

## 10. Visual Design, Typography, Spacing, Radii & Elevation

| Design Token | OpenAgents Desktop (`page/skin.coax`, `components/skin/zeron.coax`, `noir.coax`) | T3 Code Desktop (`index.css`, `SidebarThreadRow.tsx`, `ui/sidebar.tsx`) |
| --- | --- | --- |
| **Font Stack (UI)** | `Geist` (`Regular`, `Medium`, `SemiBold`) — custom GPU glyph atlas | `-apple-system, BlinkMacSystemFont, "Inter Variable", "Segoe UI", system-ui, sans-serif` (runtime-overridable in Settings → Appearance) |
| **Font Stack (Mono)** | `Geist Mono` | `ui-monospace, "SF Mono", "SFMono-Regular", Menlo, Consolas, monospace` |
| **Row Title Typography** | `13pt` Medium, `18pt` line-height (`FONT_MENU`) | `14px` (`text-sm`, `0.875rem`) Medium (`font-medium`) |
| **Section Heading Typography** | `12pt` Regular, `16pt` line-height (`k.small`), `s.muted` | `12px` (`text-xs`) Medium (`font-medium`) + `tabular-nums` count |
| **Metadata / Secondary Text** | `12pt` (`k.small`) — rarely shown in sidebar | `12px` (`text-xs`) & `11px` (`text-2xs`) with `tabular-nums` for times, timers, counts, diffs |
| **Row Heights** | `32pt` (`side_row`), `26pt` (`side_head`), `30pt` (`settings` row) | `78px` (`4.875rem` card), `60px` (compact card), `36px` (`h-9` slim/folder), `32px` (`h-8` folder header & shelf header & search input) |
| **Horizontal Insets** | Row pill inset: `8pt` (`row_inset`); text pad: `16pt` (`side_pad`); child indent: `23pt` (`icon + row_inset`) | Container inset: `8px` (`--sidebar-content-inset: 0.5rem`); row content inset: `10px` (`--sidebar-row-content-inset: 0.625rem`); folder child indent: `36px` (`ps-9`) |
| **Corner Radii** | Row pill: `8pt` (`row_radius`); heading action btn: `6pt`; bulk bar: `8pt`; update pill: full capsule (`14pt`); collapsed titlebar island: `12pt` | `--control-radius: 0.5rem` (`8px`) for folder headers; `rounded-md` (`6px`) for thread rows & search input; `rounded-lg` (`8px`) for drop overlay |
| **Icon Sizes** | Action/folder icons: `15pt`; trailing chevron: `10pt`; heading action icon: `14pt`; unread dot: `6pt` | Search/favicon: `16px` (`size-4`); header buttons: `28×28px` (`size-7`); status icons: `16px` (card) / `12px` (folder); hover action icons: `12–14px`; closed-folder dot: `6px` (`size-1.5`) |
| **Tree Guide Line** | `1pt` vertical rule at `x = side_pad + icon/2` in `opacity(s.ink, 0.08)` | None (clean `36px` indentation in folder mode) |
| **Scrollbar** | Thin overlay rail (`app_scroll_rail`), auto-fades when idle, rubber-band overscroll (`side_scr.stretch`) | `6px` (`--app-scrollbar-width`), WCAG `>3:1` thumb (`rgb(255 255 255 / 36%)` dark, `rgb(217 217 217)` light; hover `45%` / `191`) |
| **Hover / Press / Active** | Active: solid `s.raised`; Hover: `s.raised @ 75%` eased; Press: `s.hover` overlay (`text @ 8%`) | Dedicated per-state tokens: `bg-sidebar-row-hover`, `bg-sidebar-row-active`, `bg-sidebar-row-selected`; light mode adds `ring-1 ring-inset ring-sidebar-border/70` on active/selected rows |
| **Focus Ring** | No visible keyboard focus ring on sidebar rows | `focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring` on every interactive element |
| **Jump Keycap Badge** | `18pt` pill (`5pt` radius, `k.surface` fill, `k.field_edge` border), appears after **`1000ms`** `Cmd` hold | `<Kbd>` pill, appears after **`200ms`** (`THREAD_JUMP_HINT_SHOW_DELAY_MS`) `Cmd`/`Ctrl` hold |

---

## 11. Complete Color System & Theme Palette Comparison

### 11.1 Token Architecture Gap
- **OpenAgents Desktop (`components/skin/zeron.coax`, `noir.coax`, `page/customize.coax`):**
  - Has **only 1 dedicated sidebar color token** (`Theme.sidebar`), and reuses global `raised`, `hover`, `ink`, `muted`, `faint`, `menu_divider`, and `accent` for all sidebar states.
  - Because `active` (`s.raised`) and `hover` (`s.raised @ 75%`) share the same hue/base, and `selected` (multi-select) also sets `EL_S_CURRENT` (`s.raised`), **active, hovered, and multi-selected rows have weak visual differentiation**.
  - Default theme (`Coder Noir`) is strictly monochrome (`accent = #ededed`), so unread dots, focus, and highlights have no chromatic distinction from plain white text.
- **T3 Code Desktop (`packages/shared/src/themePalettes.ts`, `apps/web/src/index.css`):**
  - Defines **61 semantic theme color roles**, including **8 dedicated sidebar tokens** that vary independently across every theme and appearance:
    `sidebar`, `sidebarForeground`, `sidebarMutedForeground`, `sidebarControlSurface`, `sidebarRowHover`, `sidebarRowActive`, `sidebarRowSelected`, `sidebarBorder`.
  - Derives `--sidebar-icon-color: color-mix(in srgb, var(--contrast-sidebar-muted-foreground) 85%, var(--sidebar))` so icons sit subtly quieter than labels.
  - Applies a runtime **Contrast Boost** pipeline (`--contrast-sidebar-foreground`, `--contrast-sidebar-muted-foreground`, `--contrast-sidebar-border`) using CSS `color-mix()` toward white (dark) or black (light).
  - Supports **5 chromatic semantic status families** (`info`, `success`, `warning`, `error`, `guest`, `update`) plus `diff-addition-foreground` and `diff-deletion-foreground`, and **20 project icon colors** (`gray` through `rose`).

### 11.2 Default Dark Theme Side-by-Side (Exact Hex Values)

| Semantic Role | OpenAgents `Coder Noir` (`noir.coax`) | T3 Code `Dark` (`T3_CODE_DARK_THEME_COLORS`) | Visual Difference |
| --- | --- | --- | --- |
| **Main Canvas** | `#0a0a0a` | `#0a0a0a` | Identical |
| **Sidebar Surface** | `#050505` | `#000000` (true black) | T3 is pure `#000000` (higher contrast against `#0a0a0a` canvas) |
| **Sidebar Foreground (Primary Text)** | `#ededed` (`95%` on idle rows) | `#f1f3f7` (cool off-white) | T3 is slightly brighter & cooler; dims idle/working rows to `75%` muted |
| **Sidebar Muted Foreground** | `#818181` (faint: `#656565`) | `#a3a3a3` (secondary label: `#818181`) | T3's muted text is noticeably brighter/more legible on `#000000` |
| **Sidebar Control Surface** | `#191919` (`raised`) | `#0a0a0a` | T3 controls sit subtly just above `#000000` |
| **Sidebar Row Hover** | `#191919` @ `75%` (composite `~#141414`) | `#131313` | Nearly identical luminance |
| **Sidebar Row Active (Current)** | `#191919` | `#1a1b1b` | Nearly identical |
| **Sidebar Row Selected (Multi-select)** | `#191919` (same as active + `"✓"`) | `#111111` (distinct from active `#1a1b1b`) | T3 visually separates multi-selected rows from the open thread |
| **Sidebar Right Border** | `rgba(255,255,255,0.07)` (`~#121212`) | `#141414` | Nearly identical |
| **Primary Accent / Focus Ring** | `#ededed` (monochrome) | `#346bf1` (cobalt blue) | T3 uses chromatic blue for focus rings, drop targets, and primary actions |
| **Info / Working** | `#7fb2e8` (unused in sidebar) | `#346bf1` / `info-foreground` (`#51a2ff`) | T3 colors Working status & timers blue |
| **Success / Done / Diff +** | `#9fd08a` (unused in sidebar) | `success-foreground` / `diff-addition-foreground` | T3 colors Done badge & `+N` green |
| **Warning / Approval / Woke** | `#e6c15c` (unused in sidebar) | `#fe9a00` / `#ffb900` (`warning-foreground`) | T3 colors Approval, Limited, Woke, and branch mismatch amber |
| **Error / Failed / Diff −** | `#ff4d42` (only on bulk `Delete` text) | `#fb414a` / `#ff6467` (`error-foreground`) | T3 colors Failed badge & `−M` red |
| **Input Needed** | `#74cfd1` (unused in sidebar) | `indigo-300` (dark) / `indigo-600` (light) | T3 gives Input Needed its own distinct violet/indigo cue |

### 11.3 Default Light Theme Side-by-Side (Exact Hex Values)

| Semantic Role | OpenAgents `Daylight` (`customize.coax`) | T3 Code `Light` (`T3_CODE_LIGHT_THEME_COLORS`) |
| --- | --- | --- |
| **Main Canvas** | `#fafaf9` | `#fcfcfc` |
| **Sidebar Surface** | `#f2f2f0` | `#fafafa` |
| **Sidebar Foreground** | `#18181b` | `#27272a` |
| **Sidebar Muted Foreground** | `#52525b` (faint: `#71717a`) | `#71717b` |
| **Sidebar Row Hover** | `#ffffff @ 75%` | `#fcfcfc` |
| **Sidebar Row Active** | `#ffffff` (no border) | `#ffffff` + `ring-1 ring-inset ring-[#e4e4e7]/70` |
| **Sidebar Row Selected** | `#ffffff` | `#ffffff` + `ring-1 ring-inset ring-[#e4e4e7]/70` |
| **Sidebar Border** | `#d4d4d8` | `#e4e4e7` |
| **Accent / Focus** | `#18181b` (monochrome) | `#1b4ed8` (blue) |

### 11.4 Built-In Theme Families & Customization Depth

| Theme Feature | OpenAgents Desktop (`page/customize.coax`) | T3 Code Desktop (`themePalettes.ts`, `themePalette.ts`) |
| --- | --- | --- |
| **Built-In Dark/Light Themes** | **5 skins** (each single-appearance): `Coder Noir` (dark), `Daylight` (light), `Grove` (dark `#070c09` / `#4ade80`), `Ocean` (dark `#070c14` / `#38bdf8`), `Ember` (dark `#0c0806` / `#f59e0b`). No light variants of Grove/Ocean/Ember; no `T3 Chat` or `Iris`. | **T3 Code Light & Dark** + **5 dual-appearance (Light + Dark) OKLCH palettes**: `T3 Chat` (rose/magenta), `Grove` (forest), `Ocean` (blue), `Ember` (amber), `Iris` (violet) — **12 total built-in appearances**. |
| **System Appearance Follow** | `System` / `Dark` / `Light` (System switches between selected dark skin and `Daylight`) | Independent **Light half + Dark half** theme pairing (`t3code:theme-halves:v1`) when following system |
| **Contrast Boost** | Boolean `high_contrast` toggle (shifts text/muted/borders) | Granular continuous contrast boost (`--appearance-contrast-boost`) applied across every surface & border |
| **VS Code / OpenVSX Import** | Imports **6 keys** from local `.json` (`editor.background`, `editor.foreground`, `sideBar.background`, `panel.border`, `focusBorder`/`button.background`, `statusBar.background`) | Full VS Code + live **OpenVSX marketplace search & import** (`openVsxThemes.ts`, `vscodeThemeImport.ts`) mapping all 61 roles |
| **Custom CSS & Agent Restyling** | None | Up to **100 KB custom CSS** (`themeCss.ts`), custom background images (`themeBackground.ts`), live theme editor/inspector, and **MCP `t3_theme_set` tool** allowing agents to restyle the app |

---

## 12. Accessibility, Keyboard Navigation & Cross-Platform Parity

| Capability | OpenAgents Desktop | T3 Code Desktop |
| --- | --- | --- |
| **Screen Reader Semantics** | macOS VoiceOver `NSAccessibilityElement` (`EL_ROW`, `EL_HEADING`, `EL_BUTTON`, `EL_S_EXPANDED`, `EL_S_CURRENT`, `EL_S_BUSY`); Linux AT-SPI partial; Windows none | Full ARIA across macOS/Linux/Windows (`role="navigation"`, `aria-label`, `aria-current="page"`, `aria-expanded`, `aria-busy`, `role="combobox"` search, `sr-only` status announcements, `BackgroundThreadAnnouncer`, `announceSidebarVisibility`) |
| **Keyboard Navigation** | `Cmd+B` toggle, `Cmd+N` new chat, `Cmd+K` palette, `Cmd+1..9` jump, `` Cmd+` `` Foreman, `Ctrl+Tab` cycle | Customizable `when`-clause keybindings: `Mod+B` toggle, `F6`/`Shift+F6` workspace area cycling, `F2` inline rename, `Enter`/`Space` activate, `ContextMenu`/`Shift+F10` menu, `Mod+Z` undo, `Mod+1..9` jump (200ms hints), `Settle & Advance` |
| **Cross-Platform Sidebar Parity** | **macOS:** full (except unreachable Archived view & missing Snoozed shelf). **Linux & Windows:** missing pinned-chat drag reorder, missing `coder-host` archived sync, missing native folder picker polish | **Near-100% identical** across macOS, Linux, Windows, and Web (only OS folder-drop path reading requires desktop Electron bridge) |

---

## 13. Prioritized Gap Closure Roadmap for OpenAgents Desktop

Ordered by user impact and architectural fit with OpenAgents' native Coax GPU stack (preserving zero-Chromium 120 Hz performance):

### P0 — Fix Broken/Unreachable Functionality Today
1. **Wire the Archived View Toggle (`sidebar.coax`, `sidebar_groups.coax`, `Window+Menus.swift`):**
   - Expose a clean `Active` / `Archived (N)` filter pill or footer toggle that dispatches the already-implemented `CP_SB_VIEW_ACTIVE_ACT` / `CP_SB_VIEW_ARCHIVED_ACT` actions (and add `View → Show Archived Chats` in `AppDelegate.swift`).
2. **Add a Collapsible Snoozed Shelf (`sidebar.coax`, `sidebar_groups.coax`):**
   - Render a collapsible `Snoozed (N)` shelf at the bottom of the Active list when `cp_sb_snoozed_count(s) > 0`, showing each snoozed chat's wake time (`"2h"`, `"Tomorrow 9am"`) and an inline Wake button / right-click `Unsnooze Chat`.
   - When a snoozed chat wakes (`now_ms >= snoozed_until_ms`), show a subtle `"Woke"` status badge until opened.
3. **Add Narrow-Viewport Drawer Fallback on Desktop (`layout.coax`):**
   - Below `CP_SIDE_FROM` (`1080pt`), allow the titlebar toggle (`Cmd+B`) to slide in the existing `app_sidebar` overlay drawer with scrim instead of making the sidebar completely inaccessible.

### P1 — High-Impact Information Density & Workflow Parity
4. **Upgrade Sidebar Rows to Rich 2-Line / Optional 3-Line Cards (`nav.coax`, `sidebar.coax`):**
   - Utilize `side_row_two` (`48pt`) or a `64–72pt` card option to surface:
     - **Relative timestamp** (reusing existing `app_side_age` in `nav.coax`!)
     - **Semantic status badges** beyond busy/unread: `Approval` (`warning`), `Input` (`info`/`cyan`), `Working + elapsed timer`, `Failed` (`danger`), `Done` (`success`), `Goal` (`accent`)
     - **Secondary metadata line:** git branch + worktree glyph + `+ins −del` diff counts + model/provider mark
   - Add **visual receding** (`faint`/`muted` tint on already-read/quiet rows) so `Done`, `Approval`, `Input`, and `Failed` rows immediately stand out.
   - Add a **closed-folder priority status dot** (`warning` > `danger` > `success` > `info`) on collapsed project rows so hidden chats needing attention are never missed.
5. **Inline Sidebar Search & Project Scope Filter (`sidebar.coax`):**
   - Add a compact inline search input at the top of the sidebar (filtering by title, folder, branch, and full-text message index) plus a quick project-filter pill.
6. **Hover Quick-Actions, Inline Rename & Hover Card (`nav.coax`, `sidebar.coax`, `overlay.coax`):**
   - On row hover, swap the right-side timestamp for quick `Pin`, `Snooze`, and `Archive` icon buttons.
   - Support `F2` / double-click inline rename directly on the row.
   - Add a right-anchored hover card showing full title, project path, branch, model, and status.
7. **Dedicated Sidebar Color Tokens (`zeron.coax`, `noir.coax`, `customize.coax`):**
   - Expand `Theme` from 1 sidebar token (`sidebar`) to **6 dedicated sidebar tokens**: `sidebar`, `sidebar_fg`, `sidebar_muted`, `sidebar_row_hover`, `sidebar_row_active`, `sidebar_row_selected`, `sidebar_border`.
   - Distinguish `sidebar_row_selected` (multi-select) from `sidebar_row_active` (open chat), add `focus_ring` rendering on keyboard-focused sidebar rows, and add light variants for `Grove`, `Ocean`, and `Ember`.

### P2 — Power-User Polish & Ergonomics
8. **Attention Shelves (`Working` / `Settled`) & Cross-Shelf Drag-and-Drop:**
   - Add collapsible `Settled` shelf (with `Settle` action + `Cmd+Z` global undo capture while Undo notice is active) and extend `pin_drag` to support dragging chats between `Pinned` ↔ `Projects`/`Recents` ↔ `Settled` across macOS, Linux, and Windows.
9. **Project Folder Context Menu & Custom Project Icons:**
   - Add right-click menu on project headers (`New Chat in Project`, `Copy Path`, `Remove Project`, `Project Settings`) and per-project color/icon badges.
10. **Unsent Drafts Block, ShiftGuard & Row Motion:**
    - Surface unsent multi-chat drafts at the top of the sidebar, add a `500ms` involuntary-shift click guard (`SidebarShiftGuard` equivalent), and add smooth `150ms` row-reposition animations (`uiframe` keyed Y-spring) plus `200ms` `Cmd+1..9` jump hints and double-click-to-reset on the sidebar resize rail.
