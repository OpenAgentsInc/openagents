# Desktop access to its own Coder

The desktop uses the existing same-user control socket as a broker for its
own computer. The window sends typed NIP-HOST operations and history queries.
The resident host prepares and verifies them with the portable clients the
phones use. Secrets remain in the resident host; the window receives outcomes
and bounded history pages only.

The broker signs task operations as the locally established owner. NIP-HOST
already admits that principal without a device grant. Every request still
passes `Authority::handle`, including signature, owner, freshness, operation
validation, retained-reply, and task-owner checks. The socket's kernel peer
check is required before the broker accepts a message. A network device
cannot use this route or acquire operator authority.

The window supplies a stable request identity. The broker persists the exact
signed pending request in an encrypted cache before dispatch, rejects changed
operations under that identity, and reuses the packet while it is fresh.
After expiry, the same task-owner identity still prevents a second effect.
Cache failure refuses dispatch. Creating a task remains inert except under
the host's existing local auto-start policy.

History stays a separate read-only authority. The broker pairs a temporary
client with the configured observer's Coder source only, prepares the query
with `coder_connect::client::Client`, and verifies the observer's reply. The
same source-bound cursors and page limits apply. It cannot read arbitrary
paths or offer another engine's private history. This connection's key never
leaves the host and grants no task execution.

Using a second enrolled desktop device over loopback would require a secret
in the window or another signing broker, plus grant renewal and enrollment
state for a caller already authenticated by the operating system. The local
owner broker preserves the existing boundary and shares protocol admission
without adding those credentials. Remote computers continue through their
existing device grants and connections.


A desktop chat delegates with the shared `openagents_chat::delegation` prompt
and project rules. The host keeps an encrypted plan before dispatch and binds
the task receipt to the conversation afterward. A lost acknowledgment retries
the same admitted request. The native **Run Coder** action does not enable
execution: the configured auto-start policy still decides whether the accepted
task starts. Full project labels and computer judgments remain in encrypted
chat records across a restart.

A bound chat follows its Coder task with `openagents_chat_app::task_chat`.
The local activity reader uses the same summary builder as phone updates.
ATIF pages pass through the phone's conversation parser and transcript
projection, including tool rows. Reads run on the host worker. The window
retains at most 240 rows and 160 KiB of projected text, plus a bounded chunk
buffer for records split across pages. New turns suppress carried messages,
and a changed source refreshes its catalog and cursors.

The composer uses the phone's Send, Queue, and Answer modes and steering
choices: **Steer now** before a turn starts, or **Stop and send** while it
runs. Question and approval answers remain task input; they grant no new
execution authority. Queue editing uses the host's lease, renewal, and
privacy rules. The window displays eight queued messages per page and never
offers text editing for another sender's withheld message.

Background reads do not block a submission. An uncertain mutation keeps its
exact command ID and bytes for retry. A verified refusal preserves the draft
for editing. An acknowledgment clears only the editing state that submitted
those bytes; newer edits remain. Task revision changes also invalidate a
pressed transcript button before release.

## Coder on this computer, from a chat

A coding request typed in a desktop chat runs Coder right here, with no
pairing, no registered project, and no accept step: the same local run
`openagents chat` starts ([#10032](https://github.com/OpenAgentsInc/openagents/issues/10032),
[#10033](https://github.com/OpenAgentsInc/openagents/issues/10033)). When
the chat router's reply to a message sent from this window judges it coding
work, or the person picks **Run Coder**, the window's Coder lane
(`src/worker.rs`) calls `coder::task::local::Local::start` with the chat's
handoff prompt. Every coding agent not turned off, Codex first, then Claude
Code, Grok Build, Devin, and OpenCode (#10091, #10184), each only when signed
in here with capacity; Coder's own worktree of the project's `HEAD`; the same engine,
failover, and ATIF recording as a host's auto-start. Reopening a chat never
starts a run.

The local capability settings ([`openagents settings`](../cli/settings.md),
`coder::task::settings`) apply here as in the terminal: the agents turned off
and the order, the usage threshold, the project folders (tried after the ones
below when a chat names none, and a checkout outside them is not a project),
and what commands may reach. With `coder.start: ask_first` a coding reply
only offers **Run Coder**. The Settings page (#10021) edits the same file.
The same setting decides for a paired phone (#10101): while it is `at_once`
and **Let my phone start Coder here** is on, this computer's host advertises
`coder-start-at-once` in its presence, and a coding reply on the phone starts
Coder here with no tap; `ask_first` leaves the phone's **Run Coder**.

Its commands can use what is installed on this computer
([#10045](https://github.com/OpenAgentsInc/openagents/issues/10045)): Xcode's
`python3` and `git`, Homebrew's `rg`, rustup's `cargo`, nvm's `node`, and the
rest, with network access, while they still write only in Coder's worktree
and scratch. The allow list is derived from what is installed and recorded in
the run's ATIF; [Microcoder's repository
host](../coder/runtime/microcoder-repository.md) lists it.

The project is a default, never a gate: the chat's own project, then this
computer's projects (the one **Phones and computers** shows first), then the
project Coder last started in (`<tasks>/local/last-project`). When none of
them is a Git checkout, the chat says so with **Choose folder…** and starts
there.

The window records the task on its thread through the host
(`BindCoder`, host `local`), so the host's threads, and a phone, show it.
A bound thread follows its task with `Local::follow` from the first event:
a thread started in `openagents chat` shows its whole history and keeps
streaming while it runs. The rows are `openagents_chat_app::coder_run`,
drawn from the one event stream (`openagents_chat::coder_events`) the CLI
prints:

| Event | In the chat |
| --- | --- |
| `coder_started` | A card: the provider and model, why that provider, the project and worktree, and what it falls back to. A later turn says "Coder continued". |
| `step` | The request as the person's message; thoughts as a **Thinking** row; tool calls grouped as Grok Build groups them ([#10117](https://github.com/OpenAgentsInc/openagents/issues/10117), [design](../coder/design/2026-10-01-tool-call-groups.md)): consecutive reads, searches, listings, and fetches as one row labelled "Read 3 files, Searched 2 patterns" that a click opens to each call and what it returned, and each command or edit as its own **Run** / **Edit** row, running until its output; the reply as the assistant's message; a note as a quiet line. A later turn's carried conversation shows once. |
| `output` | Inside its command's row; a failure or time-out marks the row failed and names the exit after the command; cut output says so. |
| `provider_switched` | "Switched from Codex (…) to Claude Code (…): why", or that no other provider has capacity. |
| `progress` | The working line: step, bound, Jev's done estimate, and time. |
| `question` | A **Coder asks** card; the composer answers it. |
| `approval` | A **Coder asks to go ahead** card with **Approve** and **Deny**; the composer answers in words. |
| `result` | **Coder finished**: the summary, files changed with `+`/`−` lines, and the worktree, after a "Worked for …" line. The worktree's change against the run's base, at exact revisions (`coder::task::review`), opens in the **What changed** pane. |
| `failure` | **Coder didn't finish** and why. |
| `stopped` | The stop, in a line. |

The controls keep #10016's: **Stop Coder** stops the running turn
(`Local::stop`); while Coder works, Send queues the message for the next
turn (up to eight, each with **Send now** and **Remove**) and **Stop and
send** stops the turn and continues with the message; after a question, an
approval, or an ended turn, Send continues the task in the same worktree
(`Local::answer`). Set `OPENAGENTS_DESKTOP_CODER_EVENTS=FILE` to append every
event the window receives to `FILE` as NDJSON, the lines `openagents --json
chat follow` prints.

## Saved Codex and Claude Code sessions

**Saved sessions** reads this computer's `.codex` and `.claude` directories
through `coder-history`, on macOS and Linux; `coder-history` does not read
them on Windows. The desktop worker opens the configured local roots
only when you request the list. Catalog pages show each session's title,
harness, and saved time. Transcript pages use the shared phone conversation
parser and Rust Native transcript surface. The original files remain read-only.
An unavailable source stays visible with its status and cannot be opened.

The shared `openagents_chat_app::retained` reader bounds catalogs to 32 entries,
transcript pages to 32 KiB, projected content to 240 rows and 160 KiB, and its
chunk buffer to 384 KiB. Source identity, incarnation, record IDs, and byte
boundaries must match before a page changes the displayed rows. Earlier pages
extend the reader without replacing the newest context used for continuation.

**Continue with Coder** submits the newest loaded context, bounded to 16 KiB,
to the selected project. The UI and prompt disclose that earlier records may
be omitted. This creates an ordinary Coder task through the same local broker
and admission used by **Run Coder**. It does not resume either tool's harness.
The host's existing auto-start policy decides whether the task starts.

The shared application prepares stable conversation and request IDs. The host
saves the exact continuation plan in its encrypted control cache before
creating a conversation or dispatching a task. A lost acknowledgment retries
that plan and binds the original receipt across restarts. Changing the plan
under its ID is refused. The resulting conversation opens the existing Coder
task chat. Opening a retained session leaves an unsent ordinary chat draft
unchanged. A late continuation response does not replace a chat selected
while the request was pending.

These local file reads do not extend phone observer authority. The existing
NIP history connection continues to expose only Coder sources. The portable
reader and continuation factory compile in the phone's Rust library; no phone
screen mounts them yet. #10028 (closed) brought the phones the shared editing,
card menus, images, and code colors, not these surfaces.

## Engine and usage

The sidebar's engine rows and Settings → Coder show the engine, the model,
whether Codex and Claude Code are signed in, and each usage window. Grok
Build, a default provider for this computer's own runs, shows there whenever
it is installed, as signed in or not; it reports no usage, so its row has no
meter ([#10091](https://github.com/OpenAgentsInc/openagents/issues/10091)). Those values come from this computer's
`autostart.json` routes and the usage book. The window sends `engine_status`
and receives percents and reset times. It cannot change the engine, the
model, or a credential, and it does not read a provider token.

The host answers from a cached report. When a reading is due and usage probes
are on, the host runs `coder host autostart status --refresh` in the
background, in the Coder process. A probe reads a token only then, and only
to send it to that provider's own usage endpoint.

The ring and the route cards reimplement Zeron's account usage rings (public
MIT zeronsh/zeron) in Rust Native. The shared strip lives in
`openagents-chat-app`; no phone screen mounts it yet.

## What changed

A finished Coder task that recorded a unified diff shows a **What changed**
card. Opening the card shows that diff in a pane on the right. The pane is
read-only: it can scroll and close, and it cannot edit the change.

The shared parser in `openagents-chat-app` keeps the lines. The desktop paints
only the lines that fit in the pane, one line at a time, and applies syntax
spans as color. Spans do not change the line height. The card and the pane
reimplement Zeron's unified diff pane (public MIT zeronsh/zeron) in Rust
Native.

The card names the exact revisions it shows (#10067): the run's base commit,
the worktree's `HEAD`, and the tree of the worktree's content when it was read
(`coder::task::review`, written through a private index, so the worktree, its
index, and its refs stay as they were). The file and line counts are Git's
for that base and tree, so they stay whole when the diff is cut. A cut diff
says how much it shows; a diff Git could not write says why; binary files
count as "not counted", never zero. While the card shows, the change is read
again every ten seconds; a read that names another head marks the view stale,
keeps it on screen, holds back **Publish**, and offers **Refresh**.

**Publish** (#10068) commits exactly the reviewed tree on the worktree's
`HEAD` and pushes it as the repository's `.openagents/coder-issues.json` says:
onto its branch, fast-forward only, for `"land": "main"`; otherwise, and by
default, to `coder/review-…` with a draft pull request through `gh`. It is
the host's operation (`coder::task::publish`), keyed by the task and the
reviewed revisions, recorded beside the task before each effect, and refused
for a head the worktree has moved past. A push whose result is unknown is
recorded as uncertain; publishing again reads the remote first and pushes
only if the commit is not there. The card then links the pull request or the
commit.

The phone shows the same card from the same shared reviewer
(`openagents_chat_app::changes::Reviewer`) through NIP-HOST `task.review`
(`observe`) and `task.publish` (`operate`), with the diff line by line up to
160 lines. A computer that reviews no change for a task shows no card on the
phone and the transcript's diff, without revisions or **Publish**, on the
desktop.
