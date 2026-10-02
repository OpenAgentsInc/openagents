# Coder Terminal and OpenAgents Terminal: gap analysis

Date: 2026-10-02. Compares Coder Terminal (`~/work/coder`, private, read
from source) with OpenAgents Terminal as shipped in `1.0.0-rc.2`
(`crates/openagents-terminal`, `crates/coder-terminal`, opened by
`openagents terminal`). Coder paths below are relative to `~/work/coder` and
written `coder:path`; all other paths are relative to this repo. Claims come
from the code, not only the docs. "Not found" means a search did not turn it
up, not that it is proven absent.

## Summary

**Coder Terminal** (`coder:bins/coder-terminal`, about 113,000 lines with
tests) is a ratatui text interface in the alternate screen. Each message is a
generation over a websocket to `coder-serve` (`/v1/responses`, a "lane" such
as `flash` or `pro`). The model itself has a full tool set (Read, Write, Edit,
Glob, Grep, Bash, and more) running on your machine, and hands larger work to
Codex, Claude Code, Grok, Devin, or a hosted Box. Around that it has grown a
very wide surface: about 90 slash commands, permission modes, plugins, Gym,
fleet runs, a headless writer that phones attach to, tailnet peers, an earning
mesh, and autopilot.

**OpenAgents Terminal** is a deliberately small screen (about 2,300 lines
plus the shared `coder-terminal` components) over `openagents_chat::client`.
The same chat router the phone and desktop use answers every message; when it
judges a message is coding work, Coder runs locally in its own worktree and
streams in. Threads are the desktop app's threads and sync with a paired
phone. It has 10 slash commands and no chat or Coder logic of its own.

Headline gaps (what OpenAgents Terminal lacks that Coder Terminal has and
that would matter to people):

1. **Code reading:** no syntax highlighting in code blocks and no diff view of
   what Coder changed; Coder Terminal has tree-sitter highlighting, a file
   viewer, and a patch view.
2. **Context in:** no `@` file mentions and no image attachments.
3. **Memory of the composer:** prompt history is lost on quit, and the thread
   list cannot be searched.
4. **Watching and steering a run:** a run is shown inline only; there is no
   full-screen run view and no way to steer a running run except answering
   its questions.
5. **Copying and resilience:** no mouse selection or clipboard copy of a reply
   or code block, and no explicit reconnect or offline handling.

What OpenAgents Terminal has that Coder Terminal lacks: one router and one
thread store shared with the desktop app and the phone, QR pairing of a phone
from the terminal, a one-key install of the sync host, tool-call grouping
with Jev's progress estimate, a run that survives quitting the screen, and a
closed command list with no half-working commands.

## Feature comparison

| Area | Coder Terminal | OpenAgents Terminal | Gap |
| --- | --- | --- | --- |
| What it is | ratatui and crossterm text interface, alternate screen, never links GPUI (`coder:bins/coder-terminal/Cargo.toml:1-5`). Paints a renderer-free `coder-ui-core` grid (`coder:crates/coder-terminal/src/lib.rs:1-21`). | ratatui 0.30 and crossterm 0.29, alternate screen, panic-safe guard (`crates/openagents-terminal/Cargo.toml:17,22`, `crates/coder-terminal/src/guard.rs:48-74`). A screen over `openagents_chat::client` (`crates/openagents-terminal/src/lib.rs:1-26`). | Same shape. None. |
| Launch | `coder` (installed name), flags `--resume`, `--offline`, `--attach`, `--glyphs`, `--no-color`, a file viewer flag, about 20 subcommands (`coder:bins/coder-terminal/src/cli.rs:36-198`). | `openagents terminal` or bare `openagents` on a TTY; `--thread`, `--continue`, `--scratch`, `--local`, `--socket` (`crates/openagents-cli/src/main.rs:144-172`, `crates/openagents-cli/src/screen.rs:31-100`). Prints the reopen command on quit (`screen.rs:171-235`). | OpenAgents has no `--offline` and no read-only file viewer flag. Fine otherwise. |
| Install and update | `install-terminal.sh`/`.ps1`, SHA-256 checked, `stable`/`rc` channels, no self-updater (`coder:bins/coder-terminal/README.md:57-91`). | `scripts/install/openagents.sh` and `.ps1`, SHA-256 checked, installs `openagents` and `microcoder`, `stable`/`rc`, signed manifest, notarized Mac builds (`openagents.sh:93-191`, `docs/release/terminal.md`). No self-updater. | Parity. OpenAgents lacks the versioned folder and rollback its own scope planned (`docs/terminal/scope.md:355-358`). |
| Chat backend | One websocket to `coder-serve`, `response.create` with a lane name (`coder:bins/coder-terminal/src/main.rs:1-16`, `lane.rs:1-60`). | The shared router over NIP-CJ, through the host's control socket when one runs, else in process (`crates/openagents-chat/src/client.rs:103-133,1535-1539`). | OpenAgents is ahead: one router across terminal, desktop, and phone. |
| Threads | Each session is a JSONL file under `~/.openagents/sessions/`, scrubbed of credentials; pushed as a service thread on Pro (`coder:bins/coder-terminal/src/sync.rs:1-8`, `history.rs:40-50`). | Threads are the host's (desktop app's) threads; a phone reads and continues them. Kept locally until a host starts, then moved (`client.rs:103-133`, `crates/openagents-terminal/src/app.rs:454-461`). | OpenAgents is ahead on sync. |
| Resume | `/resume` picker over the 10 most recent, `/resume N|ID`, `--resume [ID]` (`coder:bins/coder-terminal/src/history.rs:46`, `resume_screen.rs:57`). | `--continue` (last thread in this folder, `last.rs:1-67`), `--thread ID`, Ctrl+T thread list with open, new, archive (`app.rs:277-323`). | Parity. |
| Foreign agent sessions | Watches `~/.claude/projects` and `~/.codex/sessions`, turns them into threads, answers a phone prompt by resuming that agent's session (`coder:bins/coder-terminal/src/watch.rs:1-8`). | Not found. | Gap, later (see roadmap). |
| Who does the coding | The model itself, with Read, Write, Edit, Glob, Grep, Bash, TodoWrite, AskUserQuestion, Agent and more (`coder:crates/coder-tools/src/cc/mod.rs:75-93`), plus a `delegate` tool to Codex, Claude Code, Devin, Grok, or a Box (`delegate.rs:141-170`). | Coder via `coder::task::local` with the first allowed, signed-in agent (Codex, Claude Code, Grok Build; OpenCode and Devin opt-in) in its own worktree (`crates/coder/src/task/settings.rs:57-100`). | Different by design. The router plus Coder is OpenAgents' answer; no gap to close. |
| How a run shows | One row per tool call: marker, purpose, command, elapsed time (`coder:bins/coder-terminal/src/screens/transcript.rs:472-548`). A delegation is a frame with agent, model, clock, tokens, plan side panel (`transcript.rs:327-393`). | Read/search calls fold into one line counted by verb, each command or edit one line, at most 10 visible, Ctrl+O expands all (`crates/openagents-chat/src/tool_groups.rs:1-26`, `crates/coder-terminal/src/components/run.rs:74-104`). Live `step N · ≈X% done` from Jev (`app.rs:677-701`). Result lists files with +/- counts and the worktree (`run.rs:52-62`). | OpenAgents is calmer and shows progress; Coder shows the plan. Small gap: no plan or todo list in the run view. |
| Full-screen run and steering | Ctrl/Alt+1..9 or `/open N` opens a delegation full screen with reasoning, Markdown, a patch with its own gutter, and the plan; Enter steers it (`coder:bins/coder-terminal/src/delegation.rs:1293-1313`, `delegation_steer.rs:1-8`). `/children`, `/focus`, `/tile`, tmux `panes` (`tile.rs:1-12`). | Inline only. Answering Coder's question by typing is the only input to a running run (`app.rs:400-406`). A follow-up after a run continues it through the router. | Gap. |
| Run lifetime | A turn in flight is cancelled with two presses of Esc (`coder:bins/coder-terminal/src/session_input.rs:101-119`). | Esc stops the run; quitting leaves it running in its own `microcoder` process and reopening follows it from its first event (`app.rs:326-342,559-565`). The issue flow runs in the screen's process (`app.rs:510-519`). | OpenAgents is ahead on run lifetime, except the issue flow. |
| Engines and models | Lanes `free`, `flash`, `pro`, `mesh`, `local` (`coder:bins/coder-terminal/src/lane.rs:1-60`), `/endpoint`. The README's `/mode` is not in the catalog (README drift). | No picker: the router's typed choice and `coder.providers` decide (`docs/terminal/scope.md:158-159`). Welcome card lists ready agents (`app.rs:731-743`). | Deliberate. None. |
| Slash commands | About 90 in one catalog (`coder:crates/coder-contract/src/command.rs:120-930`). Several do less than their names: `/vim` sets an unread flag (`operator.rs:206-213`), `/theme` saves a name the renderer ignores, `/ssh` opens nothing (`coder:crates/coder-tools/src/remainder.rs:158-178`), `/search` looks in the wrong folder (`command.rs:563-588`). | 10 in a closed enum: `/new`, `/threads`, `/stop`, `/export`, `/settings`, `/connect`, `/plugins`, `/expand`, `/help`, `/quit`. Only an exact `/word` is a command (`crates/openagents-terminal/src/slash.rs:11-101`). | OpenAgents is ahead on honesty. Add commands only with the features below. |
| Keys | Ctrl+P plugins, Ctrl+G Gym, Ctrl+O mouse to terminal, Alt+M permission mode, Shift+Enter newline, Ctrl/Alt+1..9 delegations, Shift+arrow selection, Cmd/Ctrl+A/C/X/V (`coder:bins/coder-terminal/src/bindings.rs:126-185`, README 458-475). Remappable through `~/.openagents/keybindings`. | Enter, Alt+Enter or Ctrl+J, Esc, Ctrl+T, Ctrl+O, Ctrl+S, PageUp/PageDown, Up/Down, Ctrl+C twice, Ctrl+D, readline keys (`app.rs:230-275`, `crates/coder-terminal/src/keys.rs:30-120`). | Gap: text selection and copy keys. Remapping is not worth it yet. |
| Mouse and clipboard | Mouse capture, drag to select, copy through pbcopy, wl-copy, xclip, or OSC 52; clicking a file reference opens the viewer (`coder:bins/coder-terminal/src/mouse.rs:12-34`, `file_viewer.rs:1-25`). | Not found: the terminal's own selection only. | Gap. |
| Layout | Single transcript plus composer; split views live outside the app (compositor tiles, tmux panes). | Single column, framed composer with two status rails, centered list overlay, 5,000-line scrollback (`crates/openagents-terminal/src/draw.rs:23-124`, `crates/coder-terminal/src/scrollback.rs:19`). | Parity in the app itself. |
| Markdown | pulldown-cmark into a core document (`coder:crates/coder-ui-core/markdown.rs:1-30`). | pulldown-cmark: headings, code, quotes, lists, tables, task lists (`crates/coder-terminal/src/markdown.rs:1-7`). | Parity. |
| Code highlighting | tree-sitter with about 32 grammars in `coder-syntax`, used for code fences and the file viewer (`coder:crates/coder-terminal/Cargo.toml:19`). | None: a code block draws as one style (`crates/coder-terminal/src/markdown.rs:188-203`). tree-sitter highlighting already exists in this repo for Rust Native (`crates/rust-native/src/syntax.rs:180`, `crates/rust-native/Cargo.toml:27-38`). | Gap, cheap to close. |
| Diffs | Patch gutter in the delegation full screen; `/diff` prints a diff stat. No inline diff for the main chat's edits (`coder:bins/coder-terminal/src/file_tools.rs:95-105`). | Per-file +/- counts and the worktree path only (`run.rs:52-62`, `crates/openagents-chat/src/coder_events.rs:625`). | Gap. |
| Approvals | Modes bypass (the default), default, accept_edits, plan; Alt+M cycles; `/yes`; rule files; pre-tool hooks; `/rewind` from write receipts (`coder:bins/coder-terminal/src/permission.rs:1-6`, `coder:crates/coder-tools/src/rules.rs:1-13`, `coder:docs/terminal-hooks.md`). | Approve everything, no prompt (#10104; `crates/coder/src/task/settings.rs:11-13`). Coder works in its own worktree, which already contains its changes. | Deliberate. See "not worth porting". |
| File and repo context | `@` mentions, up to 32 glob matches in the folder (`coder:bins/coder-terminal/src/craft.rs:20-33`). Images pasted or dropped, PNG/JPEG/GIF/WebP up to 4 MB, as `[Image #N]` (`image.rs:1-6`). `/add-dir`, `/files`, memory. | Project is the Git checkout of the folder (`app.rs:727-730`). `@` mentions and attachments not found (out of v1, `scope.md:163`). | Gap. |
| Prompt history and search | Prompts kept across sessions in `~/.openagents/coder-history` (`coder:crates/coder-terminal/src/composer/history.rs:17-28`); `/history TEXT` searches them; `/search`, `/sessions`. | Up/Down history in memory only (`crates/coder-terminal/src/editor.rs:48-58`). Thread list has no filter, though `chat_list::search` exists (`crates/openagents-chat-app/src/chat_list.rs:50`). | Gap, cheap to close. |
| Prompt queue and schedules | `/queue`, `/loop 5m PROMPT`, a `schedule_prompt` tool (`coder:bins/coder-terminal/src/schedule.rs:1-10`). | Sending while a reply streams is refused; the draft goes back (`app.rs:392-397`). | Small gap: queue one message. Scheduling belongs to the host, not the screen. |
| Settings | Many files under `~/.openagents/`; `/config`, `/theme`, `/keybindings`. | `/settings` shows the file path and every key (`crates/openagents-cli/src/screen.rs:420-437`). No editing in the screen. | Small gap: editing a few settings in place. |
| Theme | Amber, four intensities, truecolor, 256 colors, `NO_COLOR`, ASCII glyphs (`coder:crates/coder-terminal/src/theme.rs:10-24`). | White, four intensities on near-black, truecolor, 256 colors, `NO_COLOR` (`crates/coder-terminal/src/ladder.rs:87-131`). | Parity. OpenAgents has no ASCII glyph mode (not found). |
| Remote and multi-device | `serve` on `127.0.0.1:7777` with `--attach` and phone admission by pairing code (`coder:bins/coder-terminal/src/admission.rs:1-12`); tailnet peers (`tailnet.rs:1-10`); fleet `/cloud`; hosted Box. | Threads sync through the host; `/connect` pairs a phone with a text QR (`crates/openagents-terminal/src/screen.rs:497-555`); Ctrl+S installs the host as a service (`crates/openagents-cli/src/screen.rs:401-520`). | OpenAgents is ahead for phone and desktop. Gap: attaching this screen to another computer's host. |
| Offline and reconnect | `--offline` keeps typing local; reconnect with backoff 1 to 15 s, ping after 30 s, drop after 90 s; interrupted turns resume (`coder:bins/coder-terminal/src/outage.rs:1-4`, README 483-504). | Not found beyond the client's timeouts (120 s send, `client.rs:68`). The in-process backend still needs the relay. | Gap. |
| Export and usage records | `/export` ATIF, `/publish`; a delegated turn's tokens and duration reported without task text (`coder:bins/coder-terminal/src/turn_report.rs:1-15`); no analytics from the terminal (`coder:docs/analytics.md`). | `/export` ATIF with Coder tasks, validated (`crates/openagents-terminal/src/screen.rs:455-494`). Turns carry `surface: "terminal"` (`crates/openagents-chat/src/router.rs:98-139`). Analytics not found. | Parity. |
| Plugins | WebAssembly plugins, digest-pinned and sandboxed, run by the model through a `capability` tool; Ctrl+P screen to enable them; about 29 packages (`coder:plugins/README.md`, `capability.rs:1-4`). | `/plugins` lists published plugins read-only; ask about one in the chat (`crates/openagents-cli/src/screen.rs:409-418,554-580`). | Gap: running a plugin from the screen. |
| Startup speed | No benchmark found; 20 ms frame tick only while busy (`coder:bins/coder-terminal/src/main.rs:225`). | No benchmark found; welcome card waits up to 5 s for Coder readiness (`crates/openagents-terminal/src/screen.rs:33-34`). | Neither measures it. |
| Platforms | macOS arm64 and x86_64, Linux gnu and musl on both, Windows x86_64; child screens Unix only (`coder:ops/release-terminal.sh:162-168`). | Same build targets; Windows built but not run; rc.2 install checked on macOS arm64 only (`docs/release/terminal.md:40-61,180-189`). | Gap: a Windows run. |
| Tests | Golden snapshots, scripted scenarios, state machine property tests, live-screen scripts. | Pure app tests, text snapshots, a PTY test through `coder-vt`, an ignored live test (`crates/openagents-terminal/tests/pty.rs`). | Parity for its size. Acceptance mode from scope phase 2 not found. |
| Other | Gym, earning mesh, autopilot with away mode and recap, memory, browser and window tools on CoderOS, Devin handoff. | Router cards point to the app (`app.rs:630-674`). | Mostly not worth porting (below). |

## Roadmap

Sizes as in the scope: S under a day, M one to three days, L about a week.
No item adds step or time budgets to Coder runs, usage limits, or routing by
matching words; the router stays the only judge of what a message is.

**Phase 1: read and reuse what is on screen.** Highest value per effort, no
new protocol, all in the screen.

1. **Syntax highlighting in code blocks. M.** `crates/coder-terminal`
   (markdown). Every reply with code benefits. Move the tree-sitter
   highlighter from `crates/rust-native/src/syntax.rs` into a small shared
   crate and map its classes onto the four intensities plus bold and italic;
   no new colors.
2. **Persistent prompt history. S.** `crates/coder-terminal` (editor) and
   `crates/openagents-terminal`. A file in the chat home, prompts only,
   capped, written atomically like `last.rs`; scratch mode keeps its own.
3. **Search in the thread list. S.** `crates/openagents-terminal`. Typing in
   the overlay filters with the existing `chat_list::search`.
4. **Copy a reply or code block. S.** `crates/openagents-terminal`. A key
   that copies the last reply or the code block under the cursor through
   OSC 52, which works over SSH. Full mouse selection waits for phase 4.

**Phase 2: see what Coder changed.** Needs data from Coder, so after
phase 1.

5. **Diff view of a run's result. M.** `crates/openagents-chat`
   (`coder_events` carries each file's patch from the worktree) and
   `crates/coder-terminal` (run component, using phase 1's highlighter for
   `diff`). Ctrl+O on a result shows the patch per file. This is the main
   thing people need to trust a run.
6. **The run's plan in the run view. S.** `crates/coder-terminal`, if Coder's
   events carry a plan or todo list (not verified; check `coder_events`
   first).

**Phase 3: context in.** Larger, crosses into the chat service.

7. **`@` file mentions. M.** `crates/coder-terminal` (composer completion)
   and `crates/openagents-terminal`. Completes paths inside the project only
   and sends them as typed references with the message, never as words the
   screen interprets. Depends on the chat client accepting references
   (`crates/openagents-chat`).
8. **Image attachments. L.** `crates/openagents-chat` and the chat worker,
   then the composer. Only if the router and the worker accept images from
   the phone or desktop already; support for that was not verified.
9. **Queue one message while a reply streams. S.**
   `crates/openagents-terminal`.

**Phase 4: watch and steer runs.** Builds on phases 2 and 3.

10. **Full-screen run view. L.** `crates/openagents-terminal` and
    `crates/coder-terminal`. One run at a time: steps, output, patch, and a
    composer whose text goes to the run as steering, if `coder::task`
    accepts input mid-run (it accepts answers to questions today).
11. **Move the issue flow under the host. L.** `crates/coder` and
    `crates/coder-host` (already phase 3 of the scope). Then an issue run
    survives closing the screen like any other run.
12. **Mouse selection and clickable file references. M.**
    `crates/openagents-terminal`, opening a read-only file view with the
    highlighter.

**Phase 5: reach and resilience.**

13. **Reconnect and a clear offline state. M.** `crates/openagents-chat`
    (client) and the status rail. Say when the relay is unreachable, retry
    with backoff, and resume the streaming reply.
14. **Run a plugin from the screen. M.** `crates/openagents-terminal` and
    `crates/openagents-cli` (the `Extras` trait). A picker over `/plugins`
    that runs one, already listed in the scope's phase 3.
15. **Edit a few settings in place. S.** `crates/openagents-terminal`:
    which coding agents are allowed and when Coder starts.
16. **Open another computer's threads. L.** `crates/openagents-chat` over
    NIP-HOST: the terminal on a laptop following the desktop's host, as the
    phone does.
17. **Import Claude Code and Codex sessions as threads. M.** Later, and in
    the host (`crates/coder-host`), not the screen, so every app sees them.
18. **Run it on Windows once and fix what breaks. M.**

### Not worth porting

- **The long command catalog.** About 90 commands, several of which do
  nothing they promise. Add a command only with a feature that needs it.
- **Permission modes, rule files, `/yes`, `/rewind`.** The owner chose
  approve everything (#10104), and Coder works in its own worktree, so a
  change is already reviewable and discardable there.
- **Lanes, `/endpoint`, and a model picker.** The router's typed choice
  decides the engine.
- **The `coder-serve` websocket, the shell tool loop, and `coder-ui-core`.**
  OpenAgents already has the router, Coder local runs, and its own component
  set; the scope reached the same conclusion.
- **`/cloud` fleet runs, Box, Gym, earning mesh, tailnet peers.** Separate
  products with their own homes; not part of a chat screen.
- **Autopilot children, tiles, tmux panes.** One run at a time with a full
  view (phase 4) covers the need; multiple concurrent runs can wait for
  demand.
- **Key remapping and themes.** Low value; the palette is shared with the
  desktop app and the website.
- **Delegated-turn usage reports from the screen.** If usage records are
  wanted, they belong in Coder or the host, where runs live, so every app
  shares them; the screen should not add its own channel.
