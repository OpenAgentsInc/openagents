# 40. The desktop app

> **Status: implemented.** Coder runs as a native macOS desktop app
> (`apps/coder-desktop/`): a Coax state machine and GPU-rendered UI
> (`page.coax`) hosted inside an AppKit window (`App.swift`) with a native
> text editor (`KitEditorMac.swift`). It streams replies, thinking blocks,
> commands and file views from background worker processes, runs multiple
> chats concurrently (including headless background chats), persists chats
> on disk, and loads installed skills. `coax/bin/coax test
> apps/coder-desktop/page.coax --library` passes all 206 checks (25 `#test`
> blocks in `page.coax` plus its libraries).

The terminal UI ([Chapter 36](ch036-a-terminal-ui.md)) keeps Coder inside a
shell the person already has open. The desktop app takes the same Coax
engine and worker processes and puts them in a native macOS window built on
the shared GPU component carrier: crisp text and layout on Metal, a real
AppKit text editor for the composer, and a sidebar for switching between
concurrent chats without interrupting a running reply.

## What runs

```sh
apps/coder-desktop/build.sh --open                       # build Coder.app and launch it
coax/bin/coax test apps/coder-desktop/page.coax --library # 206 checks across page.coax and its libraries
```

`build.sh` compiles the Coax UI to C (`coax c app.coax --library`), compiles
the Metal shader (`coax metal components/backend/gpu-shader.coax`), links
the C/Objective-C carrier and Swift AppKit host into `Coder.app`, builds the
`mcworker` and `claudeworker` binaries into `Contents/Resources/`, copies
fonts and icons, and ad-hoc signs the bundle.

## Architecture: Coax state machine on a native GPU carrier

The app is split across three narrow layers so the UI logic stays in Coax
and testable without a window server:

| Layer | Files | Responsibility |
| --- | --- | --- |
| Coax UI & state | `apps/coder-desktop/app.coax`, `apps/coder-desktop/page.coax` (2,053 lines) | Conversation model, layout, Markdown/code rendering, sub-agent cards, saved-chat JSON serialization, keyboard/mouse actions |
| C/Metal carrier | `apps/coder-desktop/host.h`, `host.c`, `carrier.c`, `apps/shared/host/apple/ui-host.m` | Compiles `page.coax` into an opaque C ABI (`cd_*`), drives the Metal view (`components/backend/gpu-shader.coax`), and ticks animations |
| Swift AppKit host | `apps/coder-desktop/App.swift` (655 lines), `KitEditorMac.swift`, `Bridge.h` | Window lifecycle, native text editor, worker process spawning (`mcworker`, `claudeworker`), disk persistence, folder picker, `~/.claude/skills` discovery |

### The C ABI boundary (`host.h`)

Swift never reaches into Coax structs. Everything crosses `host.h` through
opaque `cd_*` functions:

- **Lifecycle and frames:** `cd_open`, `cd_view`, `cd_frame`, `cd_moving`,
  `cd_on_change`, `cd_close`, `cd_tick`.
- **Native editor bridge:** `cd_env` passes focus, editor height and reduced
  motion into Coax; `cd_editor` returns the composer's bounding box so
  AppKit positions `KitEditorMac` over the Metal view; `cd_style` hands
  AppKit the current font and colors; `cd_set_text`, `cd_submit`, `cd_new`
  and `cd_action` (sent, focus, stop, new, copy, folder, open chat) drive
  interaction.
- **Layout and preferences:** `cd_zoom` / `cd_zoom_set`, `cd_sides` /
  `cd_sides_set`, `cd_pane`, `cd_folder`, `cd_skills`, `cd_copied`.
- **Worker and sub-agent I/O:** `cd_task`, `cd_answer_line`, `cd_stop_line`,
  `cd_forget_line`, `cd_line`, `cd_failed`, `cd_running`, plus the
  `cd_agent_*` functions for delegated sub-agents and `cd_claude` /
  `cd_agent_claude` for Claude Code routing.
- **Persistence:** `cd_chats`, `cd_current`, `cd_open_id`, `cd_title`,
  `cd_chat_at`, `cd_save_due`, `cd_save`, `cd_load`, `cd_recall_line`.

Because the composer is a real AppKit text view positioned by `cd_editor`,
macOS text editing, IME input, spellcheck, selection and clipboard work
without reimplementing them on the GPU, while the conversation, sidebars,
cards and streaming Markdown render in one Metal pass.

## Concurrent chats and background runs

A person rarely waits on one long-running task with nothing else to do.
`App.swift` keeps a live headless Coax page (`cd_headless`) for any chat
that is still running when you switch away or press Command-N to start a
new chat:

- Switching chats swaps the active page (`cd_take`) while the previous
  chat's worker process keeps streaming lines into its headless page.
- `cd_tick` advances animations and timers on background pages, and
  `cd_save_due` triggers disk writes as turns complete.
- Chats persist as an `index.json` manifest plus one `<ID>.json` file per
  chat, restored via `cd_load` and fed back to the worker as conversation
  context via `cd_recall_line`.

## Working folders, skills and controls

- **Working folder:** Each chat targets a repository directory (`cd_folder`),
  passed as `cwd` in every worker task.
- **Installed skills:** `App.swift` scans `~/.claude/skills` and passes the
  discovered skill names and descriptions to `page.coax` (`cd_skills`) and
  into the worker prompt so installed skills and slash commands are
  reachable from the desktop composer.
- **Controls:** Stop cancels the active worker (`cd_stop_line`), New Chat
  (Command-N) resets or forks into a fresh chat (`cd_new` /
  `cd_forget_line`), Copy copies a reply or code block (`cd_copied`), and
  zoom/sidebar toggles adjust the workspace without restarting workers.

## Checks

```sh
coax/bin/coax test apps/coder-desktop/page.coax --library
apps/coder-desktop/build.sh
```

## Dependencies

- [Chapter 9](ch009-microcoder.md): Microcoder, the default local loop
  running inside `mcworker`.
- [Chapter 10](ch010-drive-claude-code.md): Claude Code, driven inside
  `claudeworker`.
- [Chapter 36](ch036-a-terminal-ui.md): the shared `coder/tui/` worker
  binaries.
- [Chapter 41](ch041-worker-protocol-and-sub-agents.md): the JSON-lines
  wire protocol and sub-agent delegation between `App.swift`, `page.coax`
  and the workers.

## Done means

- [x] `apps/coder-desktop/build.sh` builds a signed `Coder.app` bundle with
  the Metal shader, AppKit host, `mcworker` and `claudeworker`.
- [x] `coax/bin/coax test apps/coder-desktop/page.coax --library` passes all
  checks without a window server.
- [x] A running reply continues in a headless page when the user opens or
  starts another chat, and saved chats reload from disk.
