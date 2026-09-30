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

## Saved Codex and Claude Code sessions

**Saved sessions** reads this computer's `.codex` and `.claude` directories
through `coder-history`. The desktop worker opens the configured local roots
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
reader and continuation factory compile in the phone's Rust library; native
phone mounting remains part of #10028.

## Engine and usage

The chat header shows the engine, the model, whether Codex and Claude Code
are signed in, and each usage window. Those values come from this computer's
`autostart.json` routes and the usage book. The window sends `engine_status`
and receives percents and reset times. It cannot change the engine, the
model, or a credential, and it does not read a provider token.

The host answers from a cached report. When a reading is due and usage probes
are on, the host runs `coder host autostart status --refresh` in the
background, in the Coder process. A probe reads a token only then, and only
to send it to that provider's own usage endpoint.

The ring and the route cards reimplement Zeron's account usage rings (public
MIT zeronsh/zeron) in Rust Native. The shared strip lives in
`openagents-chat-app`. Mounting it on a phone remains part of #10028.

## What changed

A finished Coder task that recorded a unified diff shows a **What changed**
card. Opening the card shows that diff in a pane on the right. The pane is
read-only: it can scroll and close, and it cannot edit the change.

The shared parser in `openagents-chat-app` keeps the lines. The desktop paints
only the lines that fit in the pane, one line at a time, and applies syntax
spans as color. Spans do not change the line height. The card and the pane
reimplement Zeron's unified diff pane (public MIT zeronsh/zeron) in Rust
Native. Mounting the card on a phone remains part of #10028.
