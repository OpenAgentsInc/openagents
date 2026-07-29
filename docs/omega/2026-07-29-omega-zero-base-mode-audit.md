# Omega Zero Base Mode Audit

- **Date:** 2026-07-29
- **Audited repository:** `OpenAgentsInc/omega` at `/Users/christopherdavid/work/omega`
- **Audited commit:** `f3ae8769a862e61306c8e424cf59712b7d98e464` (2026-07-29)
- **Audit type:** read-only source audit. No Omega source file was changed.
- **Report location:** `openagents` repository, `docs/omega/`

This document records what Zero Base mode is, how it starts, what it draws,
what it refuses, and where its proof is incomplete. It reads the shipped
source, the delta registry `OMEGA_DELTAS.md`, and the mechanical checks in
`crates/omega_deltas/`. It does not run the Omega binary, so it makes no claim
about the rendered window.

---

## 1. Executive summary and core philosophy

### 1.1 What Zero Base is

Zero Base is the default surface of the Omega application. Omega starts with
one agent thread, the controls that operate that thread, and nothing else. The
editor around the thread is a separate mode. A person selects the editor with
the command-line option `--full-editor`.

The mode crate states the intent in its first line
(`crates/omega_zero_base/src/omega_zero_base.rs:1`):

> Zero base — one Exo thread and nothing else.

### 1.2 The three mechanisms

Zero Base uses exactly three mechanisms. The distinction decides what fails
and how (`crates/omega_zero_base/src/omega_zero_base.rs:45-56`).

| Mechanism | Meaning | Risk |
| --- | --- | --- |
| **Not rendered** | The panels, status-bar items, and Full Auto entry are never built. | Cheapest. On its own the most dangerous: the capability behind an unrendered surface stays one key press away. |
| **Disabled** | Every action outside the admitted set is refused at dispatch with a sentence. | Correct. The refusal is the reason "not rendered" is safe. |
| **Removed** | Nothing is removed. | The same binary is a full editor with `--full-editor`. An unresolvable key binding stops Omega before any window opens, so Zero Base deletes no action and edits no keymap file. |

The second mechanism is load-bearing. A surface that is only visually absent
is still reachable by a key press. The action gate is what catches that key
press.

### 1.3 The default execution model

- Zero Base is the default. `omega` with no arguments enters Zero Base.
- `--full-editor` is the only mode selector. No other argument selects the mode.
- `--zero-base` is accepted and does nothing. It asks for what it already gets.
  It is kept so that existing commands and scripts continue to work.
- Zero Base is one-way inside a process. There is no `Leave` action, no
  status-bar control, and no runtime path back to the editor.
- The mode is never written to disk. Ending the process is a complete repair.

### 1.4 Where the mode comes from

The mode is read from the parsed process command line, once, at start. It is
not a setting, not an environment variable, and not a file. The reason is
recorded in `OMEGA-DELTA-0047`: a settings value is writable by a project
settings file and by any other writer of settings. A mode that hides
authority-bearing surfaces must not be settable by something that is not the
person at the keyboard.

The mode is also not a release channel and not a second binary.
`OMEGA-DELTA-0038` requires the packaged gate to open every executable that
ships. A second binary doubles that surface.

### 1.5 Audit verdict

The implementation matches its stated policy on the paths this audit could
read. The mode has an unusual density of written rationale, and each rule has
a mechanical check in `crates/omega_deltas/`.

Three classes of weakness remain. Each is described in section 7.

1. **Proof depth.** The checks are text checks over source files. They prove
   that a line exists. They do not prove behavior. The sealed render has no
   automated visual or integration coverage at all.
2. **Gate integrity.** The action gate and the palette restriction are both
   single-slot global state in `gpui` and `command_palette_hooks`. Both remain
   publicly clearable.
3. **Documentation drift.** At least one code comment names a function that no
   longer exists, and one registry entry describes a refusal toast that the
   shipped code no longer raises.

---

## 2. Architectural flow and entry points

### 2.1 The mode crate

`crates/omega_zero_base` is a leaf crate. Its `Cargo.toml` declares no
dependencies and no dev-dependencies. This is deliberate: the crate must not
be able to read a settings store, a file, or an environment variable.

The crate holds two process globals
(`crates/omega_zero_base/src/omega_zero_base.rs:81-91`):

| Static | Type | Writer | Meaning |
| --- | --- | --- | --- |
| `ENTERED` | `AtomicBool` | `enter_from_command_line()` | The process started in Zero Base. |
| `SEALED` | `AtomicBool` | `seal()` | The Zero Base surface owns the window. |

Both transitions are one-way for the life of the process. Both use
`Ordering::SeqCst`.

The public API is small:

| Function | Purpose |
| --- | --- |
| `enter_from_command_line()` | Enter the mode. Idempotent. The only caller is the argument parser. |
| `is_active()` | Read the mode. Asked on every render. |
| `entered_from_command_line()` | The same answer as `is_active()`. Asked once at startup to decide what to install. |
| `seal()` | Seal the window. Does nothing when the mode is off. |
| `is_sealed()` | `is_active() && SEALED`. |
| `admits_action(name)` | Namespace or exact-name admission test. |
| `refusal(name)` | The one sentence a refused action answers with. |

`seal()` is guarded by `is_active()`. A build started with `--full-editor`
cannot be sealed by a stray call.

### 2.2 Command-line parsing

The parser is `clap` over the `Args` struct in `crates/zed/src/main.rs:2041`.
The relevant fields are:

- `paths_or_urls: Vec<String>` (`main.rs:2054`). A path names the project, not
  the mode.
- `zero_base: bool` (`main.rs:2068`). Accepted and never read.
- `full_editor: bool` (`main.rs:2084`). The one mode selector.
- `diff`, `dev_container`, and `demo_workroom` each declare
  `requires = "full_editor"` (`main.rs:2060`, `2133`, `2166`).

An editor-only option without `--full-editor` is a visible command-line error.
Zero Base does not silently accept a surface it cannot draw, and an
editor-only option does not silently select a different mode.

### 2.3 Entry sequence

The entry point is `main.rs:307-314`:

```rust
if !args.full_editor {
    omega_zero_base::enter_from_command_line();
    resolve_zero_base_project_arguments(&mut args);
}
```

This runs before path setup, before logging, before settings, and before any
agent UI initialization. Nothing that reads a settings file can influence the
mode.

The startup order is:

1. Parse the command line.
2. Enter Zero Base unless `--full-editor` is present.
3. Rewrite path arguments into project roots (section 3).
4. Read `--omega-send` (`OMEGA-DELTA-0093`).
5. Set the custom data directory, create paths, start logging.
6. Initialize the workspace (`main.rs:933`).
7. Install the Zero Base user-interface gate (`main.rs:938-940`).

Step 7 runs after every other `init` has registered its actions. The palette
restriction and the action gate therefore see the whole action registry.
Without the mode, step 7 does not run and Omega behaves as a build that never
had the module.

### 2.4 The user-interface module

`crates/zed/src/omega_zero_base_ui.rs` is 113 lines. It installs two things
(`omega_zero_base_ui.rs:23-26`):

1. **The palette restriction** (`restrict_command_palette`, line 33). It calls
   `CommandPaletteFilter::restrict_to(ADMITTED_NAMESPACES, ADMITTED_ACTIONS)`.
   The existing `hide_namespace` denylist stays in place, so a settings change
   that hides the agent namespace still hides it.
2. **The action gate** (`install_action_gate`, line 44). It calls
   `cx.set_action_gate(...)`. The gate runs before any listener. An action
   outside the admitted set is refused, and `report_refusal` records the
   sentence.

`report_refusal` (line 75) writes to the log only. It used to raise a toast.
The toast was removed under `omega#119` because the gate refuses actions that
the application dispatches, not only actions a person chose. A person who was
typing received a toast about `workspace::ActivatePane`, which they had never
requested. The refusal is now a log line at `info` level.

### 2.5 The admitted set

`ADMITTED_NAMESPACES` (`omega_zero_base.rs:117-125`) holds seven names:

`agent`, `command_palette`, `editor`, `markdown`, `menu`, `omega_workbench`,
`picker`.

`ADMITTED_ACTIONS` (`omega_zero_base.rs:153-221`) holds individually admitted
actions in five groups:

| Group | Examples | Reason |
| --- | --- | --- |
| Window and font | `omega::Quit`, `omega::Minimize`, `omega::ToggleFullScreen`, font size actions | Ordinary window management. |
| Settings navigation | `omega::OpenSettings`, `omega::OpenSettingsAt`, `omega::OpenSettingsPage`, `omega::OpenLegacySettings` | A provider error must be able to open the real settings window without admitting the extensions surface. |
| Identity onboarding | `onboarding::Finish` | Releases `await_identity_ready`. See section 2.6. |
| Terminal and search | `terminal::*` (14 actions), `buffer_search::*` (4 actions), `search::SelectNextMatch`, `search::SelectPreviousMatch` | The workbench terminal needs input and search. Terminal creation uses `omega_workbench`'s thread-bound wrapper, not a workspace action. |
| Folder and file | `workspace::Open`, `workspace::Save`, `project_panel::OpenInThreadTerminal` | `OMEGA-DELTA-0054`, `OMEGA-DELTA-0139`, and the scoped Files surface. |
| Voice | `workroom::StartVoice`, `ToggleVoiceMute`, `InterruptVoice`, `EndVoice`, `RetryVoice` | The composer owns five voice controls. Opening the unrendered workroom stays refused. |

`pane::*` actions stay refused. The reason is recorded in the source: the
process-wide gate cannot prove that a `pane` action was dispatched from the
terminal.

The `omega_zero_base` namespace itself is **not** admitted. It held one action,
`Leave`, and there is no leaving now. `OMEGA-DELTA-0052` states the reason: an
admitted namespace with nothing in it is a door left standing where the room
was demolished.

### 2.6 The identity onboarding admission

`onboarding::Finish` is admitted deliberately. Without it, Zero Base is a
permanent dead end on a fresh profile.

The failure was observed directly. `OMEGA-DELTA-0040` places a first launch on
identity onboarding and parks startup on `await_identity_ready`. The only
release for that wait is the first-run branch of `on_finish`, which runs from
`onboarding::Finish`. The gate refused that action. A new user reached the
identity page, created an identity, selected "Finish Setup", and nothing
happened, permanently, across restarts. The log line read
`onboarding::Finish is off in zero base`.

The admission permits *completing* the identity gate. It is the opposite of
bypassing it. `onboarding::SignIn`, `onboarding::OpenAccount`, and
`onboarding::ResetHints` stay refused.

### 2.7 The refusal sentence

`refusal(action_name)` returns one sentence
(`omega_zero_base.rs:297-302`):

> `{action_name}` is off in zero base, which shows one Exo thread and nothing
> else. Start Omega with `--full-editor` for the editor.

The sentence names the action, the mode, and the one way to get the refused
capability. It does not name a control in the window, because no such control
exists. A refusal that offered a button that does not exist would send a
person looking for it.

---

## 3. Project and workspace resolution

Zero Base draws no editor pane. A path argument therefore cannot mean "open
this buffer". It can only mean "work in this folder".

### 3.1 The plausibility rule

`crates/omega_workdir/` decides what a usable project root is. The module
answers one question: *is the working directory something a person chose?*

The rule is a plausibility test, not a project test. Requiring a marker such
as `.git` or `Cargo.toml` would refuse a plain folder of files, which is a
legitimate target for an agent and the most likely case. The rule therefore
runs the other way: reject the directories that a launcher hands over, and
accept the rest.

`NotAProjectRoot` has five typed variants
(`crates/omega_workdir/src/omega_workdir.rs:66-79`):

| Variant | Rejected because |
| --- | --- |
| `NotADirectory` | The path does not exist, or is not a directory. |
| `NotAbsolute` | A relative path names nothing on its own. |
| `FilesystemRoot` | `/` is what Finder and the Dock hand over. |
| `HomeDirectory` | Opening a home directory means scanning everything a person owns. |
| `LauncherDirectory` | Inside one of `LAUNCHER_PREFIXES`. |

`LAUNCHER_PREFIXES` holds nine entries: `/Applications`, `/Library`,
`/System`, `/bin`, `/dev`, `/etc`, `/private/var/folders`, `/sbin`, `/usr`.
Every entry is a place from which Omega is actually started by something other
than a person in a shell. A directory under `$HOME` is accepted, because
almost every real checkout is one.

The type is an enum rather than a boolean so that the surface can say which
rejection happened. "No folder is open" and "the folder you are in is the whole
disk" are different sentences.

The crate takes its inputs as parameters rather than reading ambient process
state. The reason is testability: startup is the one path that no test in the
repository reaches. The crate holds 14 unit tests.

### 3.2 Path argument resolution — `OMEGA-DELTA-0116`

`resolve_zero_base_project_arguments` (`main.rs:1675-1715`) rewrites the parsed
arguments in place. It runs only when `--full-editor` is absent.

The algorithm:

1. Return immediately when `paths_or_urls` is empty.
2. Read the current working directory. When that read fails, log and leave the
   arguments as typed.
3. For each argument:
   - When the argument contains `://`, keep it unchanged. A URL scheme belongs
     to the open listener, and reinterpreting one here would be a guess.
   - Otherwise call `omega_workdir::project_root_named(argument, cwd, home)`.
   - On success, log the resolved root and append it, without duplicates.
   - On failure, log the typed reason and keep the argument exactly as typed.
4. Write the result back to `args.paths_or_urls`.

A file argument becomes the folder that holds it. `omega src/main.rs` means
"work in `src`". A single-file worktree would repeat the `OMEGA-DELTA-0054`
failure with one file in it instead of none.

An argument that names nothing is refused rather than climbed. Falling back to
the parent of a typing error would open a directory that nobody named.

Rewriting the parsed arguments rather than deciding again further down is
deliberate. Everything after this point — the open listener, the workspace, the
worktree, and the `cwd` of an external agent — already agrees on the meaning of
a path. A second path variable for Zero Base would be a second answer to a
question that `OMEGA-DELTA-0054` already answers once.

### 3.3 Bare launch resolution — `OMEGA-DELTA-0054`

`open_zero_base_project` (`main.rs:1735-1772`) handles the bare `omega` launch.
It returns a boolean so that the caller can fall through to its ordinary empty
workspace.

1. Return `false` when the mode is off.
2. Call `omega_workdir::from_env()`. On rejection, log the typed reason and
   return `false`.
3. Call `workspace::open_paths` with the single resolved root.
4. Return `true` on success. On error, log a warning and return `false`.

The function never guesses. An implausible working directory opens no project,
and the composer states that in one line. Placing an agent's file tools in a
directory that nobody named is worse than having no directory.

This path handles the bare launch only. When a path argument was given, that
argument has already become the workspace through the open listener by the time
this function runs.

### 3.4 The defect this repaired

Zero Base originally opened no project. The consequence was not a missing
buffer. The workspace had no worktrees, so `grep`, `find_path`,
`list_directory`, `read_file`, and `terminal` all had nothing to operate on.
Every search returned no matches, and the agent reported that the workspace
appeared to be empty. That report was literally correct about the workspace and
useless about the person's code.

### 3.5 Startup branch order

`main.rs:1615` reads:

```rust
if !open_zero_base_project(&app_state, cx).await {
    // fall through to workspace::open_new
}
```

`drive_omega_send(cx).await` (`main.rs:1646`) runs after every branch. The Zero
Base branch used to return early at this point, which would have made
`--omega-send` work on an empty workspace and silently do nothing in the one
case that matters.

The `OMEGA-DELTA-0054` check asserts this order directly rather than asserting
one literal condition string. The registry records that the check's spelling
changed with `OMEGA-DELTA-0093` and that its property did not.

---

## 4. User interface and panel mechanics

### 4.1 Panel initialization

`initialize_panels` (`crates/zed/src/zed.rs:817-888`) has a Zero Base branch
that returns early.

In Zero Base the function loads the agent panel and nothing else. The project,
outline, terminal, git, debug, Agent Computer, and Sarah workroom panels are
**not rendered**: their `add_panel_when_ready` calls are skipped rather than
their code removed.

The Sarah workroom owner is still loaded (`zed.rs:861`), because voice keeps
its existing owner and state machine. Zero Base does not add that owner to a
dock.

The branch order inside the spawned task is:

1. Read the application state inside the task. Reading it outside the task
   panicked with `cannot read workspace::Workspace while it is already being
   updated`, because `initialize_panels` is called from an `observe_new` closure
   that already holds the lease. That panic stopped `omega --zero-base` before
   any window opened, on every profile, in debug and release alike.
2. `await_identity_ready(app_state, cx).await`.
3. `initialize_agent_panel(...)`.
4. `SarahWorkroomPanel::load(...)`.
5. Open the agent panel, zoom it, focus it.
6. `omega_zero_base::seal()`.

### 4.2 The seal — `OMEGA-DELTA-0053`

The seal is the second one-way transition and it is later than the mode.

Before the seal, Zero Base relied on the panel zoom. That was one control away
from being false. Selecting the sidebar toggle released the zoom and revealed
the inherited welcome surface, with "New File", "Open Project", "Clone
Repository", "Open Command Palette", "Open Settings", "Customize Keymaps",
"Explore Extensions", and "Open Agent Panel".

The action gate did not catch this. The gate refuses **actions**. The control
that released the zoom is an ordinary click listener on the title bar that
calls a workspace method. Nothing was dispatched, so nothing was refused. A
gate over actions cannot cover a surface that is merely covered.

Once sealed, the workspace renders no centre pane group, no tab bar, no
inherited title-bar controls, and no status bar.

The seal is late for a load-bearing reason. Identity onboarding is a
centre-pane item. A window with no centre pane could never show it. A mode that
sealed at startup would leave a fresh profile with nowhere to answer the
identity gate. The delta check asserts both the single call site and the order:
`await_identity_ready(app_state, cx).await.log_err();` must appear before
`omega_zero_base::seal();` in the same file.

### 4.3 The four sealed render sites

| Site | File and line | Behavior |
| --- | --- | --- |
| Centre pane group and tab bar | `crates/workspace/src/workspace.rs:9405` | `if zero_base_sealed { ... }` draws the left dock, and the centre only when `zero_base_center_visible` is true. |
| Status bar | `crates/workspace/src/workspace.rs:9710` | `.when(self.status_bar_visible(cx) && !zero_base_sealed, ...)`. |
| Reveal path | `crates/workspace/src/workspace.rs:4638` | `dismiss_zoomed_items_to_reveal` returns early. It used to close every dock that was not the one being revealed, which in this mode is the one panel the window has. |
| Title bar | `crates/title_bar/src/title_bar.rs:231` | The titlebar clears its children and returns the bare `platform_titlebar`. |

### 4.4 The status-bar rule

The status bar is not rendered in a sealed Zero Base. This repairs a second
complaint. A person hovered the bottom-left status-bar icon, read "Close Left
Dock ⌘B", selected it, and nothing happened, because `workspace::ToggleLeftDock`
is outside the admitted set.

A control that is drawn and denied is the same "looks one way, is another"
failure as the zoom, pointing the other way. The general rule follows:

> If the gate refuses an action, its control must not be drawn.

Not rendering the status bar makes every control on it obey that rule at once,
including controls that a later crate adds.

### 4.5 The centre-pane exception — `OMEGA-DELTA-0139`

The seal has one explicit exception. A plain click on a transcript file link
resolves the file against the thread's working directories, reveals the
ordinary centre pane beside the agent surface, and opens a normal editable tab
at the requested line.

State: `Workspace::zero_base_center_visible` (`workspace.rs:1369`, initialized
`false` at line 1830).

- `reveal_zero_base_center` (`workspace.rs:4555`) returns early unless the mode
  is sealed. It sets the flag, zooms every dock out, clears the zoom state, and
  emits `Event::ZoomChanged`.
- `restore_zero_base_agent_surface` (`workspace.rs:4574`) clears the flag,
  re-zooms the one open dock panel, and focuses it.
- The restore is triggered from the pane item-removed handler
  (`workspace.rs:5858`) when the sealed centre is visible and the pane reaches
  zero items.

`workspace::Save` is admitted individually so that the revealed editor can
save. The rest of the `workspace` namespace stays refused.

Callers of `reveal_zero_base_center` are the project panel (8 call sites), the
git panel (5), the workbench shell (4), the editor (1), the file peek surface
(1), and the thread view (1).

A secondary click — Command-click on macOS, Control-click elsewhere — keeps the
compact read-only modal from `OMEGA-DELTA-0119`.

### 4.6 The read-only reader — `OMEGA-DELTA-0119` and `OMEGA-DELTA-0125`

`crates/agent_ui/src/omega_file_peek.rs` is a read-only sheet in the
workspace's modal layer. `MultiWorkspace` renders that layer outside the seal.

The sheet is gated on `is_sealed()` rather than `is_active()`
(`omega_file_peek.rs:112`), because before the seal the ordinary centre pane
still exists and the ordinary path is correct.

The sheet is absolutely positioned. It takes part in no layout, so it cannot
clip or push the composer. Its height is bounded so that it covers the
transcript and not the composer.

`OMEGA-DELTA-0125` routes three further menu entries into the same sheet:
"Open Thread as Markdown" and both "Open Project Rules (AGENTS.md)" entries.
Those three used to call `add_item_to_active_pane` or `open_abs_path`, which
opened a buffer in a centre pane that a sealed Zero Base does not draw. The
buffer opened, took the composer's focus, and landed somewhere with no pixels.
An invisible success is indistinguishable from an unimplemented handler.

The sheet gained two entry points for this: `open_file` for a path that the
window already resolved, and `open_text` for a thread rendered to Markdown,
which is on no disk. A second reader for the same job would be two surfaces to
keep read-only and two to repair the next time one silently stops drawing.

### 4.7 The persistent sidebar — `OMEGA-DELTA-0130` and `OMEGA-DELTA-0148`

Zero Base has one sidebar. `agent::ToggleThreadsSidebar` toggles it. The
namespace is `agent`, which Zero Base admits.

`OMEGA-DELTA-0118` records why the namespace matters. The menu entry used to
name `multi_workspace::ToggleWorkspaceSidebar`. `multi_workspace` is outside
the admitted set, so the gate refused it before any listener ran, and the
control did nothing. Admitting the namespace would have been the wrong repair:
`multi_workspace`'s sidebar is the project switcher, and it hangs off
`MultiWorkspace`, which is *above* the `Workspace` that the seal covers. One
name added to `ADMITTED_NAMESPACES` would have restored the editor's whole
navigation inside a mode whose premise is that it is absent. The delta records
that `ADMITTED_NAMESPACES` and `ADMITTED_ACTIONS` are byte-for-byte unchanged
by it.

`OMEGA-DELTA-0130` replaced the overlay with a real column that yields:
`omega_sidebar::layout` gives the sidebar its 280px only while the content
column keeps `MIN_CONTENT_WIDTH` of 600px, and draws a 30px rail otherwise.
The stored preference is never overwritten by that yield, so widening the
window restores the sidebar.

No section in the sidebar may interrupt. `SectionBody` has no error variant. A
section that cannot load shows a quiet muted line in place. There is no toast,
no banner, no modal, and no refusal on any path in the sidebar.

`OMEGA-DELTA-0148` removed the rate-limits section, which offered neither data
nor an action. The forgiving state decoder still accepts and preserves the
retired `rate-limits` collapse key, so removing the section cannot corrupt a
person's other sidebar preferences.

### 4.8 The composer

The Zero Base composer bar is `render_zero_base_executor_bar`
(`crates/agent_ui/src/conversation_view/thread_view.rs:13592`). It is selected
at `thread_view.rs:4757`.

Contents, left to right:

1. Turn phase indicator. Rendered only while a turn exists. Permanent chrome
   that states "idle" all day is a knob; a dot that appears when the executor
   starts working is the turn talking.
2. Provider notice (`render_zero_base_provider_notice`). Asks whether anything
   on the machine can run a turn, then whether the thread has a folder. Only
   one notice is ever shown, executor first, because a thread with no executor
   cannot use a folder.
3. Flash / Pro model tier selector (`OMEGA-DELTA-0172`). Flash selects
   `google/gemini-3.6-flash`. Pro selects `openagents/kimi-k3` through the
   OpenAgents language-model provider.
4. Exo inspector toggle, when an Exo connection exists.
5. Voice controls.
6. Send button, which becomes the stop control while a turn generates.

`OMEGA-DELTA-0149` removed the executor dropdown. `OMEGA-DELTA-0150` removed
external provider controls. `OMEGA-DELTA-0055` removed the pin control, which
had shown wire tokens (`native_loop`, `external_acp`, `engine_lane`) to a
person as a choice.

Detection, attachment, warming, and internal routing for Codex, Claude Code,
Grok, and Exo remain loaded. Only the presentation changed.

`OMEGA-DELTA-0122` gives the composer a draft that survives connection.
`ConversationView` draws its own composer for the whole `Loading` state, and
`hand_loading_draft_over` moves the typed text and the caret offset into the
real composer when one exists.

### 4.9 The folder picker — `OMEGA-DELTA-0140`

Zero Base always renders the thread's working-folder value in the thread
header. The implementation is `render_thread_identity` in
`crates/agent_ui/src/agent_panel.rs`.

With an attached folder, the header shows the bounded path. Without one, it
shows `No folder attached`. Both states open the ordinary folder picker in the
current window, with `create_new_window: Some(false)`.

The delta check asserts that the header body does not contain `.next()?`. A
control that disappears when there is no worktree disappears at exactly the
moment a person most needs it.

The initial composer warning keeps its prominent "Open Folder" action. It is no
longer the only route.

### 4.10 Onboarding subtraction — `OMEGA-DELTA-0051`

In Zero Base, `render_basics_page` renders the identity section and nothing
else (`crates/onboarding/src/basics_page.rs:756`).

Theme, base keymap, agent install, import settings, vim mode, worktree
auto-trust, and telemetry are not rendered. Each has a shipped default, and
Zero Base takes it rather than asking.

The sharpest form of the original complaint was the agent grid: it showed Codex
with a green check, which means detection had already happened, and asked about
it anyway.

The identity step stays. `OMEGA-DELTA-0040` binds the identity gate and states
that it does not cover *what* onboarding asks for. The preference chrome is
outside that boundary and may go. The identity gate is inside it and stays.

### 4.11 Visual test captures

`crates/zed/src/visual_test_runner.rs` records four baselines through
`run_omega_exo_visual_tests` (line 11190 onward):

| Baseline | Size | Surface |
| --- | --- | --- |
| `omega_exo_workspace_wide` | 1320 x 860 | `FullEditor` |
| `omega_exo_workspace_narrow` | 720 x 900 | `FullEditor` |
| `omega_zero_base_wide` | 1320 x 860 | `ZeroBase` |
| `omega_zero_base_narrow` | 720 x 900 | `ZeroBase` |

The order is the mechanism. `omega_zero_base::enter_from_command_line()` is
called at line 11246, after the two ordinary captures. The mode is a process
global that is entered once and never left, so the first two scenes are
ordinary only because they were taken first.

The runner does not trust the reading order of the function. Each capture
asserts per scene (line 11384):

```rust
omega_zero_base::is_active() == (surface == ExoSceneSurface::ZeroBase)
```

A scene recorded in the wrong order fails instead of filing a subtracted window
under an ordinary name.

The runner is a separate binary with its own `main`. It never parses `Args`, so
the shipped default does not affect it. The ordinary-surface baselines still
photograph something that happens: what a person sees with `--full-editor`.

Every wait in the capture is bounded. The capture originally used
`run_until_parked`, which returns only when the scheduler has nothing left to
run. That is correct for a suite of simulated tasks and wrong while a real
`exo acp` child is attached, because the transport's read of the child's stdout
becomes runnable again as soon as it is polled. The runner spun on one core and
hung before any screenshot. Each wait now spends a step budget and moves on.

The capture ends the `exo acp` process by name rather than waiting for
`AcpConnection`'s `Drop`. Sampled at 100ms intervals with that call removed, one
capture's child was still alive while the next capture's child was starting.

The runner no longer compiles `omega_zero_base_ui`. It used to, because it
installed the mode's status-bar control by hand. That control is gone. The
palette restriction and the action gate are proved by the
`command_palette_hooks` restriction test and by the `gpui` action-gate keystroke
test, neither of which a screenshot could show.

---

## 5. Agent Client Protocol and agent integration

### 5.1 Session working directories

`AcpConnection::session_directories`
(`crates/agent_servers/src/acp.rs:1204-1246`) decides which directories a
session opens in.

The fallback is the project's **visible worktree roots**, not
`default_path_list`:

```rust
let project_roots = if project.visible_worktrees(cx).next().is_some() {
    project.default_path_list(cx)
} else {
    PathList::default()
};
```

The comment at line 1212-1218 gives the reason. `default_path_list` substitutes
the home directory when there are no worktrees. The thread header renders
exactly these roots and states "Choose a folder" when there are none, so an
empty list here is the header's own empty state. Substituting the home
directory would place the agent in a folder that nothing on screen ever named.

This preserves the `OMEGA-DELTA-0054` and `OMEGA-DELTA-0140` contract across
the Agent Client Protocol boundary. The header, the file tools, and the agent
`cwd` give one answer.

### 5.2 Directory existence — `OMEGA-DELTA-0158`

A thread's recorded working directories outlive the directories themselves.
`session_directories` therefore asks the filesystem before sending, rather than
letting the agent refuse the whole request.

The failure this repairs: the header showed a valid path, the agent was started
in a temporary directory left over from an earlier run, and the whole thread
became unusable behind `Failed to Launch — Invalid params: cwd does not exist`.

Only a local project probes the filesystem
(`let fs = project.is_local().then(...)`). A remote project starts its agent
through its remote client, where `cwd` names a directory on another host, so
nothing here can answer for it.

The delta check `a_session_never_opens_in_a_directory_that_is_gone` asserts four
properties, each of which fails silently on its own: the decision can reject a
directory at all; the substitute is the value the header renders; a remote
project is not probed; and the refusal, when nothing is left, names the control
that fixes it.

### 5.3 Routing — `OMEGA-DELTA-0150`

A new Zero Base chat always creates its session on Omega's native loop. The
unpinned routing decision is always `NativeLoop` with `UnpinnedDefault`.

`crates/agent_ui/src/agent_panel.rs:2186-2189` logs the correction when the
selected agent is not `NativeAgent`:

> `OMEGA-DELTA-0150`: a new thread in zero base is built on Omega's ...

Detecting or attaching Codex, Claude Code, Grok, or Exo is not authority to
hand that executor the conversation. External executors remain detected,
attached, and warmed behind Omega, so internal routing and delegation
infrastructure stays available.

`DetectedExternalAcp` remains readable in durable route records for sessions
created by older builds. The current routing law no longer emits it.

### 5.4 The two-selection defect — `OMEGA-DELTA-0131`

This entry records a truthful-labeling failure that is worth restating, because
it is the class of defect this mode is most exposed to.

A person selected Exo, saw **Exo** in the composer selector, asked the thread
who it was, and read back that it was Codex. Every surface in that window was
truthful except one. The thread was titled "New Codex Thread". The composer said
"Message Codex". The reply said Codex. Only the selector said Exo, and the
selector was the control the person had just used.

There were two independent agent selections:

- `omega_executor_selector::SELECTED` — read by `OmegaRouterServer::connect`
  when it fills the router's one external-ACP slot.
- `AgentPanel::selected_agent` — serialized per workspace, also written to a
  global last-used agent, and deciding which `AgentServer` the conversation is
  built on.

The panel had held `Agent::Codex` since an earlier session, so the conversation
held Codex's own server and the router was never in the path. Choosing an
executor did everything it was designed to do: it debounced, dropped the cached
connection, and rebuilt. It rebuilt Codex, three times in six seconds.

`OMEGA-DELTA-0149` and `OMEGA-DELTA-0150` closed this by removing the selector
and making the routing decision unconditional.

### 5.5 Exo boundary — `OMEGA-DELTA-0144`

Exo is absent unless the person launching the exact process opts in with
`--enable-exo` (`main.rs:278-280`). That read happens before paths, logs,
settings, and agent UI initialization, so none of those surfaces can discover or
start Exo on a default launch.

The Exo boundary is a process-level boundary. It is not exposed through a
composer selector.

### 5.6 Unattended turns — `OMEGA-DELTA-0093`

`--omega-send TEXT` sends one message on the thread that Omega opens, with
nobody at the keyboard. Companion options are `--omega-send-transcript`,
`--omega-quit-after-send`, and `--omega-send-timeout-secs` (default 300). Each
declares `requires = "omega_send"`.

The state is a `OnceLock<Option<OmegaSend>>` global, set once at startup
(`main.rs:1791`). The rationale is the same as the mode's:
`restore_or_create_workspace` is four call sites deep and takes no `Args`.

The send itself is `AgentPanel::omega_send_first_message`, which is a thin
wrapper over the call that the git panel's review action already makes. Nothing
in the driver talks to a connection, builds a prompt, or touches an `AcpThread`
except to read its status. A control surface that bypassed the production path
would prove nothing about the production path.

Every wait polls with a deadline rather than sleeping for a guessed duration.
Connecting an external agent and completing initialization are real
input/output whose length is a property of the machine.

`--omega-quit-after-send` exits with status 0 when the turn completed and
non-zero when it did not. A window that stayed alive is not a turn that
happened.

The driver refuses when the panel has no project, and reports the refusal
rather than retrying. A thread whose file tools have no worktree is the
`OMEGA-DELTA-0054` failure, and waiting for it to stop being true would spend
the whole budget.

---

## 6. Delta specification ledger

`OMEGA_DELTAS.md` holds 367 delta references across the registry. Every delta
has an identifier, a programmatic check in `crates/omega_deltas/`, and its own
test. Removing a delta is a policy change and requires deleting the entry, the
check, and the test together.

The deltas below define or constrain Zero Base behavior.

### 6.1 Core mode deltas

| Delta | Title | Rule | Check |
| --- | --- | --- | --- |
| 0047 | Zero base is read from the process command line and from nowhere else | The mode comes from the parsed command line, once. Never a setting, environment variable, release channel, or second binary. | `zero_base_is_entered_only_from_the_command_line` |
| 0048 | Zero base hides by filter and by refusal, and deletes nothing | Not-rendered plus disabled. No action and no key binding is deleted. A refusal is a sentence. | `zero_base_hides_by_filter_and_refusal_and_deletes_nothing` |
| 0049 | A zero-base turn still names its executor | The executor disclosure line is drawn by the same binding from the same typed record. No Zero-Base-specific rendering path for the line. | `a_zero_base_turn_still_names_its_executor` |
| 0050 | Zero base opens no authority path | The mode pins nothing, reaches no Full Auto path, writes no Exo configuration, and adds no identity bypass. | `zero_base_opens_no_authority_path` |
| 0051 | Zero base derives its setup, and can finish the one step it still asks for | Identity section only. `onboarding::Finish` is admitted; `SignIn`, `OpenAccount`, and `ResetHints` stay refused. | `zero_base_derives_setup_and_can_finish_identity_onboarding` |
| 0052 | Zero base is the default, the editor is a flag, and there is no way out | `omega` enters Zero Base. `--full-editor` opens the editor. The exit is deleted, not hidden. Editor-only options require the flag. | `zero_base_is_the_default_and_has_no_way_out` |
| 0053 | A sealed zero base does not render the editor | Once sealed: no centre pane, no tab bar, no inherited title-bar controls, no status bar. The seal is after the identity gate and happens exactly once. | `a_sealed_zero_base_starts_without_an_editor` |
| 0054 | Zero base opens the directory it was started in, or says it opened none | A plausible working directory becomes the project. An implausible one opens nothing and the composer asks. | `zero_base_opens_the_directory_it_was_started_in` |
| 0116 | A path argument names the project, never the mode | A path sets the project. `--full-editor` is the only mode selector. The local launcher passes no implicit flag. | `a_path_argument_sets_the_project_and_never_the_mode` |

### 6.2 Surface and control deltas

| Delta | Title | Rule |
| --- | --- | --- |
| 0021 | A thread names the executor that did its work | The executor disclosure record. Zero Base renders it in the composer bar. |
| 0040 | Startup skips onboarding and opens the front door | The identity gate and `await_identity_ready`. Constrains when the seal may happen. |
| 0093 | A turn can be driven without a keyboard | `--omega-send` drives the shipped send path. Runs after every startup branch. |
| 0100 | The composer stays at the bottom and the transcript grows up to it | An empty transcript claims the vertical space. |
| 0118 | Zero base's threads sidebar is its own | `agent::ToggleThreadsSidebar` in the admitted `agent` namespace. The admitted set does not move. |
| 0119 | A file link in the transcript opens a reader | The read-only sheet in the modal layer. Failure also draws. |
| 0122 | A wait a person can type through | A typable composer during `Loading`. The draft and caret are handed over. |
| 0125 | The thread header's menu does what it says, or is not there | Three menu entries route into the reader. Extensions is the one entry the seal guard hides. |
| 0130 | Zero base gets one persistent sidebar | One yielding column, not an overlay. No section may interrupt. |
| 0131 | Zero base has one agent selection | The label never names a choice that the application is not keeping. |
| 0139 | Transcript file links choose editing or peeking | Plain click reveals the editable centre. Secondary click keeps the read-only modal. |
| 0140 | The thread header owns a persistent folder picker | The folder value is always rendered. Both states open the picker. |
| 0144 | Exo is absent unless opted in | `--enable-exo`, read before any other subsystem. |
| 0147 | Sealed zero base keeps the native window drag strip | `PlatformTitleBar` renders as an empty platform strip. Traffic-light spacing, drag, and double-click behavior are kept. |
| 0148 | The sidebar contains no empty rate-limits section | The retired collapse key is still accepted by the decoder. |
| 0149 | The composer does not render an executor selector | Presentation boundary only. Detection and routing are unchanged. |
| 0150 | Every new chat belongs to Omega | Unpinned routing is always `NativeLoop` with `UnpinnedDefault`. |
| 0157 | The titlebar view is installed by the shipped binary | `title_bar::init` is called from Omega's own startup. Two layout actions are hidden from the palette rather than shown. |
| 0158 | A session never opens in a directory that is gone | Recorded working directories are checked before send. Local projects only. |
| 0172 | Flash / Pro model tier in the zero-base input bar | Flash selects `google/gemini-3.6-flash`. Pro selects `openagents/kimi-k3`. |

### 6.3 Superseded and amended rules

The registry records supersession rather than rewriting history. The important
chains are:

- **0047 → 0052.** 0052 changed which way the default points. 0047's rule about
  *where* the mode is read from did not move.
- **0052 → 0116.** 0052 originally read a non-empty `paths_or_urls` as a request
  for the editor. 0116 overruled it. A positional path is not a flag, and
  `omega <directory>` is the most ordinary command there is.
- **0053 → 0139.** The seal permits one explicit exception: a transcript file
  link may reveal an editable centre pane until its final tab closes.
- **0053 → 0147 → 0157.** 0147 restored the platform drag strip inside the seal.
  0157 records that `title_bar::init` had been orphaned when the collab crate
  was deleted, which left every window in every mode unmovable.
- **0049 → 0055.** The executor pin control was removed from the disclosure row.
  The pin assertion was removed from 0049's check because the policy changed,
  not to make anything pass.
- **0118 → 0130.** The overlay sidebar became one persistent yielding column.
- **0115 / 0131 → 0149 → 0150.** The executor selector was introduced, found to
  be lying, then removed, then made unnecessary by unconditional routing.

---

## 7. Gap analysis and recommendations

Findings are ordered by risk. Each names the evidence.

### 7.1 The sealed render has no automated coverage — high

**Evidence.** `crates/zed/src/visual_test_runner.rs` contains zero occurrences
of `omega_zero_base::seal`. The capture builds its own `Workspace`, adds the
agent panel by hand, and calls `panel.set_zoomed(true, ...)` directly
(`visual_test_runner.rs:11409-11418`). It never calls `initialize_panels`, which
is the only function that seals.

**Consequence.** The two Zero Base baselines photograph an **active but
unsealed** window. Every property that `OMEGA-DELTA-0053` claims — no centre
pane group, no tab bar, no inherited title-bar controls, no status bar — is
outside the photographed surface. The baselines record the pre-seal zoom state,
which is exactly the state that 0053 exists to replace.

`OMEGA-DELTA-0053` states this honestly in its own "What this does not cover"
section: the seal is reached only in a process that started in Zero Base and
answered the identity gate, so compilation, tests, and lint say nothing about
whether the window looks right.

**Recommendation.** Add a fifth capture that calls `omega_zero_base::seal()`
before rendering, or drive `initialize_panels` in the runner. A sealed baseline
would be the first mechanical proof that the four render sites do what the
delta says. Until then, the sealed surface is proved only by human observation.

### 7.2 The checks are text checks, not behavior checks — high

**Evidence.** `a_sealed_zero_base_starts_without_an_editor`
(`crates/omega_deltas/src/omega_deltas.rs:9136`) reads source files with
`std::fs::read_to_string` and asserts substrings such as
`"if zero_base_sealed {"` and
`"self.status_bar_visible(cx) && !zero_base_sealed"`. Every Zero Base check in
the file follows the same pattern.

**Consequence.** The checks catch a rebase that reverts a line. They cannot
catch a change that keeps the line and breaks its meaning, such as a render
branch that keeps the guard and draws the centre pane inside it anyway.

The registry is aware of this class of weakness. `OMEGA-DELTA-0116` records
three checks that were vacuous when first written and that only mutation
testing found.

**Recommendation.** For the highest-value rules, add an entity-level test that
constructs a `Workspace` with the seal on and asserts the rendered element tree,
rather than the source text. `Workspace::center_visible_for_tests`
(`workspace.rs:4569`) already exists and is a usable starting point;
`test_restoring_zero_base_agent_surface_does_not_reenter_workspace`
(`workspace.rs:17091`) shows the shape.

### 7.3 The action gate is a single global slot and remains clearable — medium

**Evidence.** `App::set_action_gate` documents "One gate at a time; installing
a second replaces the first" (`crates/gpui/src/app.rs:2177`).
`App::clear_action_gate` is public and unguarded (`app.rs:2183`).
`CommandPaletteFilter::clear_restriction` is likewise public
(`crates/command_palette_hooks/src/command_palette_hooks.rs`).

`OMEGA-DELTA-0052` states that Zero Base removed `clear_restriction` and
`clear_action_gate`. That is accurate about the `omega_zero_base_ui` module's
own wrappers. It is not accurate about the underlying primitives, which remain
callable from any crate in the workspace.

**Consequence.** Any future crate that installs its own action gate silently
removes Zero Base's gate. The mode would then be "not rendered" only, which the
crate documentation itself calls the most dangerous mechanism on its own. There
is no check that asserts exactly one production caller of `set_action_gate`.

**Recommendation.** Add a delta check that asserts `set_action_gate` and
`clear_action_gate` have exactly one production call site each in
`crates/zed/src/omega_zero_base_ui.rs`, with test files excluded. The existing
check already reads for the string `"cx.set_action_gate("`; the missing part is
the uniqueness assertion. Consider making `gpui` reject a second gate
installation while one is active.

### 7.4 A code comment names a function that does not exist — medium

**Evidence.** `crates/agent_servers/src/acp.rs:1215` reads:

> The thread header renders exactly these roots
> (`render_zero_base_working_directory`, OMEGA-DELTA-0140) ...

`grep -rn "render_zero_base_working_directory" --include="*.rs"` returns exactly
one hit: that comment. The actual implementation is `render_thread_identity` in
`crates/agent_ui/src/agent_panel.rs`, which is what the
`the_thread_folder_is_a_persistent_picker_control` check reads.

**Consequence.** A reader who follows the comment finds nothing. The repository's
own delta rule states that "the identifier appears in the code it governs, so a
reader who finds the code finds the reason". A stale cross-reference weakens
that guarantee at the one place where the Agent Client Protocol boundary and
the header contract meet.

**Recommendation.** Update the comment to name `render_thread_identity`. This is
a one-line documentation repair in the Omega repository and is outside the scope
of this read-only audit.

### 7.5 A registry entry describes behavior that was later removed — medium

**Evidence.** `OMEGA-DELTA-0118` states that the refusal "now reads the
workspace through" a corrected path, which restored the refusal toast. The
shipped `report_refusal` (`crates/zed/src/omega_zero_base_ui.rs:75-96`) writes a
log line only. The comment in that function records the removal under
`omega#119` and gives the reason: the gate refuses actions that the application
dispatches, not only actions a person chose.

`OMEGA-DELTA-0125` records the same repair and states that its version was
discarded on the rebase.

**Consequence.** Three entries — 0048, 0118, and 0125 — describe a visible
refusal that no longer occurs. The mode's stated safety argument is that hiding
a surface is safe *because* something refuses it out loud. In the shipped build
the out-loud half is a log line. That is a defensible decision, and it is not
the decision the registry describes.

**Recommendation.** Add a short amendment to 0048 and 0118 that records the
omega#119 reversal and its reason, in the same supersession style that 0047,
0052, and 0116 already use. The reasoning is sound; only the record is stale.

### 7.6 A partly dead refusal path remains — low

**Evidence.** `AgentPanel::refuse_in_zero_base`
(`crates/agent_ui/src/agent_panel.rs:3528`) still raises a toast. Its two
callers are `open_full_auto` (line 3561) and `toggle_full_auto` (line 3583).
Both are also reachable through the actions `full_auto_panel::OpenLauncher` and
`full_auto_panel::ToggleFocus`, and the `full_auto_panel` namespace is refused
by the process-wide gate before any listener runs.

**Consequence.** The gate refuses the action first, so the toast normally cannot
be reached from an action dispatch. It remains reachable from the two direct
callers at `agent_panel.rs:484` and `490`. The result is one refusal path that
toasts and one that logs, with no written rule about which applies where.

**Recommendation.** Either state the rule — direct programmatic callers toast,
gate refusals log — in `omega_zero_base_ui.rs`, or route both through one
helper. This is small, but the mode's own history shows that two paths for one
refusal is how one of them silently stops being the one that runs.

### 7.7 The argument resolver has no unit test — low

**Evidence.** `resolve_zero_base_project_arguments` lives in
`crates/zed/src/main.rs`, which is a binary crate with no `mod tests` for it.
Its behavior is guarded only by
`a_path_argument_sets_the_project_and_never_the_mode`, which asserts that the
body contains `omega_workdir::project_root_named(` and is longer than 200
characters.

The rules that the function itself implements have no direct test: the `://`
passthrough, the deduplication of resolved roots, and the leave-as-typed
fallback on rejection.

**Recommendation.** Move the loop into `omega_workdir` as a pure function over
`(Vec<String>, cwd, home)` and test it there. That crate already takes its
inputs as parameters for exactly this reason, and it already holds 14 tests.

### 7.8 `--zero-base` and `--full-editor` do not conflict — low

**Evidence.** `zero_base` (`main.rs:2067`) declares only `#[arg(long)]`. No
`conflicts_with` is present.

**Consequence.** `omega --zero-base --full-editor` opens the full editor
silently. The person asked for two opposite modes and received one, with no
diagnostic. This is a small case, and the argument is documented as accepted and
ignored, but the repository's own standard elsewhere is a visible command-line
error rather than a silent selection: `--diff`, `--dev-container`, and
`--demo-workroom` all declare `requires = "full_editor"`.

**Recommendation.** Add `conflicts_with = "full_editor"` to the `zero_base`
field, or log one line at startup when both are present.

### 7.9 Terminal admission is process-wide — low, accepted

**Evidence.** Fourteen `terminal::*` actions and four `buffer_search::*` actions
are admitted by exact name (`omega_zero_base.rs:153-192`). The gate has no
knowledge of the surface that dispatched them.

The source records the accepted limit
(`omega_zero_base.rs:107-112`): `pane` actions stay refused because the
process-wide gate cannot prove that they were dispatched from the terminal.

**Consequence.** If any terminal exists anywhere in the process, these actions
are admitted from any focus context. The keymap contexts
`AgentPanel > Terminal` and `WorkbenchTerminal > Terminal` narrow this in
practice, and the crate's own test
`workbench_terminal_keymaps_win_without_admitting_pane_actions` asserts the
ordering that makes those contexts win.

**Recommendation.** No change. This is a documented and bounded acceptance. It
is recorded here so that a future reader does not mistake it for an oversight.

### 7.10 Duplicate mode readers — informational

`is_active()` and `entered_from_command_line()` return the same value. The
source explains that they are kept separate because the two questions are asked
for different reasons: one decides what to install once at startup, the other is
asked on every render.

This is intentional and documented. It carries a small risk that a future
change to one accessor is not applied to the other. No action is recommended,
but the pair should be read together in any change to the mode's lifetime.

---

## 8. File index

| Path | Lines | Role |
| --- | --- | --- |
| `crates/omega_zero_base/src/omega_zero_base.rs` | 503 | The mode. Two process globals, the admitted set, the refusal sentence, four unit tests. |
| `crates/zed/src/omega_zero_base_ui.rs` | 113 | The palette restriction and the action gate. |
| `crates/zed/src/main.rs` | 2466 | Argument parsing, mode entry, path resolution, project opening, unattended send. |
| `crates/zed/src/zed.rs` | — | `initialize_panels`, the Zero Base branch, and the one seal call site. |
| `crates/workspace/src/workspace.rs` | — | The sealed render sites and the centre-pane reveal state. |
| `crates/title_bar/src/title_bar.rs` | — | The sealed titlebar and the layout-action palette filter. |
| `crates/omega_workdir/src/omega_workdir.rs` | — | The plausibility rule. 14 unit tests. |
| `crates/agent_servers/src/acp.rs` | 5477 | Session directory resolution across the Agent Client Protocol boundary. |
| `crates/agent_ui/src/agent_panel.rs` | — | The thread header, the folder picker, the Zero Base key context. |
| `crates/agent_ui/src/conversation_view/thread_view.rs` | — | The Zero Base composer bar. |
| `crates/agent_ui/src/omega_file_peek.rs` | — | The read-only reader sheet. |
| `crates/agent_ui/src/omega_sidebar.rs` | — | The persistent sidebar layout and state. |
| `crates/zed/src/visual_test_runner.rs` | 13518 | Four baselines. Two are Zero Base, both unsealed. |
| `crates/omega_deltas/src/omega_deltas.rs` | — | Every mechanical delta check. |
| `OMEGA_DELTAS.md` | 7587 | The delta registry. |

## 9. Verification commands

These commands were used to locate the evidence in this report. They are
recorded so that a later reader can repeat the audit.

```sh
# Every mechanical delta check.
cargo test -p omega_deltas

# The mode's own tests.
cargo test -p omega_zero_base

# The plausibility rule.
cargo test -p omega_workdir

# Locate every Zero Base reference.
grep -rn "zero_base\|ZeroBase" --include="*.rs" crates/

# Confirm the seal is absent from the visual runner.
grep -c "omega_zero_base::seal" crates/zed/src/visual_test_runner.rs
```

## 10. Scope limits

- This audit is read-only. It changed no file in the Omega repository.
- No Omega binary was built or started. Every statement about the rendered
  window comes from source and from the delta registry, not from observation.
- The `crates/omega_deltas` checks were not executed during this audit. Their
  content was read.
- Sections 4 and 5 describe surfaces across `agent_ui`, `workspace`,
  `title_bar`, and `agent_servers`. Those crates were read at the Zero Base call
  sites only, not audited in full.
