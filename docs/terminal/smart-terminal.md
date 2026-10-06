# Smart terminal: brainstorm, specification, and roadmap

Status: proposal, October 5, 2026, with the sheet and its input line
implemented under the [design principles](design-principles.md), which are
binding. It defines the smart terminal, decides its shape, and orders the
work. Where it records
a decision, the owner can overturn it; the open questions are
[at the end](#open-questions-for-the-owner).

Owner direction, October 5: ship the same terminal in the desktop Grid and
as a separately installable app today, then connect it to Everglade's Agent
Studio in the next pass. The [workbench roadmap](workbench-roadmap.md) records
the transcript review, product integration, paid cloud-compute option, and
milestone acceptance. It supersedes this page's original phase order. This
page remains the detailed terminal specification.

The owner's ask (2026-10-05): "That's our terminal UI product and not a real
terminal. I want us to fix that while combining the ideas: let's define a
'smart terminal' that can do both regular shell commands and also process
natural-language requests as threads etc., all the same stuff we'd envisioned
for our terminal product, and the multiplexer vision spec'd out in
~/work/coder." A follow-up asked for a close comparison with Superlogical,
the multiplexer from the Ghostty team, and for that parallel to take
priority.

Paths are relative to this repository unless they start with `~/work/coder`,
which is the earlier private Coder repository. That repository is reference
material only: this page describes its ideas, and anything taken from it is
reimplemented here, with the commit message saying so. No code, prompt,
endpoint, or secret is copied.

## Contents

- [Summary](#summary)
- [Today: three terminals, none smart](#today-three-terminals-none-smart)
- [Superlogical: the model this follows](#superlogical-the-model-this-follows)
- [Definition and vocabulary](#definition-and-vocabulary)
- [Other prior art](#other-prior-art)
- [Specification](#specification)
- [Architecture](#architecture)
- [What exists and what is missing](#what-exists-and-what-is-missing)
- [Roadmap](#roadmap)
- [Open questions for the owner](#open-questions-for-the-owner)

## Summary

A *smart terminal* is a real terminal emulator and multiplexer whose input
line takes both shell commands and natural-language requests. A command runs
in your shell and becomes a *block*: its command line, output, exit status,
timing, and working directory as one record you can copy, search, collapse,
rerun, share, or attach to a thread. A request opens a *thread*: the chat
router and Coder answer it, propose commands as blocks you approve, run them
in your shell with visible output, and keep going. Terminals live on a host,
outlive every window and device, and attach from the desktop app, Verse, the
phone, the browser, and a plain TTY.

The decisions this page takes:

1. **The session is the unit of work, not the window.** This is
   Superlogical's thesis, and the smart terminal adopts it whole: a host owns
   every terminal, closing a pane detaches, and any surface reattaches. NIP-TERM
   already has these semantics; the smart terminal makes them the default
   path, not the remote special case.
2. **Superlogical's terminal half, reimplemented in Rust over NIP-TERM.** The
   host fans raw PTY bytes out to every client before it parses them, keeps
   an authoritative `coder-vt` emulator per terminal, owns terminal side
   effects, and serves a join from a parsed-state snapshot (screen first,
   history after). The snapshot format follows libghostty's ordering, not its
   bytes; see [Interop](#interop-and-dependence).
3. **Smart is the half Superlogical has announced but not shown.** Blocks,
   threads, typed decisions, and agents attached to panes are structured
   records in the session, which is what Superlogical says comes after the
   multiplexer.
4. **The shell keeps its line editor.** A shell-integration hook hands the
   finished line to the terminal at Enter; zsh, bash, and fish keybindings,
   vi mode, and `fzf` keep working. The terminal never replaces the shell's
   editor.
5. **Shell or request is decided on this computer, visibly, before Enter.**
   An explicit mode or prefix wins; otherwise the shell's own command table
   decides structurally, and only a genuinely ambiguous line asks a local
   decision model. No shell line leaves the machine to be classified. What a
   request means stays the chat router's decision, as today.
6. **A proposed command never runs in your shell without your key.** Coder
   runs keep the approve-everything rule of
   [#10104](https://github.com/OpenAgentsInc/openagents/issues/10104),
   because they work in their own worktree. Commands that land in your live
   shell are blocks you run with Enter, under the existing `Permit`.
7. **One engine, many surfaces.** A new portable crate, `terminal-core`, is
   extracted from `crates/verse/src/terminal`; a new `terminal-gfx` holds the
   wgpu grid renderer. The standalone window and Verse's overlay run the same
   code. OpenAgents Terminal's chat becomes the thread view inside it.
8. **The first release has two mounts:** blocks and requests in the Grid's
   Verse overlay and a separately installable window sharing that code,
   with the thread initially running `openagents terminal --thread ID`.
   Everglade's workshop then opens the same app with studio context.

## Today: three terminals, none smart

The repository has three things called a terminal, and none is both a
terminal and smart.

| Product | What it is | What it lacks |
| --- | --- | --- |
| OpenAgents Terminal (`crates/openagents-terminal`, `openagents terminal`) | A full-screen ratatui chat over the shared client `openagents_chat::client`. The router answers each message; when it judges a message is coding work, Coder runs in a worktree on this computer and streams in. Modules: `app` (state and keys), `draw`, `screen` (input loop), `slash` (closed command list), `picker` (thread picker), `rows`, `rail` (Coder runs under the composer), `view` (run and file views), `last`, `prompts`. See [README](README.md), [scope](scope.md), and the [gap analysis](2026-10-02-coder-terminal-gap-analysis.md). | It is not a terminal emulator. It cannot run `vim`, `htop`, or `ssh`, and it has no shell. The scope listed "an embedded PTY/shell pane" as out of v1. |
| Coder (`crates/coder`, drawn with `crates/coder-terminal`) | The older Coder agent shell: a composer, scrollback, and a turn (`coder::turn::run`) that classifies, generates, and may run a plan of shell commands under a `Permit` (`crates/coder/src/permit.rs`, `crates/coder/src/shell.rs`), with delegation to Claude Code and Codex. | Also not an emulator. Its commands run through `supervise` in a bounded `sh -c`, not in your interactive shell. |
| The Verse terminal overlay (`crates/verse/src/terminal`, `T` in Verse) | A real terminal: PTYs through an in-process `coder-pty` host, `coder-vt` emulation, split panes and tabs with a `Ctrl+B` prefix, selection, copy mode and search, mouse reporting, OSC 8 links, OSC 52 writes, on-demand glyphs, a per-frame output budget, and a control socket (`crates/verse/src/terminal_control.rs`, `openagents verse terminal`). See the [in-world terminal](../verse/in-world-terminal.md) and its [performance receipt](../verse/verification/2026-10-05-terminal-performance/README.md). | Nothing smart: no blocks, no requests, no threads. Its PTYs are local and die with Verse; it is not yet a NIP-TERM client. It exists only inside Verse. |

Beside them sit the remote pieces: NIP-TERM host PTYs with replay and gaps
(`nips/openagents/NIP-TERM.md`, `crates/coder-pty`), the resident host's
binding that rechecks rights per message (`crates/coder-host/src/serve/terminal.rs`),
the seven host rights (`crates/coder-access/src/rights.rs`, where `Terminal`
opens and drives terminals and `Observe` reads), the client session
(`crates/coder-computers/src/terminal/session.rs`), the phone terminal screen
(`crates/coder-computers/src/terminal/screen.rs` and `project.rs`), and
`openagents computer shell HOST` (`crates/openagents-cli/src/terminal.rs`).

The smart terminal is these pieces put together, with the missing middle
built.

## Superlogical: the model this follows

Superlogical is Mitchell Hashimoto's company and product, announced July 29,
2026, and unreleased as of the reference notes: a terminal multiplexer built
on libghostty, with web, macOS, and iOS clients. The earlier Coder repository
recorded what is public about it and compared Coder to it:

- `~/work/coder/docs/os/superlogical.md`: the public architecture as of
  September 16, 2026, with each claim marked as officially announced,
  verified in libghostty's source, or attributed to the author's posts.
- `~/work/coder/docs/os/superlogical-analysis.md`: Coder's position against
  that model, what Coder adopted (issues #855 through #865 in that
  repository), and what it declined.
- `~/work/coder/docs/os/README.md` and the "Superlogical" and "Multiplexer"
  rows of `~/work/coder/docs/GLOSSARY.md`: the summary and vocabulary.
- `~/work/coder/docs/game/2026-09-16-terminal-rendering-audit.md`: the
  measured close condition the comparison set for a host-side emulator per
  terminal.

Everything below about Superlogical comes from those notes. Superlogical's
code is not public, its protocol is unpublished, and libghostty's snapshot
format is marked unstable, so treat each detail as the notes' reading on
September 16, 2026, not as a specification.

### Its model

| Part | What the notes record | Evidence in the notes |
| --- | --- | --- |
| Unit of work | The durable session, not the window. A session holds several terminal blocks, survives a client closing, resumes on another device, and shares live. The stated sequence is: build the multiplexer, make its contents composable, then make it safe to operate in production, with sessions that software can drive while humans watch and control. | Announced |
| Server and clients | A server (Go, attributed) owns the PTYs; native Apple clients (Swift, attributed) and a web client attach. Low-level parts are Zig over libghostty. | Announced platforms; stack attributed |
| Output path | The server tees raw PTY bytes to every client before its own parse finishes. The server parses authoritatively; each client parses the same bytes with its own emulator. This is parallel parsing, not a redraw stream like tmux's outer-terminal path (tmux control mode already sends raw pane output to capable clients, as iTerm2 uses). | Attributed |
| Join | A new client restores a parsed-state snapshot instead of replaying history. libghostty's Snapshot v1 orders records `TERMINAL`, screen and page records, `CONTINUATION`, then `READY`, then history pages newest first, then `FINISH`. `READY` means the client can draw and resume parsing. `CONTINUATION` carries an unfinished escape sequence or partial UTF-8. Each record is `tag: u16`, `payload_len: u32`, `crc32c: u32`, and the payload, little-endian. History metadata arrives early so a scrollbar is sized before pages arrive. The snapshot/live boundary is the outer transport's job. | Verified in libghostty |
| Rendering | libghostty separates a short locked phase that captures render state from a longer unlocked draw, with frame and per-row dirty state. | Verified in libghostty |
| Memory | Caller-driven incremental scrollback compression when idle. | Verified in libghostty |
| Side effects | Query replies, bells, titles, and clipboard requests surface as callbacks the embedder handles; with replicated parsing, not every replica may answer a query. The notes name "which side effects belong to the server" as the question to watch. | Verified API; policy unpublished |
| Viewer state | Scroll position, selection, and font size are viewer-local; concurrent input and resize policy is unpublished. | Notes' decomposition |
| Networking | The multiplexer owns SSH, and Tailscale enrollment and discovery are planned. | Attributed |
| Durability | Client-independent execution and recoverable presentation. A snapshot is not a process checkpoint; nothing establishes host-failure recovery. | Notes' analysis |

### What Coder took from it

The Coder comparison concluded that Coder already had the session half (a
headless writer every client attaches to, a versioned attach protocol, and
raw PTY bytes with a sequence number to every client) and lacked the
terminal half. It then built, in that repository:

- **A writer-side terminal per block** (#855): one authoritative emulator per
  published terminal, all on one emulation thread, which answers the
  program's device queries itself, puts a title on the terminal's record, and
  turns a bell or a refused clipboard write into a notice. Clients answer
  nothing.
- **Snapshot on join** (#856) and **history pages** (#857): the `READY`
  prefix first, then history newest first up to 2,000 rows or 1 MiB, older
  pages on request, and `next_seq` as the live boundary.
- **Shells as session members** (#858, #865): a pane is a window on a
  terminal the writer runs; closing the pane detaches, and closing the shell
  is a separate verb that asks first while a program runs.
- **Per-device shares** (#859): each device is granted submit, drive, both,
  or neither, narrowed live.
- **One driver per terminal** (#860): the first client to type at an undriven
  terminal drives; `take` and `release` move the role; other clients' keys
  and resizes are refused with `not_driver`. The driver's size sets the PTY;
  viewers draw at that size and pan, anchored to the cursor; a title reads
  `80×24 · driven by mac`.
- **An emulator in every client** (#861): desktop, phones, and the browser.

It declined SSH inside the multiplexer, a Go server or Swift clients, a
vendor relay as the default path, and any promise of process migration.
Its measurements with ten streaming terminals stayed under 0.6 ms at the
median and 2.1 ms at the 99th percentile for emulation lag and key-to-PTY
time on a Mac and on `coderos-4080`.

### What the smart terminal adopts directly

| Superlogical idea | Adopted as | Where it lands here |
| --- | --- | --- |
| The session is the unit; closing a client ends nothing | A host-side *session*: a named set of terminals, threads, and a layout, owned by the resident host. Every pane is an attachment. | `coder-pty` already keeps PTYs past detach (until idle expiry); a session record is new, in `coder-host`. |
| Raw bytes fan out before the parse | Kept as is: NIP-TERM already sends each output frame with a sequence number to every attachment, and each client parses with `coder-vt`. | `crates/coder-pty`, `nips/openagents/NIP-TERM.md` |
| An authoritative parse on the server | New: the host feeds every terminal's output through a `coder-vt` emulator of its own before or while it fans the bytes out. | `crates/coder-pty` host, behind a feature so the client half stays light |
| Side effects have one owner | The host's emulator answers device and cursor queries at the PTY's input; clients stop sending `take_replies()` output. Titles and the working directory go on the terminal's record. A bell becomes a notice. OSC 52 reaches only the typist's clipboard. | Implemented for remote terminals (#10653): `coder-host` runs `coder_vt::Authority` per terminal and serves NIP-TERM's effects feature, and the Coder mobile screen stops answering when it names the feature. A clipboard write goes only to the typist (#10675). The in-process desktop host keeps the base profile, with one device per terminal. |
| Snapshot on join, screen first, history after, parser continuation | A NIP-TERM `snapshot` frame from the host's emulator: the visible screen, modes, cursor, and parser continuation first (`ready`), then history pages newest first, with the live sequence boundary. Replaces "replay the ring and hope it covers the screen." | `coder-vt` gains serialize and restore; `coder-pty` serves it. Framing reuses libghostty's record shape (tag, length, CRC32C), reimplemented. |
| Per-row dirty state; short locked capture | `coder-vt` per-row dirty marks; the renderer rebuilds only changed rows. The Verse overlay's per-frame output budget (3 ms and 256 KiB a pane) stays. | `crates/coder-vt`, `terminal-gfx` |
| One driver, viewers pan | One *typist* per terminal, enforced by the host, with the same take, release, size, and pan rules. | `coder-pty`, `coder-host`, NIP-TERM extension |
| Per-device shares | A terminal share grant: one terminal, one grantee, watch or drive, a first readable sequence, an expiry. | Already specified in the [in-world terminal](../verse/in-world-terminal.md#grants); this page adopts it. |
| Idle scrollback compression | Later, if host memory under many sessions says so. | `coder-vt` |
| Durability stated, not overpromised | The smart terminal says what survives: a client closing (yes), a reattach (yes, from the snapshot), the host restarting (no; panes show `lost`, as NIP-TERM's generation already does). | Docs and pane chrome |

### Where the smart terminal differs, and why

| Difference | Superlogical (per the notes) | Smart terminal | Why |
| --- | --- | --- | --- |
| Natural-language requests | Not described. | One input line takes commands and requests; a request opens a thread through the chat router. | The owner's ask. The router and Coder already exist; the terminal is their missing surface. |
| Structured session contents | Stated as a later goal ("composable contents", structured data and actions). | Blocks are structured records from day one (OSC 133), and threads, proposals, and Coder runs are typed records beside terminals. | This repository already has typed decisions (Jev), the `Permit`, effect classes on commands (the router's CLI route), and ATIF traces. |
| Agents | Software can drive a session while humans watch (stated goal). | Agents attach to panes as typists under the same one-typist rule, visibly, and only when you hand them the keys. Coder runs appear as panes. | Same goal, made concrete with this repository's rights and permits. |
| Transport | Unpublished; Tailscale planned. | NIP-TERM over NIP-REACH direct channels, sealed relay artifacts, loopback, or a tailnet WebSocket; rights from NIP-HOST grants rechecked per message. | This repository's hosts, phones, and Verse already speak it; no vendor relay sees content. |
| SSH | The multiplexer owns SSH. | `coder-ssh` installs a host at the far end once, after which remote shells are NIP-TERM terminals rather than an `ssh` program nested in a pane. A plain `ssh` in a pane still works as any program does. | Matches the Coder comparison's choice: the host is the endpoint. |
| Emulator | libghostty (Zig, C API). | `coder-vt` (Rust, `vte`). | AGENTS.md keeps product code in Rust; `coder-vt` already runs on desktop, phones, and the web build path. |
| Surfaces | Web, macOS, iOS. | The standalone window, Verse's overlay and in-world screens, the phone, the browser, and a plain TTY. | Verse is a product surface here; in-world screens are a terminal surface no one else has. |
| Look | Unspecified. | The white ladder of `coder_ui::theme::Intensity` on near-black. | One palette across the app, the website, and the terminal. |

### Interop and dependence

- **No wire compatibility is possible today.** Superlogical's protocol is
  unpublished. The smart terminal does not wait for it and depends on
  nothing from it.
- **Shape compatibility is the goal.** Sessions, raw-byte fan-out, a parsed
  snapshot on join, one typist, and per-device shares match its model, so an
  adapter is a translation, not a redesign, if a public protocol appears.
- **libghostty snapshots are an optional later codec.** Decoding
  libghostty's Snapshot v1 in Rust would mean a second emulator's page and
  cell model; linking libghostty would add a Zig-built C library. Neither is
  planned. If Superlogical publishes a protocol that carries libghostty
  snapshots, revisit with a spike that measures the cost.
- **Reassessed on 2026-10-06** (#10696): neither a Superlogical protocol nor
  a versioned libghostty snapshot format is public, so this stands. The
  dated report is in
  [Optional terminal research](2026-10-06-optional-research.md#superlogical-and-libghostty-interop-10696).
- **Upstream libghostty work is prior art for `coder-vt`.** Its continuation
  record, history-page ordering, generation tracking that drops inapplicable
  pages, and two-phase render state are the designs to reimplement.

## Definition and vocabulary

A *smart terminal* is a terminal emulator and multiplexer, over terminals a
host owns, whose input line takes shell commands and natural-language
requests, records each command as a block, and answers each request as a
thread that can propose and run commands in the same terminal.

Words on this page, each with one meaning:

| Word | Means | Not |
| --- | --- | --- |
| Terminal | A NIP-TERM terminal: a program on a host PTY, with a sequence-numbered output stream. Superlogical and the earlier Coder repository call this a *terminal block* or *block*. | A window or a pane |
| Session | A host-side record: a name, its terminals, its threads, and a default layout. It outlives every client. | A chat session |
| Pane | A rectangle in a client's layout that shows one terminal or one thread. Closing it detaches. | The terminal itself |
| Block | One command run in a shell, delimited by OSC 133 marks: command line, output range, exit status, start and end time, and working directory. | The earlier repository's "block" (that is a terminal here) |
| Request | A line the terminal sends to the chat router instead of the shell. | A command |
| Thread | The existing chat thread (`openagents_chat::client`), opened by a request, persisted in the host's thread store, shown in the desktop app and on the phone. | A terminal |
| Proposal | A command a thread suggests, shown as a pending block with its reason and effect class, run only by your key. | A command that runs itself |
| Typist | The one attachment whose keys and size a terminal takes. Earlier called the driver. | A viewer |
| Share | A grant that lets another device watch or drive one terminal. | A device enrollment |
| Surface | A client that draws terminals and threads: the window, Verse, phone, web, or TTY. | A session |

## Other prior art

Short comparisons from general knowledge, not from source; details may be
out of date or wrong.

| Product | Idea | Take or leave |
| --- | --- | --- |
| Warp | Commands as blocks; an input editor the terminal owns; an AI mode for natural language, which (as far as known) can detect natural language automatically; a GPU renderer in Rust. | Take blocks and the request line. Leave replacing the shell's line editor, because it breaks zsh widgets, vi mode, and `fzf`. Leave any account requirement for local use. |
| Fig, now Amazon Q Developer CLI | Completion specs for thousands of commands; natural language to a shell command (`q translate`, as far as known). | Take the idea of command specs for proposals and effect classes later. Leave a separate overlay process. |
| Wave Terminal | Terminals beside other widgets (files, web, AI chat) in one tiled layout. | Take threads as panes beside terminals. |
| tmux and Zellij | Sessions that outlive clients, a prefix key or modes, saved layouts (Zellij's are files), and tmux control mode for native clients. | Take sessions, the prefix, and layouts. The Verse overlay already has the tmux-like prefix. |
| iTerm2 shell integration | OSC 133 prompt marks (from FinalTerm), current directory reporting, marks to jump between commands, command history per host. | Take OSC 133 and OSC 7 as the block mechanism; inject the hook as iTerm2 and Ghostty do. VS Code's terminal uses a similar private family (OSC 633) that also carries the command line. |
| Ghostty and Kitty | Fast native rendering, automatic shell integration, the Kitty keyboard protocol (progressive enhancement with `CSI u`), and Kitty's graphics protocol and remote control. | Take automatic injection and, later, the Kitty keyboard protocol in `coder-vt`. Graphics are out of scope at first. |

## Specification

### The sheet and its input line

The [design principles](design-principles.md) bind this section. The
terminal is one fixed sheet, 1200 by 800 points (3:2), that nobody resizes;
the Grid draws the same sheet anchored at the center of the screen. From top
to bottom it has a status area, the transcript with a scroll bar that is
always drawn, one input line, and a key strip. Nothing floats over these
regions, nothing moves, and every character is ASCII in the four whites.

The input line belongs to the terminal, not to the shell's line editor. The
shell runs underneath with its integration hook, and its blocks (a command,
its output, and its exit status) appear in the transcript beside questions
and answers, each as a plain block. While a command runs, keys go to it, and
a full-screen program draws in the transcript region.

#### How Enter decides

One caret, no prefix, and no mode key. At ENTER the terminal decides on this
computer whether the line is a command or a question (`terminal_core::route`)
and stops at the first rule that applies:

1. **Structure.** A path (`./run.sh`, `~/bin/x`, `/usr/bin/env`) or an
   assignment (`FOO=1 make`) is a command. A line whose quotes do not
   balance, such as the apostrophe in "what's failing here", is a question.
2. **The shell's table.** The hook reports the shell's `PATH` and its alias
   and function names when they change; builtins and reserved words are
   known. A first word that resolves nowhere is a question, unless the line
   is built like a command (flags, paths, operators, or quotes), or is one
   word, which is more likely a typo than a question.
3. **Ambiguous lines.** The first word resolves, but the line could be prose
   (`make it faster`, `find all TODOs in src`). A local score weighs shell
   syntax, flags, and paths against question words, a final `?`, and
   pronouns and determiners. The label shows the result with a `?` while
   only the score decided. No local decision model answers this rule yet;
   Lev, Laya, or Kev can replace the score here later, and no shell line
   ever leaves the computer to be classified.

When the shell does not find a command, the terminal offers the closest
names from that shell's own table, builtins, and `PATH` executables, at
most three of the equally close ones (`crates/terminal-core/src/correct.rs`).
F7 on the sheet, or Ctrl+B then t in the panes, types the corrected line
without pressing Enter, and again for the next choice. Nothing runs until
you press Enter, and what the mistyped command already did stays done.

Ctrl+B then f in the panes searches the pane's kept blocks by command and
output, with `status:ok`, `status:fail`, `status:running`, `status:N`, and
`dir:TEXT` filters (`crates/terminal-core/src/search.rs`). Enter or Down
selects the next older match, Up the newer one, and the block actions (y, d,
r) then act on it; search itself runs and attaches nothing. It names what it
could not read: blocks the pane no longer keeps, output cut by scrollback or
the 16 KB bound, and running blocks. Host block-journal pages are not read
yet; the search covers the blocks the pane holds.

The label before the input line (`SHELL >` or `ASK   >`) updates as you
type, so ENTER always does what it shows. What a question means stays the
chat router's decision (`chat-router-v1`,
[chat router](../coder/design/2026-09-28-chat-router.md)).

#### Making mistakes cheap

- **F5** runs the input line as a command whatever its label says; **F6**
  sends it as a question. UP recalls an earlier line, so a misrouted line is
  one UP and one function key away.
- After a command the shell did not find (exit 127), the status area says
  so, and F6 on an empty line asks OpenAgents about it.
- The terminal can undo a routing decision, never a command's effect.

#### Questions, answers, and proposals

A question goes to OpenAgents at ENTER with no preview: its text, the shell's
directory, and, when the last command failed, that command's scrubbed output.
The status area's `CONTEXT` row says what goes with the next question, and F2
attaches or detaches it. A question asked while another runs queues; the
status area counts the queue, and nothing blocks the input line.

How to answer travels in the system instructions
(`basic_coder::INSTRUCTIONS_TERMINAL`), never in the visible turn: plain
ASCII, no Markdown, and at most one typed command plan on the reply's last
line. The helper removes the plan from the answer's text, the terminal
shows the answer as plain text, and the plan becomes one line:

```text
PROPOSED: cargo test   [ENTER] confirm  [ESC] reject
```

ENTER on an empty input line confirms it, and a command that may change
files takes a second ENTER; ESC rejects it. A confirmed command runs in the
shell as its own block, and its output returns to the same thread, whose
answer follows in the transcript. Panes, tabs, and the thread view remain
available behind F8; the default view never splits by itself.

### Blocks

#### Shell integration

The terminal injects a small hook into zsh, bash, and fish when it starts a
shell, the way Ghostty and iTerm2 do (for zsh, a `ZDOTDIR` that sources your
own files first; for bash, an `--rcfile` that sources your login profile or
`~/.bashrc` first, in `crates/terminal-core/src/bash.rs`). bash hooks need
bash 4.4 or later; an older bash, such as the 3.2 in macOS, starts without
them. bash reports no line as you type, so Enter always reaches it and a
line starting with `# ` is the explicit request. fish sources its hooks
through `--init-command` after your configuration
(`crates/terminal-core/src/fish.rs`, fish 3.3 or later); they bind only
Enter, and only when you have not bound it yourself. The host injects it for
NIP-TERM shells too, since
`coder-pty` starts them. The hook emits:

- OSC 133 `A` (prompt start), `B` (prompt end, input start), `C` (command
  start, output follows), and `D;N` (command finished with exit status `N`).
- OSC 7 with the working directory at each prompt.
- A private OSC with the command line as the shell received it (bounded,
  escaped), the command table at each prompt (alias, function, and builtin
  names, bounded), and the accept-line request and reply described above.

`coder-vt` parses these today as unknown operating-system commands and
ignores them (`osc_dispatch` in `crates/coder-vt/src/lib.rs` handles only
OSC 0, 2, 8, and 52). It gains them as marks on absolute line numbers, the
same line addressing `crates/verse/src/terminal/select.rs` uses, so a mark
follows its text into the scrollback.

A program can print OSC 133 itself. Marks are therefore advisory: they shape
how the pane draws, never what is allowed. A forged mark cannot run anything
or attach anything to a thread.

#### The block record

| Field | Source |
| --- | --- |
| Terminal and session | The pane |
| Command line | The hook's private OSC, else the text between `B` and `C` |
| Working directory | OSC 7 at the prompt |
| Start and end time | When `C` and `D` arrive |
| Exit status | `D;N` |
| Output range | Absolute lines from `C` to `D` |
| Alternate screen used | Whether the command entered it |
| Origin | Typed by you, a proposal from thread T, or an agent typist |

The host keeps a bounded *block journal* per terminal: the record without
the output, plus the output's sequence range in the replay ring. Any client,
including a late joiner and the phone, can list blocks; a block whose output
has left the ring says so.

#### What you can do with a block

Each block draws with a gutter mark in the white ladder (the exit status as
intensity and a glyph, never a hue), its duration, and its directory when it
differs from the previous one.

- **Navigate.** The prefix, then `Up` or `Down`, jumps between blocks.
- **Copy** the command, the output, or both.
- **Search** within blocks, filtered by exit status or directory.
- **Collapse** long output to its first and last lines.
- **Rerun** the command in the same directory, as a new block.
- **Attach to a thread.** Adds the block (command, status, and a bounded
  head and tail of output) to a new or current thread as context, after you
  see exactly what will be sent.
- **Share** the block as a static excerpt, under the same consent rules.

A block from a full-screen program (`vim`, `htop`) has a command, times, and
a status but no output range.

### Threads

#### A request opens a thread

A request goes to the chat router through `openagents_chat::client`, exactly
as a message in OpenAgents Terminal does, with `surface: "terminal"` and the
context the [context strip](#context-and-consent) shows. The reply streams
into a thread pane beside the terminal (a vertical split by default), or as
a one-line card in the terminal with the thread a key away, by your setting.

The thread pane is OpenAgents Terminal's transcript: Markdown, Coder runs
with grouped tool calls, the rail, diffs, and file views, already built in
`crates/openagents-terminal` and `crates/coder-terminal`. In the first phase
it is literally a pane running `openagents terminal --thread ID`. Later it
draws natively; see [Architecture](#architecture).

Threads persist in the host's store, sync with the desktop app and a paired
phone, and resume with the thread picker, as today. A session records which
threads belong to it, so reattaching a session restores its thread panes.

#### Proposals: commands a thread suggests

When the answer is a command to run in your shell, the router or Coder
returns a proposal instead of prose: the plan shape `coder::shell` already
reads (`{"v":1,"commands":[{"command","why"}]}`), plus an effect class. The
terminal shows each proposal as a pending block in the pane the request came
from: the command, why, and its effect class. Nothing runs until you press
Enter on it, which types it at your prompt as if you had typed it, so it
lands in your shell history and your shell's own semantics apply. You can
edit it first.

Its output block attaches back to the thread automatically, because the
thread proposed it. The thread then judges the outcome (the `outcome`
question in `crates/coder/src/classify.rs` already does this for Coder's own
shell rounds) and proposes the next step, answers, or stops. That is the
loop: request, proposal, your key, visible output, next proposal.

The router needs one new route for this. The `cli` route proposes
`openagents` commands, descended from a generated command tree with effect
classes (`crates/coder/src/cli_route.rs`). A `shell` route proposes ordinary
shell commands, with the effect class from a typed question rather than a
command tree, and the existing gate table decides what a surface may be
offered.

#### Permits and approvals

- The `Permit` (`crates/coder/src/permit.rs`) still decides whether a turn
  may propose commands at all. A turn the router sent to clarification
  proposes nothing.
- A proposal into your live shell always waits for your key. An opt-in
  setting may auto-run proposals whose effect class is `read_only`, matching
  the router's gate for the desktop and terminal chat. It is off by default
  and admitted per workspace root (the prefix, then `R`), recorded in
  `~/.openagents/terminal/autorun.json`; only an exact pending revision bound
  to the current pane, classed read-only, and written as one plain command
  runs (`terminal_core::autorun`, #10695).
- Coder runs are unchanged: they start under the router's dispatch, work in
  their own worktree, and approve every step (#10104). The difference is
  where they run: a Coder run touches its worktree, a proposal touches your
  shell.

#### Agents attached to panes

- **As programs.** Codex, Claude Code, Grok Build, Devin, and `microcoder`
  are terminal programs; any pane can run one, as the Verse overlay's `o`
  key already opens OpenAgents Terminal.
- **Coder runs as panes.** A running Coder task can open as a pane: today's
  full-screen run view (`Ctrl+R` in OpenAgents Terminal) in a split.
- **As typists.** An agent can drive a terminal only when you hand it the
  typist role, for that terminal, with a visible badge in the pane's title
  (`driven by Codex`). Any key you press takes the role back at once. Every
  key an agent sends is recorded in the thread. The Verse control socket
  (`openagents verse terminal send`) is a same-user precedent; under the
  smart terminal it obeys the typist rule like any client.
- **As watchers.** An agent can read a pane's blocks only through what a
  thread attaches, never by reading the screen on its own.

### The real terminal underneath

- Full VT behavior through `coder-vt`: everything the Verse overlay reached
  in its parity pass (selection, copy mode, mouse reporting, focus events,
  cursor shapes, OSC 8 links, OSC 52 writes, wide characters, function keys,
  and keypad modes), plus the Kitty keyboard protocol later.
- Any program: `vim`, `htop`, `less`, `ssh`, `tmux` (the prefix twice sends
  the prefix through), and the ratatui programs in this repository.
- OpenAgents Terminal's chat becomes one view inside the smart terminal, not
  the whole product. Its `openagents terminal` command keeps working on a
  TTY.

### The multiplexer

The model is Superlogical's session with the Verse overlay's controls.

- **Sessions outlive windows and devices.** Every pane attaches to a host
  terminal. Closing a pane detaches; closing a terminal is a separate action
  that asks first while a program runs. A host restart marks panes `lost`
  and offers a new shell, never presenting a new process as the old one.
- **Panes and tabs.** The binary split tree in
  `crates/verse/src/terminal/layout.rs`, with zoom, tabs, and geometric focus.
  `crates/coder-wm`'s dwindle tree stays the compositor's.
- **Panes on several computers.** Each pane names its host; a tab can mix
  this computer and a remote one, with the host and route (loopback,
  tailnet, direct, or relay) in the pane's title.
- **One typist per terminal**, enforced by the host for every attachment,
  including two devices of the same owner, with the take, release, size, and
  pan rules adopted from the earlier repository's driver.
- **Shares** with watch and drive rights, per terminal, as specified in the
  [in-world terminal](../verse/in-world-terminal.md#sharing-with-others):
  a share starts at the terminal's head, pausing blanks it for viewers, and
  revoking it detaches them.
- **Layouts saved** on the host as part of the session (terminal
  references, the tree, tab names, and thread panes, never output), so a
  layout follows you between devices. A surface may keep a local override
  for its own screen size.
- **Keys.** The `Ctrl+B` prefix the overlay already uses, and `coder-binds`
  Super chords on CoderOS. The [prefix table](../verse/in-world-terminal.md#multiplexing)
  gains `a` (ask), `Up` and `Down` with the prefix for blocks, and `t` (open
  the thread pane).

### Surfaces

| Surface | How it draws | Transport | Notes |
| --- | --- | --- | --- |
| Standalone window | `winit` and wgpu, `terminal-gfx` | Local host or NIP-TERM | The product. Same code as Verse's overlay. |
| Verse overlay and in-world screens | `terminal-gfx` into the frame or a texture | The same | In-world screens per the [in-world terminal](../verse/in-world-terminal.md#in-world-screens-others-see) plan; the world carries bindings, never bytes. |
| Phone | Today, the Rust Native terminal view in `coder-computers`; later the grid renderer | NIP-TERM through `coder_computers::live` | Blocks, threads, and proposals fit the phone well: tap a proposal to run it on your computer. |
| Browser | `terminal-gfx` on WebGPU or WebGL2 (`wasm32`) | A host WebSocket over a tailnet, or relay artifacts at the lower rate | Needs a `wasm32` session transport. |
| Plain TTY | A degraded mode over SSH: the shell hook gives `#` requests and inline proposals in your normal terminal, and `openagents terminal` stays the thread view | Local | The optional [TTY multiplexer](../../crates/terminal-mux/README.md) draws admitted host grids with ratatui; it retains TTY redraw limitations. |

### Context and consent

What a request may send is shown before it is sent, in a *context strip*
above the ask line or the chip: one item per thing, each removable with a
key.

- Always: the request text and the surface.
- By default, shown and removable: the working directory, the Git branch,
  and a change count (not the diff).
- Only when you attach it or the request is about it (for example, a request
  typed right after a failed block, which the strip offers): a block's
  command, exit status, and a bounded head and tail of its output.
- Never: other panes, unattached blocks, environment variables, the
  clipboard, files (except through a Coder run, which works in its own
  worktree), or anything a share viewer typed.

Rules:

- Nothing leaves the machine without consent. A request goes to the chat
  worker, which is off the machine; the strip is the consent. Shell lines
  are never sent to classify them (rule 4 runs locally).
- Attached output is scrubbed for credential shapes before it is shown in
  the strip, so you see the scrubbed text that would be sent. Scrubbing is a
  backstop; the strip is the control.
- Threads record what was attached. ATIF exports and traces contain only
  attached blocks.
- Blocks, titles, working directories, and command lines stay out of NIP-MV,
  presence, world chat, Verse replays, and logs, as the in-world terminal
  already requires.

#### Static block excerpts

Implemented 2026-10-06
([#10697](https://github.com/OpenAgentsInc/openagents/issues/10697),
`terminal_core::excerpt`). A block can leave the terminal as a static
excerpt, `openagents.terminal-excerpt.v1`: the scrubbed command, its exit
status and duration, the first 20 and last 20 output lines (each at most
200 characters), how many lines were left out between them, whether the
terminal still held the whole output, and its source: the mount instance,
the block number, and a digest over the scrubbed block. A digest over the
excerpt's own content lets a recipient check it with
`excerpt::verify` without reaching the terminal; the sharer, who still has
the block, checks the source with `excerpt::same_source`. It holds no
working directory, environment, other block or pane, clipboard content, or
undisplayed output. A running block, a full-screen program's block, and a
block without a command are refused.

Sharing is two steps through the mount's control request. `{"op":
"excerpt", "block": N}` answers the preview (the excerpt, its plain text,
and its digest); `{"op": "excerpt", "block": N, "consent": DIGEST}` exports
exactly that excerpt to the caller and records its digest, block, and time
in the status's `exports`. Consent to a preview that changed since, such as
output that left retention, refuses. An excerpt is static: it is not a
watch or drive share, it grants no input, task, review, or spending right,
and nothing about it reaches presence. The recipient keeps the excerpt;
the mount keeps only its identity for the session.

### Safety

- **Rights stay the host's.** `terminal`, `observe`, and terminal shares are
  checked per message by `coder-host`. The smart terminal adds no authority.
- **Permits and keys.** Proposals need your key; Coder runs follow #10104 in
  their worktree; the `Permit` narrows both.
- **Dangerous-command guard.** A proposal whose effect class is
  destructive (recursive deletes, disk tools, force pushes, piping a
  download to a shell, `sudo`) shows a warning band and needs a second key.
  The deny list in `crates/coder/src/shell.rs` refuses the commands that end
  a machine outright, as it does for Coder's own shell rounds. Commands you
  type yourself are never second-guessed.
- **Paste.** A multi-line clipboard paste bound for a program that did not
  enable bracketed paste, on the primary screen, shows its line count and
  waits; Enter sends the exact text once and Escape drops it
  (`crates/terminal-core/src/paste.rs`). A full-screen program, the sheet's
  input line, and text an agent sends over the control socket are not held.
- **Secrets on screen when sharing.** A share starts at the terminal's head,
  so earlier output is never sent; pausing blanks the pane for viewers; the
  pane shows who watches and who types; a password prompt with echo off
  shows nothing to anyone.
- **Agent typists** are visible, revocable with any key, and recorded.
- **Output is untrusted.** Marks are advisory, links open only on a modified
  click, clipboard reads stay impossible, and a background pane cannot write
  the clipboard.
- **Tests stay off the owner's computers**: a scratch host under a temporary
  `HOME`, as AGENTS.md requires.

## Architecture

```text
 host (coder host serve)                                    any surface
 ┌─────────────────────────────────────┐                    ┌──────────────────────────────────┐
 │ session: terminals, threads, layout │                    │ terminal-core                    │
 │ coder-pty host                      │   NIP-TERM over    │  session client, panes, layout,  │
 │  PTY ──tee──► frames (seq) ─────────┼──NIP-REACH, relay,─┼─► coder-vt per pane              │
 │       └────► coder-vt (authority):  │   or loopback      │  blocks from OSC 133 marks       │
 │              query replies, title,  │                    │  input line: chip, rules 1 to 4  │
 │              cwd, block journal,    │◄── snapshot on ────┤  typist, shares, context strip   │
 │              snapshot on join       │    join, input     │        │                         │
 │ rights: NIP-HOST grants, shares     │                    │        ▼                         │
 │ shell hook injection                │                    │ terminal-gfx (wgpu grid)         │
 └─────────────────────────────────────┘                    │  window │ Verse │ web │ (phone)  │
                │                                           └────────┬─────────────────────────┘
                ▼                                                    │ requests, proposals
   thread store (host) ◄──── openagents_chat::client ◄───────────────┘
        └─► chat router (NIP-CJ) ─► Coder (coder::task::local) in a worktree
```

| Layer | Crate | Reuses | New |
| --- | --- | --- | --- |
| Emulator | `crates/coder-vt` | Everything it has | OSC 133 and OSC 7 marks, the private hook OSC, per-row dirty marks, serialize and restore for snapshots, later the Kitty keyboard protocol |
| Host terminals | `crates/coder-pty` (host feature) | PTYs, the replay ring, gaps, idle expiry, per-attachment budgets | An authoritative `coder-vt` per terminal, side-effect ownership, the block journal, snapshot frames, the typist seat, shell-hook injection |
| Protocol | `nips/openagents/NIP-TERM.md` | Open, attach, input, resize, signal, close, frames | Extensions: `snapshot`, `history`, `typist` (take, release), `share`, block journal reads |
| Authority | `crates/coder-access`, `crates/coder-host` | Rights, per-message checks | The share grant; the session record and its saved layout |
| Client engine | **new** `crates/terminal-core` | Extracted from `crates/verse/src/terminal`: `layout.rs`, `select.rs`, `copy.rs`, the encoders in `keys.rs` and `mouse.rs` (with its own key type instead of `winit`'s), `stats.rs`, and the pane/session code in `pty.rs` behind a trait with two backends: in-process `coder-pty` and `coder_computers::terminal::session` | Blocks, the input-line state machine, the context strip, the typist and share client state, session layouts. It absorbs the `coder-mux` crate the in-world terminal page proposed. No renderer, `winit`, or network. |
| Renderer | **new** `crates/terminal-gfx` | Extracted `draw.rs` and `glyphs.rs`, over `verse-gfx`'s `UiBatch` and `Atlas` | Instanced cells per pane, damage by row, block gutters, chips, render to texture for in-world screens |
| Thread view | `crates/openagents-terminal`, `crates/coder-terminal` | Today's app state and components | A crossterm-free and Oniguruma-free feature of `coder-terminal`, so the thread view's ratatui `Buffer` draws as a grid in `terminal-gfx` on every surface |
| Requests and proposals | `crates/openagents-chat`, `crates/coder` | `openagents_chat::client`, the router, `Permit`, the plan shape in `coder::shell`, the `outcome` question, the gate table | The `shell` route, the effect-class question, proposal events on the client stream |
| Local decisions | `crates/lev`, `crates/laya`, `crates/kev` | Their `POST /v1/systemone` doors | The `line-kind` question set in `questions/`, with a measured baseline before a threshold is set |
| Window | **new** `crates/terminal-app` | `terminal-core`, `terminal-gfx`, `winit` | The standalone binary, bundled in the Mac `.app` and Linux packages beside `openagents` |
| Verse | `crates/verse` | Overlay, control socket | Becomes a thin host of `terminal-core` and `terminal-gfx` |
| Phone and web | `crates/coder-computers`, `crates/everglade-web` | The phone terminal screen and session | Blocks and proposals in the phone view; a `wasm32` transport |
| Look | `crates/coder-ui` | `theme::Intensity`, the white ladder | Nothing |

Boundaries:

- `terminal-core` decides nothing about authority; the host does.
- The terminal never interprets a request; the router does.
- Nothing in `terminal-core` or `terminal-gfx` depends on Verse, so the
  window does not link the world, and Verse keeps building for the web
  without the terminal feature until the web phase.

## What exists and what is missing

Estimates are agent-hours at this repository's measured pace, including
tests and a capture or receipt: one coding agent working one area, with
several areas running in parallel. The pace is from the commit history: 795
commits from October 3 to 5, 2026, including the Verse engine work (279
Verse commits on October 4), and the in-world terminal itself, which went
from nothing to a multi-pane overlay in about an hour and to VT parity with
a performance receipt in about another hour on October 5. The earlier
Coder repository's multiplexer research and code also shorten the
Superlogical half.

| Area | Exists | Missing | Agent-hours |
| --- | --- | --- | --- |
| Emulator | `coder-vt` with the Verse parity pass | OSC 133 and OSC 7 marks, hook OSC, per-row dirty marks | 1 |
| Shell hook | Nothing | zsh, bash, and fish hooks with injection; the accept-line request and reply; the command table | 1 to 2 |
| Blocks | Absolute-line selection in Verse | Block index, gutter, navigation, copy, collapse, rerun, attach | 1 to 2 |
| Input line | The ask key does not exist | Chip, rules 1 to 3, the corrected-command offer | 1 |
| Local line classifier | Lev, Laya, and Kev doors | The `line-kind` question set, baseline and threshold measurement | 2 |
| Thread pane, first form | `openagents terminal --thread ID` in a pane | Open it from a request with the pane's context | 0.5 |
| Proposals | `coder::shell` plan shape, `Permit`, the CLI route's gates | The `shell` route, effect-class question, proposal events, pending blocks | 2 to 3 |
| Context strip | Nothing | Strip, scrubbing, attach rules | 1 |
| Crate extraction | `crates/verse/src/terminal` | `terminal-core` and `terminal-gfx` split, Verse moved onto them | 2 |
| Standalone window | Nothing | `terminal-app`: window, menus, fonts, settings, packaging | 3 |
| Native thread view | `coder-terminal` components | Crossterm-free feature; `Buffer` to grid | 2 |
| Host authority parse and effects | Client-side replies | Host `coder-vt`, effect ownership, clients stop answering | 1 to 2 |
| Snapshot on join and history | Ring replay with gaps | `coder-vt` serialize and restore, snapshot and history frames, NIP-TERM text, conformance tests | 3 to 4 |
| Sessions and saved layouts | Host session records with list, read, write, and remove (#10652) | Clients that save and restore layouts from them | 1 |
| Typist | Nothing | Host seat, take and release, pan rule, title badge | 1 |
| Shares | Share grant, enforcement, and the relay guest in `coder-pty` and `coder-host` (NIP-TERM [Shares](../../nips/openagents/NIP-TERM.md#shares), #10676) | Pause and viewer list (#10681) | 1 to 2 |
| Block journal on host | Nothing | Journal, NIP-TERM reads | 1 |
| Agents as typists | Verse control socket | Typist handoff to an agent, recording, badge | 1 |
| Phone | Terminal screen and session | Blocks, proposals, thread link in the phone view | 1 to 2 |
| Web | `everglade-web` | `wasm32` transport, browser keys and clipboard | 4 to 6 |
| TTY degraded mode | `openagents terminal` | Hook-only requests and inline proposals in a plain terminal | 1 |

## Roadmap

The [workbench roadmap](workbench-roadmap.md#delivery-sequence-and-ownership)
owns delivery order and issue dependencies. The standalone app moves into
the first release alongside the Grid demo; the previous three-to-four-day
estimate is not a launch commitment.

1. **Today: Grid and standalone MVP.** #10642 delivers blocks, requests,
   context, and pending proposals with zsh integration first. In parallel,
   extract `terminal-core` and `terminal-gfx` in #10643, mount both in
   `terminal-app` and Verse, and package a native standalone install.
   #10644 retains the failing-test demo, independent install, and
   performance evidence; it depends on both implementation issues.
2. **Next pass: Everglade.** Open the same application against studio goals,
   seats, tasks, decisions, memory, and exact reviews. Keep station placement
   separate. Extend shell integration to bash and fish.
3. **Host terminals and sessions.** Authoritative emulation and side effects,
   snapshots and history, journals, saved sessions, multi-host panes, typists,
   and shares. Host work can run beside the workshop adapter.
4. **The full workbench.** Native thread rendering and typed panes for agent
   children, files, tools, artifacts, background work, knowledge, and Gym.
   Add the local classifier only after its measured baseline is ready.
5. **Paid cloud placement.** An explicit cloud-computer option paid with
   credits over the shared money and admission contracts. Sponsored cloud
   inference and paid remote execution retain distinct identities.
6. **Mobile, web, TTY, and world screens.** Shared resource contracts and
   permitted controls on each supported surface; optional screen textures
   and furniture. None requires another router or task owner.

## Open questions for the owner

The workbench plan sets working defaults for this release: OpenAgents
Terminal is the product, `openagents-terminal` is the proposed graphical
binary, `openagents terminal` keeps its TTY chat behavior, the window ships
beside the Grid MVP, zsh is integrated first, every shell proposal waits for
Enter, and native macOS is the first install target. The remaining questions
below are later design choices, not blockers for today's three issues.

1. **Name.** Does the smart terminal take the name OpenAgents Terminal, with
   today's chat screen renamed the thread view, or does it get a name of its
   own?
2. **Packaging.** A separate `terminal-app` binary bundled beside
   `openagents`, a window the desktop app opens, or both? Should `openagents
   terminal` on a machine with a display open the window?
3. **Default routing.** Is the rule order above right, especially that an
   ambiguous line with a resolving first word runs as a command when no
   local model is confident? Should `#` or another prefix be the request
   prefix?
4. **Jev for the line question.** Lev covers macOS on-device; is opting in to
   Jev acceptable on other systems, or should those use a local Laya or Kev
   door only?
5. **Auto-run of read-only proposals.** Off by default, as proposed, or on
   for your own computers?
6. **Shell route.** Should general shell proposals come from the router's
   new `shell` route, from Coder's turn, or both?
7. **Typist across your own devices.** Host-enforced one typist for all
   attachments, including your phone and laptop together, as proposed?
8. **Layouts on the host.** This page decides that layouts live in the
   host's session record so they follow you. Is a local-only option needed?
9. **libghostty.** Is a Zig-built C library ever acceptable as an optional
   codec for Superlogical interop, or does the terminal stay pure Rust?
10. **Superlogical itself.** If Superlogical publishes its protocol, is a
    client adapter (our surfaces attaching to its sessions) or a server
    adapter (its clients attaching to our hosts) more valuable?
11. **Windows.** The `coder-pty` host supports ConPTY. Is Windows a target
    for the window in phase 2, or later?
