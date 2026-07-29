# Omega single-experience plan — remove the editor around Zero Base

- **Date:** 2026-07-29
- **Subject repository:** `OpenAgentsInc/omega` at
  `/Users/christopherdavid/work/omega`
- **Subject commit:** `f3ae8769a862e61306c8e424cf59712b7d98e464`
- **Document type:** design analysis and phased plan. This document changes no
  runtime authority, allocates no delta number, and grants nothing.
- **Position in the document chain:** this plan follows the
  [zero-base mode design](./2026-07-26-omega-zero-base-mode.md) (2026-07-26)
  and the [zero-base mode audit](./2026-07-29-omega-zero-base-mode-audit.md)
  (2026-07-29). The design proposed the mode. The audit recorded what shipped.
  This plan proposes the next step: Zero Base stops being a mode and becomes
  the application. The
  [Omega alpha roadmap](./2026-07-29-omega-alpha-roadmap.md) sequences this
  plan together with the Episode 263 three-mode alpha program.

## 1. The ask

The owner wants one single experience. Zero Base is the main application. The
legacy Zed editor around it goes away. The buttons that do nothing get fixed
or removed. There is no second surface to fall back to and no mode flag to
explain.

Two facts frame the work.

First, Zero Base already won. `omega` with no arguments opens Zero Base
(`OMEGA-DELTA-0052`). All 134 GitHub issues on the repository are closed. The
last month of commits built the workbench shell, the sidebar, the composer,
the voice controls, and the delta registry that guards them. The editor is
already an opt-in flag (`--full-editor`) that nothing in the product story
needs.

Second, the current architecture is a subtraction, not a foundation. The full
editor is compiled, initialized, and registered on every launch. Zero Base
hides it behind three mechanisms: panels that are never added, a render seal,
and a process-wide action gate that refuses everything outside an admitted
set. That architecture was the right first move — it shipped in days and
deleted nothing it might need back. It is also the direct cause of every
broken button the owner is hitting now, and section 2 shows why.

## 2. Why buttons break today — three mechanisms, not one

The owner's framing is that buttons do not work because the main editor init
never happened. That is half right. The workspace is created, `register_actions`
runs, and every crate's `init` runs in Zero Base too
(`crates/zed/src/main.rs:537-849` is unconditional). The breakage comes from
three distinct mechanisms, and each needs a different repair.

### 2.1 Mechanism A — the gate refuses silently

Every action outside the admitted set is refused at dispatch, and since
omega#119 the refusal is a log line only
(`crates/zed/src/omega_zero_base_ui.rs:75-96`). No toast, no disabled state,
no visual difference. A refused button is pixel-identical to a broken one.

### 2.2 Mechanism B — the panels were never loaded

`initialize_panels` early-returns in Zero Base
(`crates/zed/src/zed.rs:825-888`). It loads the agent panel and nothing else.
`ProjectPanel`, `OutlinePanel`, `TerminalPanel`, `GitPanel`, `DebugPanel`,
`AgentComputerPanel`, and `SarahWorkroomPanel` are never added to any dock, so
every `workspace.panel::<T>()` lookup for them returns `None` forever. This is
the true half of "editor init never happened."

### 2.3 Mechanism C — the centre pane succeeds invisibly

A sealed Zero Base renders no centre pane unless `zero_base_center_visible`
is set (`crates/workspace/src/workspace.rs:9405-9440`). Any code path that
calls `open_path`, `open_abs_path`, or `add_item_to_active_pane` without first
calling `reveal_zero_base_center` opens a buffer into a pane with no pixels.
The open succeeds, steals composer focus, and shows nothing. An invisible
success is indistinguishable from an unimplemented handler.

### 2.4 The confirmed broken controls

The list below was verified against source at `f3ae8769a8` and against the
live logs (`~/Library/Logs/omega-rc/omega-rc.log`,
`~/Library/Logs/omega-dev/omega-dev.log`).

| # | Control | Mechanism | Evidence |
| --- | --- | --- | --- |
| C1 | The native macOS menu bar. Roughly 90 of 104 items are refused: File > New, Open, all dock toggles, all panels, Extensions, About, Restart, Find in Project, Go to File. | A | `app_menus.rs` has zero `omega_zero_base` references. Log: `workspace::NewFile is off in zero base`, `omega::Restart is off in zero base`. |
| C2 | "Sarah" entry in the `+` new-thread menu. | A + B | Dispatches `workroom::OpenPanel`, which is refused, and the panel it would focus was never added. Regression from `4b3b890bd6` (2026-07-29), which moved the entry out of its zero-base guard at owner direction. Log: three refused clicks on 2026-07-28. |
| C3 | "Add More Agents" entry in the `+` menu. | A + C | Dispatches `omega::AcpRegistry` (refused). Its handler also opens into the invisible centre. |
| C4 | Workbench rail surfaces Files, Git, Terminal. | B | The buttons render enabled, then `prepare_*_surface` fails with "the native Files surface is still loading" because `ProjectPanel`, `GitPanel`, `TerminalPanel` exist only in the full-editor branch. The error renders as a small rail warning triangle. Log: `could not prepare the Files work surface`. |
| C5 | Directory links and directory mentions in the transcript. | B | They emit `RevealInProjectPanel`, whose only subscriber is the never-added `ProjectPanel`. Total silent no-op. |
| C6 | Vim keys in the composer when `vim_mode` was on. | A | The `vim` namespace is not admitted. The rc log shows more than 180 refused vim actions from one session. The owner has since set `vim_mode: false`. Repair: admit the vim action set (section 7, vim stays). |
| C7 | Thread-outline artifact "open source". | C | `navigate_to_outline_artifact_source` calls `workspace.open_path` with no reveal. The same bug was fixed one hour earlier for `thread_view.rs` in `259930d92f`, but two more call sites remain. |
| C8 | Skill mention chips. | C | `open_skill_file` and `open_skill_content_buffer` open into the invisible centre. |
| C9 | The shared `open_abs_path_at_point` helper. | C | Only the file-peek caller reveals the centre first. Every other caller lands invisibly. |

Two notes on this table.

- The Omega-side audit (`~/work/omega/docs/omega/zero-base-audit-2026-07-29.md`)
  concludes "no presently identified functional failures." C1 through C9
  contradict that conclusion. Its own follow-up item — review new actions for
  reachability, not just for whether their buttons render — is exactly what C2
  and C3 violate.
- The workbench tests are green because `test_support.rs:531-551` loads and
  adds exactly the three panels that the shipped Zero Base branch skips. The
  proof harness exercises a configuration that production never runs. That
  divergence is itself a defect.

### 2.5 The pattern

`OMEGA-DELTA-0053` states the mode's one-directional law: *if the gate refuses
an action, its control must not be drawn.* The C-list shows the law needs its
converse: **if a control is drawn, its action must be admitted, its handler's
dependencies must exist, and its result must be visible.** Every entry above
violates one of those three clauses. The single-experience architecture in
section 3 makes the converse structural instead of case-by-case.

## 3. The design inversion

Today Omega builds the whole editor, then hides and refuses it. The
single-experience end state inverts that: **build only what is drawn, and
everything drawn works.**

Consequences of the inversion:

1. **The action gate stops being load-bearing.** Today the gate is the safety
   mechanism that makes "not rendered" safe. When the refused surfaces no
   longer exist, the gate has nothing to refuse. It can remain as a tripwire —
   a refusal in the log becomes a bug report, not a designed outcome.
2. **The seal stops being a seal.** `is_sealed()` currently guards four render
   sites against an editor that is one un-zoom away. When the editor is gone,
   the sealed layout is simply the layout.
3. **The admitted set becomes the action inventory.** Instead of a fence
   around a larger registry, `ADMITTED_NAMESPACES` plus `ADMITTED_ACTIONS`
   approximates the complete set of actions the application registers at all.
4. **One init path.** `initialize_panels`, `initialize_workspace`, and
   `app_menus` each currently have two shapes. They collapse to one, and the
   test harness (`test_support.rs`) mirrors it instead of diverging from it.

## 4. What stays — the true Zero Base closure

The measured dependency closure of the Zero Base surface is 190 of the 245
crates in the `zed` build graph. The important entries are not obvious, and
three of them are the reason "remove the editor" does not mean "remove the
`editor` crate."

| Kept surface | Why it stays |
| --- | --- |
| `editor` (161k LOC), `multi_buffer`, `language`, `lsp` | The composer is an ordinary `editor::Editor` (`message_editor.rs:502`). The `editor` namespace is admitted for exactly this reason. |
| `workspace`, `project`, `worktree` | The window root, docks, worktrees, and the file tools' project model. The sealed layout is a `Workspace` render branch. |
| `project_panel`, `git_ui`, `search`, `terminal_view`, `terminal` | The workbench shell hosts the real panels as its Files, Git, Search, and Terminal surfaces (`workbench_shell.rs` imports all four). Reuse of these was an explicit decision in omega#125: no web-style approximations, no second implementations. |
| The centre pane and `workspace::Save` | `OMEGA-DELTA-0139`: a transcript file link opens a real editable pane beside the agent surface. This is the single sanctioned editor exception, and it survives. |
| `title_bar` | The sealed drag strip (`OMEGA-DELTA-0147`) and the `title_bar::init` orphan repair (`OMEGA-DELTA-0157`). |
| `command_palette`, `picker`, `menu` | Admitted namespaces. The palette is the designed discovery surface. |
| `settings_ui` | `omega::OpenSettings` and friends are individually admitted, and settings opens its own window, which is why it is one of the few controls that already works. |
| `onboarding` (identity section) | `OMEGA-DELTA-0040`: the identity gate is a centre-pane item and must render before the seal. `OMEGA-DELTA-0051` already strips the rest of the page. |
| `markdown`, `buffer_search`, `notifications` | Transcript rendering, terminal search, and the toast layer outside the seal. |
| `vim` and `assets/keymaps/vim.json` | Owner decision, 2026-07-29 (section 7). Modal editing in the composer and the `OMEGA-DELTA-0139` centre pane. Requires admitting the vim action set. |

## 5. What goes

### 5.1 The mode split itself

Five branch points in `crates/zed` carry the whole divergence:

- `main.rs:307-314` — the `--full-editor` test around mode entry.
- `main.rs:938-940` — the conditional install of the palette restriction and
  action gate.
- `zed.rs:553-565` — the `MultiWorkspace` sidebar registration (full editor
  only).
- `zed.rs:611-694` — the status-bar build, with its fifteen editor status
  items.
- `zed.rs:825-935` — the two-branch `initialize_panels`.

All five collapse to their Zero Base shape. The `--full-editor`, `--diff`,
`--dev-container`, and `--demo-workroom` arguments go with them.

### 5.2 The crate removal set

The measured full-editor-only set was 54 to 55 crates. With `vim` retained by
the section 7 owner decision, the removal set is roughly 54 crates, 222,000
lines, and 249 `.rs` files — about 18% of the build graph. The set without
vim remains transitively closed (the original measurement dropped the graph
from 245 to 190 crates with zero unexpected extra drops, and vim is a leaf on
that graph). The largest entries:

| Crate | LOC | Crate | LOC |
| --- | ---: | --- | ---: |
| `debugger_ui` (+`debugger_tools`, `dap_adapters`) | 29,596 | `file_finder` | 7,777 |
| `sidebar` | 23,779 | `keymap_editor` | 6,054 |
| `dev_container` | 16,513 | `language_tools` | 5,483 |
| `edit_prediction` family (6 crates) | 42,532 | `diagnostics` | 5,100 |
| `repl` | 12,526 | `markdown_preview` | 3,535 |
| `recent_projects` | 9,236 | `call` (+ `livekit` linkage in `title_bar`) | 3,407 |
| `outline_panel` (+`outline`) | 9,489 | `extensions_ui` | 2,408 |
| `tab_switcher` | 1,503 | | |

Plus the long tail of selectors and previews: `theme_selector`,
`language_selector`, `toolchain_selector`, `encoding_selector`,
`line_ending_selector`, `settings_profile_selector`, `go_to_line`,
`project_symbols`, `image_viewer`, `csv_preview`, `svg_preview`, `journal`,
`feedback`, `tasks_ui`, `snippets_ui`, `acp_tools`, `inspector_ui`,
`miniprofiler_ui`, `component_preview`, `which_key`, `activity_indicator`,
`lsp_locations`, `install_cli`, `auto_update_ui`, and others.

Several removals require edits inside kept crates, in dependency order:

- `recent_projects` is used by `title_bar` (kept) — remove the project menu
  from the title bar first. It is not drawn in the sealed window anyway.
- `markdown_preview` is used by `project_panel` (kept) — three call sites.
- `call` and `livekit_client` are used by `title_bar` — remove the
  screen-share popover.
- `outline` is used by `outline_panel` only, and falls out with it.
- `initialize_pane` (`zed.rs:1600-1667`) builds toolbar items from `search`,
  `diagnostics`, `language_tools`, `debugger_tools`, `acp_tools`, and
  `image_viewer` in both modes today, and must be pruned to the kept set.

Extensions deserve a note: `OMEGA-DELTA-0026` already points the extension
registry at an `.invalid` host, so `extensions_ui` is a surface that reaches
nothing. Removing it is deleting a dead end, not a capability.

### 5.3 What was already broken and is now simply deleted

Section 4 of the 2026-07-26 design listed the surfaces that were hidden
*because* they were broken: extensions, updates UI, edit predictions. In the
single experience they are removed rather than hidden, which resolves the
2026-07-26 document's own warning that its brand-gate classifications would
need a pass when hiding ever became removal.

## 6. Everything the full editor has that Zero Base does not — keep it if

This section is the complete keep-or-cut catalog. It lists every part of the
Zed full editor that Zero Base does not render, grouped by kind, with the
concrete condition under which keeping it makes sense. An entry with no
believable condition is a cut. The default disposition for every entry is
**remove**, per the owner's single-experience direction — a keep is a
deliberate decision against that default, recorded with its reason.

Three entries in the catalog are already dead ends today: the extensions
surface points at an `.invalid` registry (`OMEGA-DELTA-0026`), the update
surface has no feed behind it, and edit predictions default to provider
`"none"`. Keeping any of those means reviving a capability, not preserving
one.

### 6.1 Surfaces and panels

| Part | What it is | Keep it if you want |
| --- | --- | --- |
| Full centre pane system: tabs, splits, pane groups | Multi-file editing with `pane::Split*`, tab bars, drag-to-split. Zero Base has exactly one revealed pane (`OMEGA-DELTA-0139`). | Long hand-editing sessions across several files at once inside Omega. If editing stays incidental (fix a line the agent named), the single revealed pane is enough. |
| Outline panel (`outline_panel`, 8,255 LOC) | Symbol tree for the active buffer, docked. | Navigating large files by structure while hand-editing. The agent answers "where is X" conversationally, which is the Zero Base substitute. |
| Debugger (`debugger_ui` + `debugger_tools` + `dap_adapters`, 29,596 LOC) | Breakpoints, stack frames, variable list, DAP adapters. | Interactive step-through debugging in Omega. If debugging happens through the agent and the workbench terminal, this is the largest clean cut in the set. |
| Diagnostics panel and indicator (`diagnostics`, 5,100 LOC) | Project-wide error list plus the status-bar error count. | A standing project-errors view. The agent surfaces diagnostics per task, and the terminal shows compiler output. |
| Multi-workspace sidebar (`sidebar`, 23,779 LOC) | The project switcher hanging off `MultiWorkspace`. | Several projects open in one window with fast switching. Zero Base's premise is one thread bound to one folder, so this contradicts the product shape. |
| Welcome and onboarding pages beyond identity | New-file, clone-repo, explore-extensions starter surface. | Nothing. `OMEGA-DELTA-0051` already subtracted it, and the identity section stays. |
| Dev container support (`dev_container`, 16,513 LOC, `--dev-container`) | Opening projects inside container-defined environments. | Container-isolated agent workspaces as a product feature. Nothing in the current roadmap asks for it. |
| Diff view (`--diff`) | Two-file diff as a startup mode. | Command-line diff review. The workbench Review surface covers agent-change review, which is the case the product cares about. |

### 6.2 The status bar and its items

The status bar itself does not render in Zero Base (`OMEGA-DELTA-0053`), so
each item is only worth keeping if it gets a new home, most plausibly the
composer bar or the workbench rail.

| Item | Keep it if you want |
| --- | --- |
| Cursor position + `go_to_line` | Line:column readout and go-to-line while editing in the revealed pane. Cheap and genuinely useful the moment real editing happens. |
| Language selector | Manually overriding syntax for a buffer. Rarely needed when files open from transcript links with real paths. |
| LSP button (`language_tools`) | Seeing and restarting language servers. Useful for debugging LSP behind the agent's tools, not just the editor. |
| Activity indicator | Progress for language-server downloads and long background work. Without it, LSP startup is invisible. |
| Edit-prediction button (`edit_prediction` family, 42,532 LOC) | Inline AI completions while hand-editing. Dead today (provider `"none"`), and the agent is the product's completion story. |
| Git blame + merge-conflict indicators (`git_ui`, kept crate) | Blame and conflict state in the revealed pane. The Git workbench surface already exists, so these are candidates to resurface there rather than keep as chrome. |
| Vim mode indicator | The modal-state readout for the kept vim mode (section 7). Needs the new home most urgently of this list. |
| Encoding, line-ending, toolchain selectors, image info | File-encoding edge cases, per-project toolchain switching, image metadata. Niche. Cut unless a real workflow surfaces. |
| Search button (`search`, kept crate) | A mouse path into project search. The workbench Search surface is the Zero Base path. |

### 6.3 Navigation and pickers

| Part | Keep it if you want |
| --- | --- |
| File finder (`file_finder`, 7,777 LOC, cmd-p) | Keyboard-first jump-to-file into the revealed pane. The strongest keep candidate in this group: it matches the keyboard-centric product feel and costs little. |
| Tab switcher (`tab_switcher`, 1,503 LOC) | Cycling among many open tabs. Only meaningful if the full tab system stays (6.1). |
| Buffer outline modal (`outline`, 1,234 LOC, cmd-shift-o) | Jump-to-symbol inside the open buffer. Same condition as hand-editing generally. |
| Project symbols (`project_symbols`, 1,392 LOC) | Workspace-wide symbol jump. The agent's search tools are the substitute. |
| Recent projects (`recent_projects`, 9,236 LOC) | Reopening past projects from a picker. The threads sidebar already reopens past work with its folder attached, which is the Zero Base equivalent. |

### 6.4 Editing extras

| Part | Keep it if you want |
| --- | --- |
| REPL / Jupyter kernels (`repl`, 12,526 LOC) | Notebook-style evaluation inside buffers. No roadmap demand. |
| Tasks (`tasks_ui`, 2,128 LOC) | Zed's task-runner UI. The agent and the thread terminal run commands in this product. |
| Snippets UI (`snippets_ui`) | Snippet management. Hand-editing convenience only. |
| Markdown / SVG / CSV previews, image viewer (~9,000 LOC combined) | Rendered previews when opening those file types from transcript links. Markdown preview is the least implausible keep, since agents produce markdown constantly — but the transcript already renders markdown, which is why the default stays remove. |
| Journal (`journal`) | Daily-notes files. No product connection. |

### 6.5 Configuration and app surfaces

| Part | Keep it if you want |
| --- | --- |
| Keymap editor (`keymap_editor`, 6,054 LOC) | GUI keymap editing. The settings window and JSON keymap files remain the supported path. |
| Theme selector (`theme_selector`) | Quick theme switching from the palette. Settings covers it; Omega ships a deliberate look. |
| Settings profiles (`settings_profile_selector`) | Switching whole settings profiles. Niche. |
| Extensions UI (`extensions_ui`, 2,408 LOC) | A browsable extension registry. Dead today (`.invalid` host). Keep only when an Omega extension ecosystem is a decided product direction — and then it returns as a designed surface, not this one. |
| Update UI (`auto_update_ui`) + Install CLI (`install_cli`) | In-app update checks and the `omega` shell-command installer. Dead feed today, but the 2026-07-27 cloud build audit proposes an owned update path — if that lands, an update surface returns on purpose. The CLI installer is worth a one-command equivalent somewhere. |
| Feedback (`feedback`) | The Zed feedback dialog. Omega's feedback path is not Zed's. |

### 6.6 Developer and diagnostic tooling

| Part | Keep it if you want |
| --- | --- |
| ACP tools (`acp_tools`, 842 LOC) | A live view of Agent Client Protocol traffic. The strongest keep in this group: Omega *is* an ACP product, and its own developers debug that boundary. Candidate to keep behind a dev flag rather than delete. |
| GPUI inspector (`inspector_ui`), miniprofiler, component preview, input-latency UI, `which_key` | UI-framework debugging and development aids. Keep whichever ones Omega's own UI development actually uses, behind dev builds, and delete the rest. |
| LSP logs (`language_tools`, 5,483 LOC) | Language-server log inspection. Same dev-flag logic as ACP tools if LSP stays load-bearing for agent tools. |

### 6.7 Collaboration remnant

| Part | Keep it if you want |
| --- | --- |
| Calls and screen share (`call`, 3,407 LOC, plus `livekit_client` linkage in `title_bar`) | Zed-style calls and screen sharing. Zed collab is already removed (`OMEGA-DELTA-0012`), and Sarah voice runs on its own path in `workroom_ui`, not on `call`. Cut, and unlink `livekit_client` from the title bar with it. |

### 6.8 Summary

The keeps with a live case, in rough priority order: **file finder**,
**cursor position and go-to-line**, **ACP tools behind a dev flag**, the
**LSP/activity visibility pair**, and **markdown preview** as a maybe. The
conditional revivals are **update UI** (owned feed) and **extensions UI**
(ecosystem decision). Everything else in this catalog has an agent-shaped or
workbench-shaped substitute already in the product, which is the practical
meaning of the single experience.

## 7. Vim stays — owner decision, 2026-07-29

The owner decided on 2026-07-29: **vim mode is kept.** `vim` (47,986 LOC,
about 56 source files) moves from the removal set to the kept closure, and
`assets/keymaps/vim.json` stays.

The decision matches the observed demand: the rc log shows a session with
`vim_mode: true` and more than 180 refused vim actions in the composer, after
which the setting was turned off. That was an owner who tried vim and found it
broken by the action gate, not an owner who did not want vim.

Keeping vim creates three concrete work items:

1. **Admit the `vim` namespace** (or the vim action set) so modal editing
   works in the composer and in the `OMEGA-DELTA-0139` centre pane. This is
   the repair for C6 and belongs in Phase 0 with the other admissions.
2. **Keep `vim.json` out of the keymap strip.** The Phase 3 keymap discipline
   applies only to the namespaces whose crates are actually deleted.
3. **Re-home the mode indicator.** `vim::ModeIndicator` is a status-bar item,
   and the status bar does not exist in the single experience. The composer
   bar is the natural host. Until it has a home, vim works without a visible
   mode readout, which is worth a small follow-up rather than a blocker.

## 8. The phased plan

The phases are ordered so the application gets less broken at every step.
Phase 0 repairs what ships today and needs no removal. Removal starts only
after the surface is honest.

### Phase 0 — make the drawn surface work (repair, no removal)

1. **Load the workbench panels in Zero Base.** Extend the Zero Base branch of
   `initialize_panels` to load and add `ProjectPanel`, `GitPanel`, and
   `TerminalPanel`, exactly as `test_support.rs:531-551` already proves works.
   This fixes C4 (Files, Git, Terminal rail surfaces) and C5 (directory
   reveal), and it re-converges the shipped init path with the test harness.
2. **Reveal before every centre-pane open.** Route every `open_path` /
   `open_abs_path` / `add_item_to_active_pane` call reachable from the Zero
   Base surface through one helper that calls `reveal_zero_base_center`
   first — the pattern `omega_file_peek::open_editable_request` already uses.
   Fixes C7, C8, C9, and half of C3.
3. **Make the Sarah entry work.** The owner directed on 2026-07-29 that the
   Sarah panel is always offered (`4b3b890bd6`), so hiding it again is not the
   repair. Add `SarahWorkroomPanel` to a dock in the Zero Base branch (it is
   already loaded, its handle currently dropped), admit `workroom::OpenPanel`,
   and let the existing dock render show it. Fixes C2 honestly.
4. **Fix or remove "Add More Agents".** Either admit `omega::AcpRegistry` and
   reveal the centre for its registry view, or remove the entry until the
   registry is a workbench surface. Fixes C3.
5. **Admit vim** (section 7, decided: vim stays). Admit the vim action set so
   `vim_mode: true` works in the composer instead of being silently refused,
   and verify the modal keys against the C6 log evidence.

Phase 0's exit test is behavioral: a session driven over the drawn surface
produces **zero refusal lines in the log**. The gate's log output flips from
designed outcome to defect signal, which is the section 3 inversion done
early.

### Phase 1 — one menu bar

Replace `app_menus` with a menu built from the admitted set: App (About,
Settings, Quit, Hide), Edit (clipboard and undo into the composer), View
(font size, full screen, sidebar and workbench toggles), Thread (new thread,
open folder, voice), Window, Help. Roughly 90 refused items disappear (C1).
This is presentation only — no capability changes — and it is the single
highest-visibility fix for "buttons that do nothing."

### Phase 2 — remove the mode split

1. Delete `--full-editor` and the editor-only arguments. Decide the failure
   shape for a stale `omega --full-editor` invocation: recommend a one-line
   startup error naming the removal, for one release, then a plain unknown
   argument.
2. Collapse the five branch points (section 5.1) to their Zero Base shape.
   Delete the `sidebar` crate registration and the status-bar block.
3. `omega_zero_base::is_active()` becomes constant `true`. Keep the crate as
   the action-inventory authority (admitted set, refusal sentence), and fold
   `ENTERED` away. The seal simplifies to the identity-gate ordering it
   actually protects: bootstrap centre pane until identity is ready, agent
   surface after.
4. Retire the two `FullEditor` visual baselines and land the **sealed**
   baseline the audit's gap 7.1 already calls for. The sealed render is about
   to become the only render, and it currently has zero automated coverage —
   this is the highest-risk item in the whole plan and lands before, not
   after, the layout collapse.
5. Update `PRODUCT.md`, which still calls Omega "an IDE" and asks to
   "preserve familiar editor controls." It is the stale product-intent
   document, and this phase makes its staleness a contradiction.

### Phase 3 — delete the crate set

Delete the 54-crate set in reverse dependency order, in bounded commits, each
commit carrying its own keymap strip and delta bookkeeping (section 9). The
edits inside kept crates (`title_bar`, `project_panel`, `initialize_pane`,
`notifications`) land first so each crate deletion is a leaf deletion. The
`zed` crate's own `main.rs` init list (some forty `::init` calls in
`main.rs:636-849`) shrinks with each commit.

### Phase 4 — invert the proofs

1. The action gate remains installed as a tripwire. Add a delta check that
   the refusal log is empty across the visual-proof and `--omega-send` smoke
   runs.
2. Extend `OMEGA-DELTA-0129`'s "nothing between the keystroke and the model"
   discipline: nothing between the click and its effect. Each workbench
   control's delta names its handler's dependency (panel, reveal, admission)
   so the C-class cannot silently reappear.
3. Rewrite `taxonomy.md` and the mode documentation: Zero Base is no longer a
   mode with an escape, it is the application.

## 9. The constraint ledger — what breaks if this is done naively

Every constraint below is mechanical and already tripped somebody once.

1. **The keymap startup panic.** The built-in keymap is unwrapped at startup.
   A binding that names a deleted action kills the process while
   `cargo check --workspace` stays green — `0.2.0-rc6` died exactly this way.
   Every crate deletion must strip its bindings from all three platform
   keymaps in the same commit: roughly 100 bindings per keymap across the
   removal set. `assets/keymaps/vim.json` stays, because vim stays. Each
   removed namespace goes into `FORBIDDEN_KEYMAP_NAMESPACES` so it cannot
   return, following the `OMEGA-DELTA-0009` / `0012` precedent: *unreachable
   code that a rebase can revive is not a removal.*
2. **The delta registry is a policy ledger, and this plan reverses recorded
   policy.** `OMEGA-DELTA-0048` says Zero Base "deletes nothing" and its
   check asserts the full-editor panel-load literals still exist in `zed.rs`
   and that nine hidden namespaces (`debugger::`, `git::`, `pane::`,
   `project_panel::`, `workspace::`, and others) are still bound in all three
   keymaps. `OMEGA-DELTA-0052` asserts the `--full-editor` literals in three
   separate places. `0053` and `0116` pin the flag too. These entries are
   amended in the registry's own supersession style — the owner's
   single-experience direction is the recorded reason — and their checks
   change in the same commits. The registry rule holds: delete or amend the
   entry, the check, and the test together, and never weaken a check merely
   to pass.
3. **The brand gate has minimum-inventory floors.** `script/omega-brand-gate.json`
   requires at least 1,500 Rust files. The removal set deletes roughly 249 of
   1,903, leaving about 1,654 — a headroom near 150 files. The floors
   (`minimum_rust_files`, `minimum_rust_string_literals`,
   `actions.minimum_inventory`, and four more) must be re-measured and
   lowered in the same commits as the deletions, with the re-measurement
   recorded, because the gate's own comment forbids loosening it to make a
   build pass. Two classified prose literals (in `edit_prediction_ui` and
   `debugger_ui`) go stale on deletion and leave the classification file with
   them.
4. **The identity gate needs a centre pane once.** `OMEGA-DELTA-0040` renders
   identity onboarding as a centre-pane item before the seal. The collapse of
   the mode split must preserve the bootstrap-then-agent-surface ordering, or
   a fresh profile becomes the dead end that `onboarding::Finish`'s admission
   already repaired once.
5. **`test_support` must mirror shipped init.** C4 existed because the test
   harness loaded panels production skipped. After Phase 0 the two paths are
   the same code path, and a delta check should assert it.
6. **The visual baselines flip meaning.** The two `FullEditor` baselines
   photograph a surface Phase 2 deletes, and the two Zero Base baselines
   photograph an unsealed window (audit gap 7.1). The sealed baseline lands
   first, the ordinary baselines retire with the mode, and the
   `is_active() == (surface == ZeroBase)` per-scene assertion simplifies to a
   constant along with the mode itself.
7. **The packaged gate stays single-binary.** `OMEGA-DELTA-0038` requires the
   packaged gate to open every executable that ships. Nothing in this plan
   adds a binary, and the visual runner remains a test binary, but the gate
   re-runs on the shrunk package.

## 10. Delta bookkeeping this plan implies

Following the 2026-07-26 convention, this document allocates no numbers. The
work needs approximately four new entries and four amendments:

- **New: Zero Base is the application.** There is no mode flag, no editor
  surface, and no runtime that renders the pre-2026-07 editor. The check
  asserts `--full-editor` is absent from `Args` and that `main.rs` has one
  startup shape.
- **New: drawn implies working.** Every control the surface renders dispatches
  an admitted action whose handler dependencies exist in the shipped init
  path. Checked per-surface (rail, `+` menu, header menu, menu bar) rather
  than globally, because a global render-to-admission proof is not
  mechanically available.
- **New: removed editor crates stay removed.** Extends `REMOVED_FILES` and
  `FORBIDDEN_KEYMAP_NAMESPACES` with the section 5.2 set.
- **New: the refusal log is empty in proof runs.** The gate is a tripwire, and
  a refusal recorded during the smoke or visual proofs fails the run.
- **Amend 0048** (deletes nothing → the subtraction became the product, with
  the keymap-panic discipline retained for the kept surface), **amend 0052**
  (the editor flag is gone with the editor), **amend 0053** (the sealed
  render is the render), **amend 0116** (a path argument names the project —
  unchanged in substance, restated without the mode vocabulary).

## 11. Open owner decisions

1. **Vim** — decided, 2026-07-29: vim stays (section 7). The remaining
   choice is where the mode indicator lives once the status bar is gone.
2. **`--full-editor` failure shape** — explanatory startup error for one
   release, or immediate unknown-argument failure. Recommendation: the error,
   because the flag is in the owner's own muscle memory and scripts.
3. **Sarah surface shape** — dock panel (Phase 0 recommendation) now, or hold
   the entry until the workroom becomes a workbench surface. The owner's
  `4b3b890bd6` direction says the entry stays either way.
4. **Menu-bar minimal set** — Phase 1 proposes App/Edit/View/Thread/Window/
   Help. The exact item list is copy the owner should see before it ships.
5. **`PRODUCT.md` rewrite** — the product contract is the owner's document,
   and Phase 2 makes the "IDE" language a contradiction rather than a lag.

## 12. Verification

- Phase exit for 0 and 1: `cargo test -p omega_deltas -p omega_zero_base`,
  plus a scripted `--omega-send` session over the drawn surface with an
  empty refusal log.
- Phase exit for 2: the sealed visual baseline matches, the full-editor
  baselines are gone, and a fresh-profile launch still completes identity
  onboarding.
- Phase exit for 3: `cargo build --workspace` and the delta suite green with
  the brand gate re-measured, and the three keymaps parse with zero
  references to removed namespaces (`keymaps_name_no_deleted_action`).
- Continuous: every commit in Phase 3 is a leaf-crate deletion whose keymap
  strip and delta amendment travel in the same commit, so any single commit
  reverts cleanly.

## 13. Size estimate

Phase 0 and 1 are days: they touch `zed.rs`, `agent_panel.rs`, `app_menus.rs`,
and the admitted set, all in code paths the delta suite already covers.
Phase 2 is small in lines but heavy in delta bookkeeping. Phase 3 is the bulk:
roughly 222,000 lines and 249 files across ~54 crates with vim retained,
mechanical but ordered, with the keymap and brand-gate discipline making each
step verifiable. The end state is a build graph 18% smaller, one init path,
one render path, and a surface where a drawn control is a working control.
