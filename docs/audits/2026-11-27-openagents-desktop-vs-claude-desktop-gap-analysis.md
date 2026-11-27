# OpenAgents Desktop vs. Claude Desktop — UX, Panes & Architecture Gap Analysis

- **Date:** 2026-11-27
- **OpenAgents Desktop snapshot:** `baa7b8ab63` (`apps/openagents-desktop/`, `components/`, `coax/lib/agent/desktop/flags.coax`)
- **Claude Desktop reference:** `~/work/cc/book-desktop/` (*Reverse Engineering Claude Desktop*, targeting `/Applications/Claude.app` `6.28899.6` / live Retina atlas `7.37095.0`, Electron `45.3.3`, Chapters `00–13` + Appendix A `atlas-01`–`atlas-17`)
- **Objective:** Identify every meaningful UX, pane, navigation, interaction, and macOS-native-feel gap between OpenAgents Desktop and Claude Desktop, and lay out a concrete Coax Native implementation plan to bring OpenAgents' **ergonomics, spatial calm, and pane feel** up to (and past) Claude Desktop **while strictly preserving OpenAgents' pure-black Geist visual identity**.

---

## 1. Executive Summary

OpenAgents Desktop and Claude Desktop have inverted strengths:

- **OpenAgents Desktop** has a far stronger **runtime and rendering foundation**: native Coax + Metal 120 Hz ProMotion (zero Chromium, zero Node.js, tiny memory footprint), a resident local daemon (`coder-host`) whose runs survive app quits, native local Metal 4 inference (`Inky`/`Psionic`), Steer-vs-Queue mid-stream controls, `/goal` autonomous loops, per-chat USD budgets, an embedded iOS Simulator pane (`oasim`), a 3D Metaverse/GameBuilder/Gym viewport, and zero-config iPhone pairing.
- **Claude Desktop** wins decisively on **everyday UX calm, spatial hierarchy, pane polish, discoverability, and macOS system presence**:
  1. **Calm, uncluttered chrome:** No heavy bordered titlebar cutting across an empty chat; subtle `<` `>` navigation history; a clean `[Chat | </> Code]` workspace mode switch that re-shapes both sidebar and canvas for the task at hand; a `64pt` collapsed icon rail instead of vanishing to `0px`.
  2. **Cohesive pane & surface model:** Six disciplined layout templates (Two-Column, Three-Column Split, Catalog Directory, Settings Master-Detail Modal, Floating Overlay, Modal Dialog), `8pt` invisible drag targets over `1px` hairlines, a clear top-right pane toggle, visual **Recent Artifacts** cards, rich multi-format File Viewers (`.docx`/`.pptx`/`.pdf`/`.xlsx`), and real ANSI/VT100 terminal rendering.
  3. **Zero-friction macOS presence:** A global floating **Quick Access** panel (`Tap Option twice` anywhere on macOS) with display work-area clamping, a dedicated Dictation shortcut, a rich menu-bar tray with live usage/status, and a **floating modal Settings sheet with instant search** that preserves spatial context instead of blanking out the workspace.
  4. **Clear information architecture without flag fragmentation:** Projects, Artifacts, Scheduled Tasks, and Plugins/Connectors/Skills are first-class, calm directory screens directly reachable from the sidebar — whereas OpenAgents currently hides 30 of its 31 desktop surfaces behind `OA_EXPERIMENTAL=1` feature flags (`coax/lib/agent/desktop/flags.coax`) or buries them inside full-window Settings tabs and `⌘K`.

### Scorecard Across 15 Desktop UX & Systems Domains

| # | Domain | OpenAgents Desktop | Claude Desktop | Gap Direction |
| --- | --- | --- | --- | --- |
| 1 | Rendering, Speed & Footprint | Native Coax + Metal 120 Hz ProMotion, zero Chromium, instant cold start | 972 MB Electron 45.3.3 bundle, relies on opacity-0 boot gate + 9 workers to mask Chromium weight | **OpenAgents far ahead** |
| 2 | Window Chrome & Navigation | Full-width 38pt bordered bar on all views; no `<`/`>` history; sidebar hides to `0px` | 44pt quiet chrome (no bar border on chat canvas), `<` `>` history, `[Chat \| Code]` mode switch, `64pt` collapsed icon rail | **Claude Desktop ahead** |
| 3 | Workspace Modes & Sidebar IA | Flat Working/Pinned/Recents/Snoozed/Archived; repo label per row; no Projects/Artifacts/Plugins/Scheduled top-level views | Mode-aware sidebar: Chat mode (catalog links + clean recents) vs Code mode (**repo-grouped thread trees** with blue unread dots) | **Claude Desktop ahead** |
| 4 | Split Panes & Multi-Pane Ergonomics | Single right-pane multiplexer (`cp_side_pane`); no bottom dock; `cp_edge` narrow hit target; right-pane discovery scattered | Clean right-pane toggle in top-right, `8pt` grab target on `1px` hairline, unified Artifact/File/Diff/Terminal/Sim panes; still single right slot (no bottom dock) | **Moderate gap (Claude Desktop cleaner; both need bottom/multi-pane)** |
| 5 | Artifacts & Rich File Viewer | Syntax-highlighted text viewer/editor (16 KB edit / 256 KB read) + 3D scenes; no Artifact gallery or Office/PDF viewers | Sandboxed Artifacts (Code/Preview + version switcher + Install-as-App) + **Recent Artifacts** visual cards + `.docx`/`.pptx`/`.pdf`/`.xlsx` viewer | **Large gap (Claude Desktop ahead)** |
| 6 | Integrated Terminal | `termpane.coax` (flag-gated `DC_FLAG_TERMINAL`): 8 tabs, 8 MiB scrollback, clickable paths, **but strips ANSI escapes** | Full ANSI/VT100 terminal grid (`ptyHostWorker` + `node-pty`) | **Medium–large gap (ANSI rendering + default availability)** |
| 7 | Composer & Input Ergonomics | Strong custom GPU editor (`UiEditor`), `@` mentions, `/` commands, Steer vs Queue, context ring, `/goal`; inline permission mode missing; voice flag-gated | Tiptap rich editor, double-ring zero-shift elevation, `Project or folder` + `Model / Effort / Auto` neatly below pill, inline `[Chat \| Cowork]` toggle, dedicated Voice & Dictation shortcuts | **Mixed (OpenAgents ahead on Steer/Queue & `/goal`; Claude ahead on layout polish & mode controls)** |
| 8 | Conversation Canvas & Message Actions | Virtualized Markdown, grouped tool trees, inline diffs, minimap, Jump-to-latest; **no inline Edit or Retry on message bubbles**; fixed column width | Hover `MessageActionBar` (Copy / **Retry** / **Edit** / feedback), `Transcript width` (Narrow/Wide) setting, `28pt` scroll-edge alpha fades, `10pt` fade-up entrance, `⌘K` Conversation Outline | **Medium gap (Claude Desktop ahead on message actions & scroll polish)** |
| 9 | Settings UX & Discoverability | Full-window takeover (`s[0].set_open`), no settings search, minimal release hides most tabs behind flags | Centered **floating scrim modal** (`⌘,`) preserving workspace context, **live Search box** in left nav, grouped categories, crisp toggle/select rows | **Large gap (Claude Desktop far cleaner)** |
| 10 | Auxiliary Windows & Quick Access | No global Quick Entry (`NSPanel` only for Hold-to-Quit); 2-item `NSStatusItem` menu (`Show`/`Quit`) | Global floating **Quick Access** (`Tap Option twice`, work-area `Skooching`), pre-mounted `<1ms` Find-in-Page overlay, Menu Bar with live usage/status, `sessionHalo` border for autonomous runs | **Large gap (Claude Desktop ahead)** |
| 11 | Code Mode, Worktrees & Diffs | Strong automatic APFS CoW worktrees, `git-guard`, checkpoints, review pane — but Git/Diff/Checkpoints/Terminal all hidden behind `DC_FLAG_GIT` / `DC_FLAG_TERMINAL` | Dedicated `</> Code` mode with repo-grouped sessions, `Choose environment` + `Import CLI sessions` onboarding, unified Diff Workbench + Stage/Commit/Discard | **Medium gap (mostly IA & default-on cohesion; OpenAgents engine is stronger)** |
| 12 | Local Sandbox & Autonomy Safety | macOS shell commands run unsandboxed (`danger-full-access`; `bwrap` Linux-only); strong interactive `permit.coax` prompts + Human Supremacy browser banner | Apple `Virtualization.framework` Linux micro-VM (`smol-bin.img` + selective VirtioFS `hideJson`) + 5 hard Computer Use safety refusal invariants (Secure Input, active typing, occlusion, off-Space, isolated consent) | **Large gap on macOS sandboxing & CU safety** |
| 13 | Plugins, Connectors, MCP & Skills | Implemented behind `DC_FLAG_MCP` (16 servers, OAuth PKCE, 64 skills) inside Settings; no catalog screen or Finder package install | First-class **Plugins** directory (`All \| Connectors \| Skills`, search, 2-col cards), bundled first-party MCP + 7 always-on skills, `.dxt`/`.mcpb`/`.skill` Finder install | **Medium gap (UI & packaging)** |
| 14 | Daemon, Budgets, Mobile & 3D | Resident daemon, `/goal`, USD budgets, Tailscale/Bonjour iPhone sync, `oasim`, GameBuilder/Gym, local Metal 4 Inky | None of these (except iOS Sim helper) | **OpenAgents far ahead** |
| 15 | Feature Cohesion (Release Posture) | 30 of 31 capabilities disabled by default behind `OA_EXPERIMENTAL=1`, creating a jarring split between "bare `--minimal`" and "kitchen-sink experimental" | Cohesive curated default experience with progressive disclosure (everything visible has a home; zero dead/flag-gated sprawl) | **Critical product/UX gap** |

---

## 2. Design Preservation Contract (Keep OpenAgents Looking Like OpenAgents)

We explicitly **do NOT** want to copy Claude Desktop's visual skin. We want Claude Desktop's **spatial discipline, pane ergonomics, navigation calm, and tactile macOS behavior** rendered in **OpenAgents' native pure-black OLED design language**.

| Visual Dimension | Keep (OpenAgents Identity) | Adopt (Claude Desktop Ergonomics) |
| --- | --- | --- |
| **Color & Surfaces** | Pure-black OLED `#000000` canvas & sidebar (`page/skin.coax`), `#111111` composer/cards, `#141414` popovers/bubbles, `#1a1b1b` active rows, 1-physical-pixel `#262626` dividers | Surface contrast hierarchy without heavy horizontal bars; **double-ring focus elevation** (inset + outer hairline glow in `#346bf1` / `#262626`) that never shifts layout by 1px; `28pt` scroll-edge fade masks into `#000000` |
| **Typography** | **Geist** (Regular/Medium/SemiBold) + **Geist Mono**, optical clarity, 8-step zoom (`80%–200%`) | Calmer vertical rhythm (`24pt` prose line height, `20pt`/`24pt` turn spacing, `28pt` uncluttered empty-state greeting), configurable `Transcript width` (`Narrow` 768pt / `Wide`) |
| **Accents & Semantics** | `#346bf1` electric blue primary, `#34d399` emerald done, `#fe9a00` amber approval, `#fb414a` red stop/danger, `#818cf8` indigo input, `#ad98fe` violet plan | Restrained chrome: quiet icons at rest, semantic color reserved for live status dots (unread blue dot, running pulse, approval amber, autonomous `sessionHalo` border) |
| **Motion & Renderer** | Coax + Metal 120 Hz ProMotion (`CADisplayLink`, `spring.coax`, `keyed.coax`), reduced-motion respect | `0.18s` out-quart sidebar collapse to `64pt` rail, `10pt` upward message reveal, instant `<1ms` pre-warmed overlays, zero-flash first paint |

---

## 3. Detailed Gap Analysis (Surface by Surface)

### 3.1 Window Chrome, Titlebar & Navigation
- **Current OpenAgents state (`apps/openagents-desktop/page/layout/titlebar.coax`, `Window+TitleBar.swift`):**
  - Draws a `38pt` (`CP_BAR_PT`) full-width header bar (`cp_header_bar`) with a `1px` bottom hairline across every screen, even an empty chat.
  - Top-left cluster has traffic lights (`x = 21, 44, 67`, `y = 20pt`), sidebar toggle (`x = 88pt`), and Search (`⌘K`).
  - Explicitly notes on line 8 of `titlebar.coax`: *"No back or forward: Coder keeps no history of where it was to go back to."*
  - Collapsing the sidebar slides it completely off-screen to `0px`, leaving no quick icons for New Chat, Search, Dashboard, or Settings.
- **Claude Desktop (`Ch 05`, `atlas-01`, `atlas-03`):**
  - `44pt` top strip (`TRAFFIC_INSET_X = 78pt`), traffic lights at `x: 14, y: 15`, followed by sidebar toggle and **`<` `>` Back/Forward navigation arrows** (`M09`).
  - **No horizontal border bar across the empty Chat canvas** — the main area breathes all the way to the top edge of the window, with a single subtle action icon (e.g., temporary/incognito chat or pane toggle) in the top-right.
  - Top-right of the sidebar header hosts a crisp 2-button mode switch **`[Chat | </> Code]`** that shifts the entire workspace cleanly between conversational work and repository-centric coding.
  - Collapsing the sidebar transitions (`0.18s`, `AM_OUT_QUART`) into a **`64pt` icon-only rail** (`O04 CollapsedIconRail`) so `+ New`, Search, Projects, Artifacts, Scheduled, Plugins, and Account remain one click away even on a 13" laptop.
- **User pain this solves:** Stopping OpenAgents from feeling like a rigid boxed-in developer tool when you just want to think/chat, while making back-and-forth jumping between threads, projects, diffs, and catalogs effortless.

### 3.2 Sidebar & Information Architecture
- **Current OpenAgents state (`page/layout/sidebar.coax`, `page/sidebar_groups.coax`):**
  - Sidebar lists `New chat`, `Dashboard`, (optional `Foreman`), then `Working`, `Pinned`, `Recents` (flat list capped at 50 chats, each showing repo + branch metadata inline), `Snoozed`, `Archived`.
  - Utility icons live jammed at the bottom; Projects, Artifacts, Scheduled Tasks, and MCP/Skills have no dedicated sidebar destinations (Scheduled Tasks and MCP/Skills are buried in Settings tabs behind flags).
  - No top/bottom scroll-edge alpha fade masks — scrolling threads clip harshly against header/footer hairlines.
- **Claude Desktop (`Ch 05`, `atlas-01`–`atlas-06`):**
  - **Chat Mode sidebar:** Clean top nav (`+ New`, `Search`, `Projects`, `Artifacts`, `Scheduled`, `Plugins`), followed by collapsible `Recents v` with single-line titles and hover `...` menus (zero visual metadata noise), pinned account pill at bottom.
  - **Code Mode sidebar (`atlas-03`):** Automatically switches the lower list to **repository-grouped session trees** (`openagents v`, `gamebuilder v`) with indented worktree/sub-session rows and subtle blue unread dots (`#346bf1` fits our skin natively!).
  - **`28pt` scroll-edge alpha fade** (`--cds-scroll-fade-size`) so thread lists dissolve smoothly under the top nav and account footer.
- **User pain this solves:** Flat recents lists become unreadable past ~15 threads when juggling multiple repos plus non-coding chats. Mode-aware grouping keeps Chat calm and Code organized by repo/worktree.

### 3.3 Pane System & Multi-Pane Ergonomics ("Panes & Stuff")
- **Current OpenAgents state (`apps/openagents-desktop/page/layout/layout.coax:29`):**
  - `cp_side_pane` is a priority-ordered if-else chain across **10 different right-side panes** (`Forge`, `Side Chat`, `Browser`, `Preview`, `Terminal`, `Simulator`, `Metaverse`, `File Viewer`, `Git Review`, `Todos + Subagent`).
  - **Pain points:**
    1. **Single-slot contention & invisible switching:** Because all 10 panes fight for the single right-hand slot via a fixed precedence chain, opening Terminal hides File Viewer/Diffs; there is no visible **Right-Pane Tab / Activity Switcher** showing which panes are open or letting you flip between `Files | Diffs | Terminal | Browser | Preview | Sim` with one click (`⌘1`–`⌘6` / tab pills).
    2. **No bottom dock option:** Developers expect Terminal (or build/test logs) to optionally dock **along the bottom** under the conversation/editor so you can see a diff or file *and* run commands simultaneously.
    3. **Split divider grab feel:** `cp_edge` (`page/input.coax:396`) should guarantee an `8pt` invisible hit-slop centered on the `1px` `#262626` divider with a subtle `#346bf1` hover highlight and cursor feedback, plus double-click to reset width.
    4. **No persistent top-right pane toggle:** In `atlas-03`, a clean sidebar/inspector toggle icon sits in the top-right corner of the main canvas (`⌘\`` or `⌥⌘S`), making the right workspace pane immediately discoverable and toggleable without hunting through menus or `⌘K`.
    5. **Pane state persistence per thread:** Claude Desktop keeps pane context natural per mode/session; OpenAgents should remember open right/bottom pane state per chat (e.g., Code thread remembers Diff + Terminal; Metaverse thread remembers 3D viewport).
    6. **ANSI-stripped terminal (`page/termpane.coax:2`):** Stripping ANSI escape codes makes `termpane.coax` feel broken for `git diff`, test runners, compilers, `htop`, and TUI agents compared to a real VT100/xterm cell grid.
    7. **File Viewer & Artifacts gap:** `page/filepane.coax` only renders plain/highlighted text (capped at 16 KB edit / 256 KB read) and `.ple-scene.json`. It lacks:
       - A **unified Artifacts pane** with `Code | Preview` toggle, version selector, Copy/Download, and an **Artifacts catalog** (`Recent artifacts` cards on the New Chat home screen).
       - **Rich document previews** (PDF, Markdown rendered preview, images with zoom/pan, CSV/JSON tables, and eventually `.docx`/`.pptx`/`.xlsx`).

### 3.4 Composer, Empty States & Input Ergonomics
- **Current OpenAgents state (`page/layout/conversation.coax:952`):**
  - Solid `16pt`-radius GPU composer with custom `UiEditor`, `+` attach, `@` mentions, `/` slash commands, effort menu, context ring, folder/branch chip, and best-in-class **Steer (`Return`) vs Queue (`⇧Return`)** mid-stream controls.
  - **Gaps & friction:**
    1. **Empty-state polish:** New Chat centers the composer, which is good, but lacks a calm **greeting header** (`Good morning, Christopher` in Geist Medium `26–28pt`), **Recent Artifacts** / **Resume recent work** visual strip below in Chat mode, and a structured **`Choose environment` / `Choose a folder` / `Import sessions`** onboarding card in Code mode (`atlas-03`).
    2. **Control placement & clutter:** Placing too many tiny utility chips inside the composer footer makes it feel cramped. Claude Desktop separates **inside the pill** (`+`, mode toggle, voice/dictation, send/stop) from **quietly below the pill** (`Project or folder v` on the left; `Model · Effort · Permission/Auto` on the right).
    3. **Permission / Autonomy Mode control missing from composer:** OpenAgents has `permit.coax` (`full_auto` / `approval_mode`), yet users cannot see or switch `Ask | Auto-edit | Full Auto` right at the composer like Claude Desktop's `Auto` selector.
    4. **Focus elevation without layout shift:** Adopt double-ring panel styling (subtle `#262626` resting hairline → soft `#346bf1`/neutral double-ring on focus) so typing focus feels tactile and crisp on pure `#000000`.
    5. **Voice & Dictation ergonomics:** Expose dedicated, configurable **Dictation shortcut** (e.g., `⌘D` or `Fn` double-tap) and **Voice mode** icon directly on the right side of the composer by default instead of gating behind `DC_FLAG_VOICE`.

### 3.5 Conversation Canvas & Message Actions
- **Current OpenAgents state (`page/layout/conversation.coax:384`):**
  - Virtualized turns, collapsible `Thought process` and grouped tool trees (`Read`, `Edited` with inline diffs, `Ran`, `Subagent`), fenced code blocks with copy, `Jump to latest`, and a Transcript Minimap.
  - **Gaps & friction:**
    1. **Missing inline message actions (`M16 MessageActionBar`):** Hovering a user message or assistant reply should smoothly reveal a quiet action row:
       - On **user bubbles:** `Copy`, **`Edit & resend`** (branching or rewinding cleanly from that turn).
       - On **assistant turns:** `Copy`, **`Retry / Regenerate`**, `Fork from here`, `Checkpoint diff / Rewind` (not hidden behind `DC_FLAG_GIT`), and model/timing info.
    2. **Configurable `Transcript width`:** Give users `Narrow (768pt)` vs `Wide (1040pt)` / `Full` in Appearance settings (`atlas-07`), since coding transcripts with diffs and tables benefit from wider columns while reading prose feels best at `768pt`.
    3. **Scroll-edge alpha fades & subtle reveal motion:** Add `28pt` top/bottom alpha fade masks over the `#000000` canvas so scrolled messages dissolve cleanly under the header and above the floating composer instead of hard-clipping.
    4. **Conversation Outline in `⌘K`:** Claude Desktop's `⌘K → Conversation outline` (`atlas-07`) lets users jump directly to any user prompt or heading in a 100-turn thread — trivial to add as a mode/filter in `page/palette.coax` alongside our minimap.

### 3.6 Settings UX & Feature-Flag Fragmentation
- **Current OpenAgents state (`apps/openagents-desktop/settings.coax`, `coax/lib/agent/desktop/flags.coax`):**
  - `⌘,` sets `s[0].set_open = true` and **replaces the entire window** (`layout.coax:57`). You lose sight of your chat and sidebar, and there is **no Settings search bar**.
  - Meanwhile, `flags.coax` gates **30 of 31 desktop features** to `false` by default — including `git`, `terminal`, `mcp`, `browser`, `preview`, `voice`, `scheduled`, `usage`, `customize`, `environments`, `simulator`, and `forge`. That means out-of-the-box OpenAgents feels artificially stripped down, while flipping `OA_EXPERIMENTAL=1` dumps everything on screen without cohesive IA.
- **Claude Desktop (`atlas-07`–`atlas-17`):**
  - `⌘,` opens a **floating centered modal card** (`~860 × 640pt`, `16pt` radius, `1px` `#262626` border, `rgba(0,0,0,0.65)` scrim) over the live workspace.
  - Left column (`~210pt`) starts with a **`Q Search` input** that filters all settings live across every category, followed by cleanly grouped sections with `1px` separators:
    - *Account / Billing / Usage*
    - *Appearance / Capabilities / Notifications / Privacy*
    - *Plugins & MCP / Code*
    - *Desktop App* (Release channel, Run on startup, Quick Access shortcut, Voice/Dictation shortcuts, Keyboard shortcuts, Menu bar, Keep computer awake)
  - Every setting row has a crisp title, muted 1-line explanation, and right-aligned control (segmented control, dropdown, or toggle switch).

### 3.7 Auxiliary Windows & macOS Native Presence
- **Current OpenAgents state (`AppDelegate.swift`, `Activation.swift`, `Notifications.swift`):**
  - Single main window (plus `openNewWindow`), Hold-to-Quit HUD (`NSPanel`), `UNUserNotificationCenter` + `NSSound` + Dock badge, and a bare 2-item `NSStatusItem` (`Show OpenAgents`, `Quit OpenAgents`).
  - **Missing macOS-native surfaces that define Claude Desktop's daily feel:**
    1. **Global Quick Access (`NSPanel` + Global Hotkey):** Press `Tap Option twice` (or custom shortcut) from *any* macOS app to summon a sleek `#111111` floating mini-composer (`620pt` wide, `20pt` radius, crisp `#262626` border + macOS shadow, `nonactivatingPanel` + `canJoinAllSpaces`), attach active window screenshot or clipboard context with one click, get a fast answer or hand off into the main window. Clamp automatically to `NSScreen.visibleFrame` as it grows (`Skooching`) and remember relative screen position.
    2. **Rich Menu Bar Popover (`NSPopover` on `NSStatusItem`):** Show currently running agents/chats with live elapsed timers & approval badges, quick "New chat" / "Quick Access" actions, local model (`Inky`) / daemon (`coder-host`) status dot, and today's usage/spend meter — clicking any running session jumps straight to it.
    3. **Autonomous Run Perimeter Halo (`sessionHalo`):** When Foreman, Browser Agent, or Computer Use is actively driving a window or autonomous `/goal` loop, render a subtle `2px` inset `#346bf1` (or `#fe9a00` when awaiting approval) perimeter glow so the user always has unmistakable peripheral awareness without intrusive modal popups.
    4. **Native Window Polish:** Pre-warm `Find-in-Page` (`⌘F`) as a sleek floating `320 × 40pt` top-right card (`#141414` + `#262626` border) rather than shifting layout, and expose `Run on startup`, `Menu bar icon`, `Keep computer awake`, and `Quick Access shortcut` right inside `Settings → Desktop App`.

### 3.8 Code Mode, Local Sandbox & Computer Use Safety
- **Current OpenAgents state:**
  - OpenAgents already has **superior Git worktree engineering** (APFS CoW clones, `.coder/worktree.json`, `git-guard`, shared `COAX_TOOL_CACHE` / `CARGO_TARGET_DIR`, per-turn git ref checkpoints).
  - **Gaps:**
    1. **Onboarding & visibility:** Promote `git`, `terminal`, `mcp`, `usage`, `voice`, and `browser` into a clean, non-experimental **Code Mode** experience instead of hiding them behind `OA_EXPERIMENTAL=1`. Add an empty-state `Choose folder` / `Import Claude Code / Codex sessions` card when starting a Code session.
    2. **Diff Workbench UX:** Pair `page/gitdiff.coax` with a file-tree list (`+N / -M` badges), unified/split toggle, Stage/Unstage/Commit/Discard controls, and **inline diff comments** that attach directly into the composer as structured review feedback.
    3. **macOS Local Command Sandbox:** Today `README.md:95` admits macOS runs commands unsandboxed (`danger-full-access`) because `bwrap` is Linux-only. To match Claude Desktop's Cowork safety posture natively on macOS, implement a **macOS sandbox tier** using either Apple's `Virtualization.framework` Linux micro-VM (selective VirtioFS mount with dotfile/secret hiding) for untrusted/autonomous runs, plus macOS `sandbox-exec`/Seatbelt profiles for lightweight workspace-write isolation.
    4. **Computer Use Safety Invariants:** Adopt Claude Desktop's 5 hard safety refusals (Secure Input active, human actively typing, foreign-window occlusion, off-Space containment, isolated consent boundary) inside our native macOS driver (`docs/desktop/macos-drive-and-screenshots.md`).

### 3.9 Plugins, Connectors, MCP & Skills Directory
- **Current OpenAgents state:** `page/mcpskills.coax` works well technically (HTTP/Stdio MCP, OAuth PKCE, secret redaction, 64 skills), but is buried inside a flag-gated Settings tab.
- **Claude Desktop (`atlas-05`, `Ch 12`):** Elevates **Plugins** to a clean main-canvas catalog screen (`Search` + `All | Connectors | Skills` filter pills + `Add v` button + 2-column verified cards), ships 7 always-on bundled skills (`docx`, `pdf`, `pptx`, `xlsx`, `frontend-design`, etc.), and registers Finder document types so double-clicking a `.skill` or `.mcpb` bundle opens a clean install dialog.

---

## 4. Concrete Implementation Blueprint (Coax Native, Preserving OpenAgents UI)

Organized into 4 incremental phases so each phase ships a noticeable "wow, this feels great" jump without rewriting the renderer or breaking OpenAgents' pure-black Geist aesthetic.

### Phase P0 — Spatial Calm, Shell Chrome & Floating Settings (Highest Impact-to-Effort)
*Goal: Immediately make the app feel calm, intentional, and spacious like Claude Desktop while keeping `#000000` + Geist + `#346bf1`.*

1. **Quiet Titlebar + Navigation History (`page/layout/titlebar.coax`, `Window+TitleBar.swift`):**
   - Stop drawing the heavy full-width bottom border (`cp_bar_border`) over empty Chat canvases; render header chrome cleanly when content scrolls beneath it (with a `28pt` `#000000` alpha scroll-fade mask) or when a title/pane bar is needed.
   - Add **`<` `>` Back/Forward history buttons** next to the sidebar toggle (`cp_nav_history` ring buffer of `{view, chat_id, pane}` states, bound to `⌘[` / `⌘]`).
   - Add a compact **`[Chat | </> Code]` segmented mode toggle** at the top-right of the sidebar header (and in the titlebar cluster when the sidebar is collapsed).
   - Add a single **right-pane toggle icon** at the top-right of the main canvas (`⌥⌘S` / click) so users always see and control the inspector/workspace pane.
2. **Collapsible `64pt` Icon Rail (`page/layout/sidebar.coax`, `layout.coax`):**
   - Change `side_collapsed` on desktop widths (`>= 900pt`) from collapsing to `0px` → collapsing smoothly (`0.18s` `spring.coax`) to a **`64pt` icon rail** (`+ New`, `Search`, `Projects`, `Artifacts`, `Scheduled`, `Plugins`, and bottom `Account`/`Settings`), while keeping `0px` + overlay drawer below `900pt`.
   - Add `28pt` top/bottom scroll-edge alpha fade masks on the sidebar thread list.
3. **Mode-Aware Sidebar Grouping (`page/layout/sidebar.coax`, `page/sidebar_groups.coax`):**
   - **Chat Mode:** Top catalog links (`New`, `Search`, `Projects`, `Artifacts`, `Scheduled`, `Plugins`) + clean single-line `Recents` (hover `...` menu for Pin/Archive/Rename/Delete; remove per-row repo/branch clutter in Chat mode).
   - **Code Mode:** Group sessions by **repository / workspace folder** with collapsible repo headers, indented worktree threads, and `#346bf1` unread dots.
4. **Floating Modal Settings with Live Search (`apps/openagents-desktop/settings.coax`, `layout.coax:57`):**
   - Render Settings as a centered floating `#111111` modal card (`min(880pt, win_w - 80)` × `min(660pt, win_h - 80)`, `16pt` radius, `1px` `#262626` border) over a `rgba(0,0,0,0.65)` scrim instead of replacing `cp_layout` wholesale.
   - Add a live **`Search` input** at the top of the `210pt` Settings left nav, grouped sections, and add **`Transcript width` (`Narrow` / `Wide`)** + **`Desktop app`** settings (Run on startup, Quick Access shortcut, Dictation shortcut, Menu bar, Keep computer awake).

### Phase P1 — Panes That Actually Feel Great ("UX & Panes")
*Goal: Eliminate right-pane contention, add bottom docking, make splitting/switching effortless, and upgrade Terminal + Artifacts.*

1. **Unified Workspace Pane Header & Switcher (`page/layout/layout.coax`, `components/app/pane.coax`):**
   - Replace the invisible if-else priority chain in `cp_side_pane` with an explicit **`CpPaneDock` model**:
     - **Right Dock** with a clean `36pt` top tab strip of active/available panes for the current thread (`Files`, `Diffs`, `Artifacts`, `Browser`, `Preview`, `Sim`, `3D`, `Subagent`) + `Expand` and `Close` buttons.
     - **Optional Bottom Dock** (`180..420pt`, toggleable `⌃\``) specifically for **Integrated Terminal / Run Logs**, so you can view a Diff or File on the right *and* run commands underneath simultaneously.
     - Persist open dock layout per chat/mode.
2. **Tactile Split Dividers (`page/input.coax`, `components/app/pane.coax`):**
   - Enforce an **`8pt` invisible hit area** centered on every `1px` `#262626` vertical/horizontal split hairline; highlight to `2px` `#346bf1` on hover/drag; double-click resets to default width/height; clamp minimum pane width (`340pt`) and main canvas width (`420pt`).
3. **Real VT100/ANSI Terminal Rendering (`page/termpane.coax`, `Window+Terminal.swift`):**
   - Replace the ANSI-stripping buffer in `termpane.coax` with a proper VT100/SGR cell grid (16/256/24-bit color mapped cleanly onto our `#000000` OLED surface, bold/dim/underline, cursor rendering, alternate screen buffer, selection copy, and clickable file:line links).
   - Graduate `DC_FLAG_TERMINAL` and `DC_FLAG_GIT` into **default-on Code Mode** (no `OA_EXPERIMENTAL=1` needed).
4. **Artifacts Surface & Rich File Viewer (`page/filepane.coax`, new `page/artifacts.coax`):**
   - Add a dedicated **Artifact Pane** (`Code | Preview` segmented toggle, version dropdown, Copy/Save/Open, and live rendered Markdown/HTML/SVG/Mermaid/CSV preview).
   - Show a compact **Recent artifacts** 3-card thumbnail strip below the New Chat composer (`atlas-01` pattern, styled in `#111111` cards with `#262626` borders).
   - Add native PDF (`PDFKit` texture/view) and image zoom/pan viewers in `filepane.coax`.

### Phase P2 — Composer Ergonomics, Message Actions & Catalog Screens
*Goal: Make every turn feel fluid and put Projects, Scheduled Tasks, and Plugins one click away.*

1. **De-Cluttered Composer & Mode Controls (`page/layout/conversation.coax`):**
   - Apply **double-ring focus elevation** on the `#111111` composer pill (zero 1px layout jitter).
   - Place `Project / Folder` + `Permission Mode` (`Ask | Auto-edit | Full Auto`) cleanly below the composer on the left, and `Model · Effort` + context ring cleanly on the right — leaving the inside bottom bar crisp (`+`, tools/toggle pills, voice/dictation, morphing Send/Stop/Steer button).
   - Graduate `DC_FLAG_VOICE` to default-on with a configurable Dictation shortcut.
2. **Inline Message Action Bar & Conversation Outline (`page/layout/conversation.coax`, `page/palette.coax`):**
   - Hover-revealed `MessageActionBar` under every turn:
     - User turn: `Copy`, **`Edit & resend`**, `Rewind here`
     - Assistant turn: `Copy`, **`Retry`**, `Fork`, `View checkpoint diff`
   - Add `28pt` scroll-edge alpha fade masks above the composer and below the top bar, plus `⌘K → Outline` for instant jump to any prompt/heading in long threads.
3. **First-Class Catalog Screens (`T3 CatalogDirectoryTemplate`):**
   - Promote **Projects**, **Artifacts**, **Scheduled Tasks**, and **Plugins (`All | Connectors | Skills`)** into clean main-canvas directory screens accessible from the sidebar, using a single shared Coax `app_catalog_grid` component (page title + primary `#346bf1` CTA + search bar + filter pills + responsive 2-column `#111111` cards).
   - Clean up the 31-flag `flags.coax` split so core desktop features (Code Mode, Git/Diffs/Checkpoints, Terminal, MCP/Skills, Voice, Scheduled Tasks, Usage, Browser) are curated and cohesive by default, reserving flags strictly for genuinely unfinished/experimental subsystems (e.g., Gym, Metaverse, Wallet).

### Phase P3 — macOS Native Presence & Local Sandboxing
*Goal: Make OpenAgents Desktop feel like a flagship macOS citizen even when the main window isn't focused.*

1. **Global Quick Access Floating Panel (`apps/openagents-desktop/QuickAccessPanel.swift`):**
   - Borderless `NSPanel` (`.nonactivatingPanel`, `.floating`, `.canJoinAllSpaces`, `#111111` Coax surface + `1px` `#262626` border + native macOS shadow).
   - Triggered globally via **double-tap `Option`** (or customizable shortcut in `Settings → Desktop App` via `CGEvent` tap / `Carbon` hotkey).
   - One-click attach active window screenshot (`ScreenCaptureKit`) or selected text/clipboard; auto-clamps to `NSScreen.visibleFrame` as it grows; "Open in main window" handoff button.
2. **Rich Menu Bar Popover (`AppDelegate.swift` → `TrayPopover.swift`):**
   - Replace the 2-item `NSMenu` with a compact `#111111` `NSPopover` displaying active/running chats with live timers, attention/approval badges, quick "New chat" / "Quick Access", `coder-host` + local `Inky` status dots, and daily spend/usage bar.
3. **Autonomous `sessionHalo` & Computer Use Safety:**
   - Render a subtle `2px` inset perimeter stroke (`#346bf1` running / `#fe9a00` needs approval) when autonomous Foreman/Browser/Computer-Use agents are active.
   - Enforce the 5 hard Computer Use refusal invariants (Secure Input, active human keydown, foreign window occlusion, off-Space check, isolated consent modal).
4. **macOS Local Command Sandbox:**
   - Add a macOS `Virtualization.framework` micro-VM tier (selective VirtioFS mount + secret/dotfile hiding) or lightweight Seatbelt workspace-write profile so macOS users get real sandboxing without losing local performance.

---

## 5. Where OpenAgents Desktop Is Already Ahead (Preserve & Amplify)

Do **not** dilute these advantages while closing the UX gaps above:

1. **Zero-Chromium 120 Hz Metal Renderer:** Instant cold start, crisp subpixel rendering, and a fraction of Claude Desktop's 972 MB disk / multi-hundred-MB RAM footprint.
2. **Resident `coder-host` Daemon:** Agent runs survive app quit and reattach live — Claude Desktop kills local sessions when the app quits.
3. **Mid-Stream Steer (`Return`) vs. Queue (`⇧Return`):** Already better than Claude Desktop's streaming interruption UX.
4. **APFS CoW Worktrees, `/goal` Verification Loops & Per-Chat USD Budgets:** Far stronger engineering primitives for serious autonomous coding.
5. **Native iOS Simulator (`oasim`), Local Metal 4 `Inky` Inference, 3D GameBuilder/Metaverse & Zero-Config iPhone Sync:** Differentiated surfaces Claude Desktop either doesn't have or ships as heavy external helpers.

---

## 6. Recommended First Sprint (1–2 Weeks, Maximum Feel Delta)

If we execute only **five** items first to make OpenAgents Desktop immediately *feel* like Claude Desktop in our pure-black Geist skin:

1. **`[Chat | </> Code]` mode switch + repo-grouped Code sidebar + `64pt` collapsed icon rail** (`sidebar.coax`, `titlebar.coax`, `layout.coax`).
2. **Quiet empty-chat canvas + `<` `>` Back/Forward navigation + top-right right-pane toggle** (`titlebar.coax`, `conversation.coax`).
3. **Floating modal Settings with live Search + `Transcript width` + `Desktop app` tab** (`settings.coax`, `layout.coax`).
4. **Unified Right-Pane tab switcher + Bottom Terminal dock + real ANSI/VT100 terminal + `8pt` split-divider hit targets** (`layout.coax`, `termpane.coax`, `input.coax`).
5. **Global `Tap Option twice` Quick Access `NSPanel` + inline `Edit` / `Retry` on message bubbles** (`QuickAccessPanel.swift`, `conversation.coax`).
