# OpenAgents Terminal: scope

Status: scoping, 2026-10-01. Nothing here is built yet. The issues to open
are listed [at the end](#github-issues-to-open).

Owner's ask (2026-10-01): scope an OpenAgents Terminal app, the same idea as
Coder Terminal, kept simple, and tied into OpenAgents and everything we have
now.

## What it is

OpenAgents Terminal is a full-screen chat with OpenAgents in your terminal.
You type a message and the same chat router the phone and the desktop use
answers it. When the router judges the message is coding work, Coder runs on
this computer, in the folder you started from, and its steps stream into the
same screen. It uses the same identity as this computer's host and the same
threads (a thread started in the terminal shows in the desktop app and on a
paired phone, and the other way round). It routes through the same router and
Jev, and uses the same plugins, knowledge, settings, and coding agents (Codex,
Claude Code, Grok Build). It adds no chat logic and no Coder logic of its own:
it is a screen over `openagents chat`.

## The short answer

1. **Where it lives:** in the existing `openagents` binary. Running bare
   `openagents` on a TTY opens it, and so does `openagents terminal`. The app
   code is a new library crate, `crates/openagents-terminal`. There is no new
   binary to ship.
2. **Backend:** what `openagents chat` already does. It uses the host's
   control socket when a host runs, so threads sync with the desktop and the
   phone. Otherwise it runs the chat service in process. Coder runs through
   `coder::task::local`. First, the backend code moves out of
   `crates/openagents-cli/src/chat.rs` into a library.
3. **Rendering:** build on this repo's `crates/coder-terminal` (ratatui, the
   amber ladder, composer, markdown, scrollback, guard). It is already the
   in-repo port of Coder Terminal's look. Do not port the coder repo's
   `coder-ui-core` grid.
4. **Coder runs:** start at once by default and approve everything (#10104),
   with no step or time budgets. Esc stops the run.
5. **Distribution:** ship it inside what we already ship (the Mac `.app`'s
   `Contents/Helpers/openagents`, and the Linux AppImage/`.deb`). Add one
   `curl | sh` installer for computers without the app, with `stable`/`rc`
   channels, which is the coder repo's model.

## Prior art: Coder Terminal (`~/work/coder`, private)

It is described here, not copied.

- **Product.** `bins/coder-terminal`: a chat in the alternate screen, built
  on ratatui. It works over SSH and in tmux and has no GPU UI. Each message
  is one streamed generation over a websocket to `coder-serve`
  (`/v1/responses`, `model: "chat"`). The model's one local tool is `shell`,
  which runs on the user's machine with no approval prompt; a second tool,
  `delegate`, hands work to Codex, Claude Code, or Devin. `/cloud` orders a
  fleet run. Plugins (Ctrl+P) and the Gym (Ctrl+G) are extra screens.
- **Component library.** `crates/coder-ui-core` is a renderer-free grid of
  cells in one amber color with four intensities (`Quarter` to `Full`): 37
  components, each with golden snapshots (card, composer, markdown/prose,
  turn, transcript flow, glyphs). `crates/coder-terminal` there is the
  ratatui renderer for that grid (ladder to truecolor, 256 colors, or
  `NO_COLOR`; ten-frame braille spinner; readline-style keys).
- **Slash commands.** One catalog (`crates/coder-contract/src/command.rs`)
  with about 90 commands: clear, help, plugins, permissions, plan, export,
  resume, sessions, doctor, diff, review, commit, pr, theme, vim, logout,
  whoami, update-check, and so on. Most were never used.
- **Backend.** A GitHub browser sign-in stores a session bearer at
  `~/.openagents/session`. `login --agent` is a device-code flow. Runs and
  thread sync go over MCP tools (`order_run`, `read_run_events`, `stop_run`,
  `open_thread`, `append_entries`, `claim_turn`, …). Esc sends
  `response.cancel`. A second Ctrl+C on an empty draft quits.
- **Permissions.** Sessions start in `bypass` (allow everything). `default`,
  `accept_edits`, and `plan` are opt-in modes.
- **Install.** `ops/install-terminal.sh` (and `.ps1`), served by
  `coder-serve` at `/install-terminal.sh`, proxied from the GCS bucket
  `openagentsgemini-cli-releases`. Objects:
  - `coder-terminal.<channel>`, a bare version
  - `coder-terminal-<version>-<platform>`
  - `SHA256SUMS-coder-terminal-<version>`

  `stable` is `X.Y.Z` and `rc` is `X.Y.Z-rc.N`. A channel moves only after
  every platform's artifact and checksum read back. macOS binaries are
  codesigned (hardened runtime) and notarized; a bare binary cannot be
  stapled, so Gatekeeper is checked with `spctl` before upload. Linux builds
  for gnu and musl use cargo-zigbuild. There is no self-updater:
  `/update-check` reports, and re-running the installer updates.
- **Tests.** Golden snapshots, scripted scenarios against a fixture door,
  proptest state machines, and `ops/tests/terminal-*.sh` live-screen
  harnesses.

What carries over: the look (amber ladder, hairline frame, composer), the
alternate-screen, SSH-safe shape, approve-everything as the default, Esc to
stop, the release channel layout, and the signing steps. What does not carry
over: the `coder-serve` websocket, GitHub sessions, MCP run tools, the `shell`
tool loop, `/cloud`, and the long command catalog. OpenAgents already has its
own answer to each of those (the chat worker over NIP-CJ, the host, Coder
local runs, and NIP-HOST).

## What this repo already has

| Need | Exists now | Path |
| --- | --- | --- |
| Talk to the router, threads, stop, list, read, export | The chat service `Command`/`Snapshot`, used by phone, desktop, and CLI | `crates/openagents-chat/src/service.rs`, `basic_chats.rs`, `thread.rs` |
| Surface on the wire | `router::Surface::Terminal` (`surface: "terminal"`, `client: "openagents-cli"`) | `crates/openagents-chat/src/router.rs` |
| Three backends: host socket, in process, scratch | `Backend` in the CLI (crate-private) | `crates/openagents-cli/src/chat.rs` |
| Coder on this computer from a chat, with engine choice and failover | `coder::task::local`, wired up by the CLI | `crates/coder/src/task/local.rs`, `crates/openagents-cli/src/chat_coder.rs` |
| Coder event stream (started, step, output, progress, question, approval, result, failure, stopped) | `coder_events` + `Mapper` | `crates/openagents-chat/src/coder_events.rs` |
| Issue flow and queue | `coder::task::issue_run`, `openagents chat work` | `crates/openagents-cli/src/chat_work.rs` |
| Settings (providers, `start`, access, projects) | `~/.openagents/settings.json` | `crates/coder/src/task/settings.rs`, [docs/cli/settings.md](../cli/settings.md) |
| Chat-list ordering, commands, Coder run view state | Shared app state (no platform deps) | `crates/openagents-chat-app/src/{chat_list,commands,coder_run,task_chat}.rs` |
| Terminal design system | Amber `Intensity` ladder, hairline frame, composer and editor, readline keys, markdown, bounded scrollback, panic-safe `Guard`, two-lane events, spinner, and a Rust Native view adapter | `crates/coder-terminal/src/*`, [docs/coder/runtime/terminal.md](../coder/runtime/terminal.md) |
| A full-screen terminal already using it | `coder` with no args: the older Coder agent shell (Classify/Generate doors, not the chat router) | `crates/coder/src/main.rs` |
| Pairing a phone | The host's QR pairing; `coder_connect::pairing::terminal_qr` draws a QR as text; `openagents connect` | `crates/coder-connect/src/pairing.rs`, `crates/openagents-cli/src/connect.rs` |
| Plugins | `openagents plugin list/run/test` | `crates/openagents-cli/src/catalog.rs`, [docs/plugins](../plugins/README.md) |
| Knowledge | `openagents kb` (local NIP-KB) | `crates/openagents-cli/src/kb.rs`, `crates/knowledge` |
| Host as a service | `coder-service` (launchd/systemd, trial updates) | `crates/coder-service`, [docs/coder/runtime/host-service.md](../coder/runtime/host-service.md) |
| Release gate | `scripts/release/acceptance.sh` | [docs/release/acceptance.md](../release/acceptance.md) |
| Packaging | Mac `.app` ships `Contents/Helpers/openagents`; Linux AppImage/`.deb`; `install-coder.sh` symlink and rollback scheme | `scripts/desktop/*`, `scripts/install-coder.sh` |

Almost everything the terminal needs already runs headlessly behind
`openagents chat`. The terminal is mostly a screen and an input loop.

## Scope for v1

In:

- One full-screen screen: a transcript, a composer, and a status line
  showing the backend (`host` or `this terminal`), the project folder, the
  thread, and the engine.
- Send, stream the reply with markdown, and show the router's typed offers,
  follow-ups, and cards as text lines. A follow-up runs when the person
  picks it with a key.
- Coder runs inline: the start line (who runs and why), steps, commands with
  their exit code and last lines of output, progress, provider switches, and
  the result (files changed, worktree). Esc stops it.
- A question from Coder: the composer answers it (`answer`).
- A thread list overlay (Ctrl+T or `/threads`) using `chat_list` ordering;
  open, new, archive.
- Slash commands, as a short closed list: `/new`, `/threads`, `/stop`,
  `/export`, `/settings` (prints, and opens the file path), `/connect` (shows
  the pairing QR), `/plugins`, `/help`, `/quit`.
- `openagents terminal --scratch` for tests and smokes.
- Resume: `openagents terminal --thread ID`, and the last thread for this
  folder.

Out of v1 (named so nobody builds them by accident):

- Model or engine pickers. Settings and the router's typed engine choice
  decide.
- Permission modes, `plan` mode, and approval prompts. Everything is
  approved (#10104); the only control is Stop.
- Step or time budgets in the UI.
- Attachments, images, file pickers, paste-as-file. Text only.
- An embedded PTY/shell pane, tiling, `/cloud`, fleet runs, delegation trees,
  the Gym screens, the Verse, the wallet, and voice.
- Windows (the CLI builds there; the screen is unverified). Mouse selection
  beyond what the terminal emulator does natively.
- A self-updater. Re-running the installer updates.
- Any keyword or string matching on what the person typed. Slash commands
  are the only parse, and only of a leading `/word` against the closed list.
  Everything else goes to the router.

## Architecture

```text
openagents (crates/openagents-cli)          one binary, already shipped
  ├─ bare on a TTY / `terminal` ─► openagents-terminal (new lib crate)
  │                                   ├─ app state + input loop
  │                                   ├─ draw: coder-terminal (ratatui)
  │                                   └─ chat client ─┐
  └─ `chat …` (unchanged) ────────────────────────────┤
                                                      ▼
                         chat client lib (moved out of openagents-cli/src/chat.rs)
                           ├─ Host:      host control socket ─► coder-host ─► NIP-HOST thread.* ─► phone
                           ├─ InProcess: openagents_chat::service ─► NIP-CJ ─► chat worker (relay.openagents.com)
                           └─ Coder:     coder::task::local ─► Codex / Claude Code / Grok Build
```

Recommendation: **extend the `openagents` binary, with the app in a new
library crate `crates/openagents-terminal`.**

- One binary is already signed, bundled in the `.app`, packaged for Linux,
  and run by the release gate. A new bin would need all of that again.
- `openagents-cli` already links `coder`, which already links
  `coder-terminal`, so the TUI adds no new heavy dependencies.
- The app logic stays out of `main.rs` and stays testable without a TTY:
  state in, view lines out.
- Bare `openagents` today prints usage and exits 64. On a TTY it would open
  the terminal; piped or with `--json` it keeps printing usage. `openagents
  terminal` is the explicit spelling.

New work:

1. **Extract the chat client.** Move `Backend` (host / in-process / scratch),
   backend selection, `chat_migrate`, and the Coder run glue from
   `crates/openagents-cli/src/chat.rs` and `chat_coder.rs` into a library
   both the command and the screen call. The best home is a `client` module
   in `crates/openagents-chat`, behind a feature so phones do not link the
   Unix socket code. `openagents chat` must behave byte for byte the same,
   and its tests are the guard.
2. **Make it event-driven.** The CLI today blocks and prints. The screen
   needs the same operations as a stream of typed events (`partial`, `route`,
   `offer`, `result`, `coder`, the Coder events) on a channel. That is the
   `--json` NDJSON stream as Rust values, not text.
3. **Surface over the host.** Through the host, a request says `desktop`
   (the host adds the desktop context, #10077). A host control op must carry
   the caller's surface, so the router sees `terminal` for terminal sends
   whichever backend carries them. That is a small change in
   `crates/coder-host` (control) and in `openagents_chat` (`Context`).
4. **The screen.** In `crates/openagents-terminal`: app state (transcript of
   typed rows, composer, overlay, run state reusing `openagents_chat_app::
   coder_run`'s view model where it is platform-free), keymap, slash command
   table (a closed enum), and draw.
5. **What becomes of `coder` with no args.** It is the older Coder agent
   shell and does not use the router or threads. Once the OpenAgents
   Terminal ships, `coder` (no args, on a TTY) should open it too, or say
   where it went. See the open questions.

## Rendering stack

Recommendation: **build on this repo's `crates/coder-terminal`.** Do not port
`coder-ui-core`.

- `crates/coder-terminal` already has the parts that matter, with tests:
  - the amber `Intensity` ladder (truecolor, 256 colors, `NO_COLOR`)
  - the hairline frame
  - the composer, `Editor`, and readline keys (wide characters, ZWJ)
  - markdown
  - the wrap cache, and `Scrollback` bounded at 5,000 lines
  - the panic-safe `Guard`
  - two-lane events, so a command outcome is never dropped behind streamed
    text

  [docs/coder/runtime/terminal.md](../coder/runtime/terminal.md) covers it.
- `coder-ui-core` is a second component model (a cell grid with four
  renderers). Here, `rust-native` already plays that role, and
  `coder_terminal::native` already draws Rust Native views in the terminal.
  Porting a second grid would give us two component systems.
- Missing pieces to add to `crates/coder-terminal` (each written fresh,
  copying no coder-repo code):
  - a **turn** block (user, reply, Coder run)
  - a **card** (welcome, sign-in, pairing QR)
  - a **run** block (the step, command, and output rows the desktop draws for
    `coder_events`)
  - a **list overlay** (threads, plugins)
  - golden text snapshots for each, rendered with ratatui's `TestBackend`
- Full screen in the alternate screen, like Coder Terminal and `coder`
  today. On exit, print the thread ID and how to resume.

## Coder runs from the terminal

- **Where they run.** On this computer, in the Git checkout the terminal was
  started from. They use the same path `openagents chat` uses:
  `coder::task::local` and the host auto-start's `Policy::launch`, in a
  detached worktree, with tasks in `~/.openagents/tasks`, so the desktop and
  the phone see them. When a host runs and there is no checkout here, the
  host's own handoff runs it in the host's project, as `chat` does.
  Recommendation: do not run Coder inside the terminal process. The run is
  the `microcoder` controller process (as today), so closing the screen does
  not kill the run, and `openagents terminal --thread ID` follows it again.
  The one exception is the issue flow, which runs in the process that
  started it today; v1 should say so before it starts, or move the flow
  under the host (see open questions).
- **Starting.** `coder.start: at_once` (the default) starts as soon as the
  router judges the message is coding work. Under `ask_first`, the offer
  line takes Enter to run it. Engine choice is the router's typed Jev
  `engine` choice (INVARIANTS row on engine choice). Nothing matches words.
- **Approvals.** Approve everything by default, per #10104: no prompt for
  commands, commits, or pushes on the owner's own computer. The terminal
  never draws an approval prompt in v1. If an `approval` event still
  arrives (from an older engine), it shows as a line, and the composer
  answers it like a question.
- **Engines.** Whatever settings allow, in order: Codex, then Claude Code,
  then Grok Build, each only when signed in. The start line shows who runs
  and why, and failover shows as `provider_switched` lines. `/settings`
  shows the list. The terminal does not sign anyone into an engine; it says
  which one is missing.
- **Budgets.** None in the UI and none added by the terminal. Progress shows
  Jev's "% done" estimate, not a step count against a limit.
- **Stopping.** Esc while a reply streams stops receiving it, with the apps'
  words. Esc while Coder runs is `chat stop`, which stops the running turn.
  Ctrl+C clears the draft; a second Ctrl+C on an empty draft quits, and the
  run keeps going. `/stop` does what Esc does.

## Identity, sign-in, and pairing

- **No account sign-in.** OpenAgents chat needs no login and no model key.
  The identity is a device key, as for the CLI today:
  - **Host running** (the desktop app, or `openagents host serve
    --control`): the terminal speaks the host's control socket. Threads are
    the desktop's, and a paired phone lists and continues them (NIP-HOST
    `thread.*`, #10035). This is the "synced" mode and should be the
    normal one.
  - **No host:** in process, with `~/.openagents/chat/device.key` and its own
    encrypted store. Threads stay on this computer until a host starts, then
    join it once (`chat_migrate`).
- **Recommendation.** When no host runs, the welcome card offers one key:
  "Keep this computer's chats in sync with your phone". It installs the host
  as a user service (`coder-service`, launchd or systemd), with no Tailscale
  and no extra commands. On a Mac with the app installed, the app's host is
  used as is.
- **Pairing a phone.** `/connect` shows the host's single-use pairing code as
  a text QR (`coder_connect::pairing::terminal_qr`), with the same words as
  the desktop window. The host stays the only issuer of access. This works
  over SSH, which makes a headless Linux box pairable with no browser.
- **Surface.** `Surface::Terminal` already exists. The terminal sends
  `surface: "terminal"`, `client: "openagents-terminal"` (a new client word,
  so worker logs can tell the screen from scripts using `openagents chat`),
  plus `context.computer` and `context.project` as `chat` does (#10077).
  Through the host, the surface must survive the hop (see Architecture,
  item 3).
- **Wallet, Verse.** Out of v1. Router cards that point at the Wallet or the
  Verse show as a line saying where to open them (phone or desktop).

## Install and distribution

Three ways, in order of who gets them:

1. **With the app (no new work).** The Mac `.app` already ships
   `Contents/Helpers/openagents`. Add a menu item, **Install `openagents`
   command**, that links it into `~/.local/bin` or `/usr/local/bin`. The
   Linux AppImage/`.deb` already carry the binary; the `.deb` should put
   `openagents` on `PATH`.
2. **Installer script, for computers without the app (SSH boxes, Linux
   servers).** `curl -fsSL https://openagents.com/install-terminal.sh | sh`.
   It follows the coder repo's model, rebuilt here:
   - **Bucket.** Reuse `openagentsgemini-cli-releases` under a new prefix,
     `openagents/` (or use `openagentsgemini-oa-updates/cli/` beside the
     desktop's manifests). Objects:
     - `openagents.<channel>`
     - `openagents-<version>-<platform>.tar.gz`
     - `SHA256SUMS-openagents-<version>`
     - an Ed25519-signed manifest, using the desktop's `sign-manifest.sh`
       key scheme
   - **Channels.** `stable` and `rc`, with `OPENAGENTS_CHANNEL=rc`. A
     channel moves only after every platform's artifact and checksum read
     back.
   - **Payload.** `openagents` and `microcoder`, because a Coder run needs
     the matching controller (#10074). Install them into
     `~/.openagents/versions/openagents-<version>/` and point a symlink at
     them, with `--rollback`, the same way `scripts/install-coder.sh` does
     today.
   - **Served by.** The website Worker (`apps/openagents.com`) serves the
     script and proxies or redirects to the bucket.
3. **From source:** `cargo install --path crates/openagents-cli` plus
   `microcoder`, as now.

Platforms and signing:

- **macOS** (aarch64, x86_64, or one universal binary with `lipo`, as the
  desktop does): sign with the Developer ID `OpenAgents, Inc.
  (HQWSG26L43)`, hardened runtime, and identifier `com.openagents.cli`.
  Notarize a zip of the binaries with `notarytool`, using the credentials
  `scripts/desktop/package-macos.sh` already uses. A bare binary cannot be
  stapled, so check `spctl --assess` before upload. `curl` does not set the
  quarantine flag, so a notarized binary runs. Build it with
  `OPENAGENTS_DESKTOP_RELEASE=1` only if it should share the app's keychain
  items; the CLI's own keys are files, so likely not.
- **Linux** (x86_64 and aarch64, gnu and musl): build in Docker as
  `scripts/desktop/build-linux-release.sh` does. Checksums plus a signed
  manifest stand in for code signing.
- **Windows:** out of v1.

## Release gate scenarios

`scripts/release/acceptance.sh` needs a terminal driver. ratatui's
`TestBackend` covers unit tests. The gate should drive the real binary in a
real pseudo-terminal: a `--acceptance DIR` mode like the desktop's, which
reads scripted keys and writes the screen as text after each event. Text
captures go in the evidence folder. The gate adds `terminal` to
`--bin-dir`/`--app`, and the scenarios are:

| Scenario | Checks |
| --- | --- |
| `term-who-are-you` | A plain chat streams a reply. The status line says `host`, and the thread shows in the desktop host's list. |
| `term-working-directory` | "what's your working dir" names the scratch project (#10077). |
| `term-delegate-now` | A coding message starts Coder at once, and the screen shows the start line, steps, and result with files changed. No approval line appears. |
| `term-push-main` | "commit and push a note to main" on a scratch repo with a local bare remote finishes with no question or approval (#10104). |
| `term-stop` | Esc during a run gives one `stopped` line, and the task store shows it stopped. |
| `term-quit-run-continues` | Quitting during a run leaves it running, and `--thread ID` follows it to the result. |
| `term-followup` | A follow-up after a run is answered from `context.coder_run`. |
| `term-engine-claude` | "use claude for this" puts Claude Code first. |
| `term-threads-sync` | A thread started in the terminal is readable through the phone-shaped NIP-HOST client, and a phone follow-up appears in the terminal. |
| `term-no-host` | With no host, it runs in process, and its threads join the host when one starts. |
| `term-connect-qr` | `/connect` draws a QR whose code a phone-shaped client redeems. |
| `term-plugins` | "which plugins can I test?" names the catalog (as `plugins-chat`). |
| `term-resize-unicode` | Resize mid-stream with wide and ZWJ text: no torn rows, and the terminal is restored on exit and on panic. |
| `term-ssh-narrow` | 80x24 with `NO_COLOR`: everything readable, nothing cut. |

## Phased tasks

Sizes: S is under a day, M is 1 to 3 days, L is about a week. There are no
time or step limits on Coder runs; sizes are only for planning.

**Phase 0: groundwork**

- Extract the chat client (backends, selection, migration, and Coder glue)
  from `openagents-cli` into `openagents_chat::client`. `openagents chat`
  output must be unchanged. **M**
- Add an event stream API over it (typed events on a channel) alongside the
  existing blocking calls. **M**
- Carry the caller's surface through the host control socket. Add the
  `openagents-terminal` client word. Update the INVARIANTS row on surfaces
  if one applies. **S**
- Land #10104 (approve everything), which the terminal depends on. **M**
  (already open)

**Phase 1: the screen (v1)**

- `crates/openagents-terminal`: app state, input loop, transcript rows, and
  composer, over `coder-terminal`. **L**
- Coder run rendering (start, step, output, progress, switch, result,
  question), with Esc to stop and quit-keeps-running. **M**
- Turn, card, run, and overlay components in `coder-terminal`, with text
  snapshots. **M**
- Thread list overlay, resume (`--thread`, last-for-folder), and new
  thread. **M**
- The slash command enum (closed list) and `/help`. **S**
- Welcome card: backend, project, engines ready, and the offer to keep
  chats in sync (install the host service). **M**
- `/connect`: QR pairing through the host. **S**
- Bare `openagents` on a TTY opens the terminal; `openagents terminal`. **S**
- Docs: this folder becomes the user guide, plus updates to
  [docs/cli/README.md](../cli/README.md) and the glossary
  ("OpenAgents Terminal"). **S**

**Phase 2: ship**

- Acceptance mode (`--acceptance DIR`, scripted keys, text captures) and the
  gate scenarios above in `scripts/release/acceptance.sh`. **L**
- `scripts/terminal/release.sh`: build macOS universal and Linux gnu/musl
  for both architectures, sign, notarize, `spctl`, checksums, signed
  manifest, upload, and the channel-move-after-readback rule. **M**
- `install-terminal.sh` (served by the website Worker), with a versioned
  install, symlink, rollback, `rc` channel, and its own test like
  `scripts/test-install-coder.sh`. **M**
- Mac app menu item **Install `openagents` command**; `.deb` puts it on
  `PATH`. **S**
- Website install page row for the terminal (`crates/openagents-web/src/pages/install.rs`). **S**

**Phase 3: after v1 (not scoped further)**

- Decide what `coder` with no args becomes.
- Move the issue flow under the host, so it survives the screen closing.
- `/work` for the issue queue, a Windows build, and a plugin run picker.

## GitHub issues to open

Not opened. Proposed titles:

1. Terminal: move the `openagents chat` backends and Coder glue into `openagents_chat::client` (no behavior change)
2. Terminal: event-stream API over the chat client (typed events on a channel)
3. Host control: carry the caller's surface so terminal sends say `terminal`
4. Terminal: `crates/openagents-terminal` v1 screen on `coder-terminal` (transcript, composer, status line)
5. Terminal: render Coder runs inline; Esc stops; quitting leaves the run going
6. coder-terminal: turn, card, run, and overlay components with text snapshots
7. Terminal: thread list overlay and resume (`--thread`, last thread for this folder)
8. Terminal: welcome card and one-key host service install for phone sync
9. Terminal: `/connect` shows the host's pairing QR as text
10. Terminal: bare `openagents` on a TTY opens the terminal; `openagents terminal`
11. Release gate: terminal acceptance mode and the `term-*` scenarios
12. Release: `install-terminal.sh`, stable/rc channels, signed and notarized CLI builds for macOS and Linux
13. Mac app: menu item to install the `openagents` command; `.deb` puts it on PATH
14. Docs: OpenAgents Terminal user guide, CLI README, and glossary entry

## Open questions for the owner

1. **Name and command.** Is it "OpenAgents Terminal", opened by bare
   `openagents`? Or should the command be `oa`, or `coder`, as Coder Terminal
   was installed?
2. **`coder` with no args.** It opens the older Coder agent shell today. Send
   it to the new terminal, keep it, or retire it?
3. **Sync by default.** When no host runs, should the terminal install the
   host service on first run without asking (so threads sync with the phone),
   or offer it on the welcome card as proposed?
4. **Issue flow.** Is it OK that `work on #N` runs in the terminal's process
   in v1 (quitting mid-flow leaves the claim without a closing comment)? Or
   must it move under the host before the terminal ships?
5. **Distribution.** Is shipping inside the app plus a `curl | sh` installer
   enough? Is Homebrew wanted? Should the installer reuse the
   `openagentsgemini-cli-releases` bucket?
6. **Linux first-class?** Is a headless Linux box (over SSH, paired by QR in
   the terminal) a v1 target, or Mac only first?
7. **Website terminal.** #10106 brings back the homepage terminal chat as
   orientation. Should it look like this app (same amber design), or stay
   separate?
