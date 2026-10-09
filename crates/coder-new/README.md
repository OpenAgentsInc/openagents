# Coder terminal

A Ratatui terminal with bundled plugins, live chat, and a separate demo mode.
Interactive runs start in live mode with an empty transcript. `/demo` toggles between live and demo;
`--demo` starts with the fixtures. Demo mode shows Read, Search, Edit, and Run
examples, a diff, four running
delegations, and a composer. Delegations use the same agent names, tasks, and
token counts as the rail. Their magenta diamonds pulse while running commands
show a rotating spinner. Each agent conversation has its own tool calls,
arguments, results, and current activity.

The [shared component and web catalog specification](../../docs/coder/rust-native/coder-components.md)
uses this crate's public presentation as the complete source inventory for the
first Coder Cloud deliverable at `/components`. It requires shared Rust Native
components, interactive fixtures, and exact full-screen web recreation; it does
not establish a delivered browser UI.

Type `/` to see commands above the input, following the existing OpenAgents
terminal's slash suggestions. The command and description occupy separate
columns. Typing filters the list; Up/Down selects, Tab completes, Enter runs,
and Esc dismisses it. The supported commands include `/demo`, `/plugins`, `/models`,
`/appearance`, `/brainstorm`, and `/help`. `/models` appears when a model provider plugin is enabled;
`/brainstorm` appears when its plugin is enabled.

Enter `/appearance` to configure **Use System Terminal Background**. Press Space
or Enter to toggle it, and Esc to return. When on, Coder leaves backgrounds
unpainted so your terminal's configured background shows. When off, Coder uses
its existing background colors. The option defaults to off and persists in
`~/.openagents/coder-new/appearance.json` (or your `--state` directory).

Edit examples reuse Coder's port of Grok Build's diff renderer and syntax
highlighter. Rust tokens keep their syntax colors on red and green change
backgrounds, with line numbers and wrapping. The main view shows a cursor
change; the `grok-build` conversation shows a larger style change.

Mock plugin calls show their qualified operation, arguments, result, and running
state. The main conversation uses `terminal-inspector.layout.inspect` and
`palette-audit.colors.check`. Agent conversations also use keyboard and
conversation audit examples. These fixtures do not load or invoke plugins.

All transcript message bodies use Coder's shared Markdown renderer, ported from
Grok Build. Live, streaming, stopped, demo, and user messages render headings,
lists, emphasis, links, quotes, tables, and syntax-highlighted code fences.
User messages keep the compact prompt band. Markdown wraps to the transcript
width while model labels stay separate from the message content.
See the [Markdown transcript preview](../../docs/coder-new/markdown.svg).

Press F2 or enter `/plugins` to manage the bundled plugins. Up/Down selects a
plugin, Space turns it on or off, and Enter opens its settings. **Coder loop**,
**Jev**, **OpenAgents CLI**, and **ACP Subagents** default to on. **OpenRouter
BYOK** defaults to off; a key imported at startup enables it when no saved
preference exists. Saving the first key also enables it. A saved off preference stays off.

On macOS and Linux, this picker also lists locally installed extensions from
`openagents plugin install`. It refreshes on opening and once a second while
visible, preserving the selected plugin and unsaved settings. Enter shows an
installed extension's details and the CLI commands to enable or disable it.

OpenRouter's settings screen has a masked **OpenRouter API key**, an optional model ID, and
the fixed direct endpoint `https://openrouter.ai/api/v1`. Tab moves between
fields and actions; Enter selects an action; Esc cancels or returns to chat.
In live mode, **Test API key** checks the entered or saved key with
`GET /api/v1/key`. Saving a key also checks it; successful checks show
**Verified**, while errors report their cause. Turning the plugin off retains
the key and model settings. The default model is
`openrouter/free`, including when you leave the model field blank.
Replies stream from `/api/v1/chat/completions`
on a background worker, with prior live messages included. Esc stops a reply;
failed requests are not retried automatically. Argument validation errors return
to the model so it can submit a corrected plugin call within the same bounded
turn. Call identities, size limits, and completion are checked before dispatch.

**Brainstorm** defaults to off. Its settings show the configured HTTPS recipient
and the fixed house perspective. Opening, saving, enabling, and disabling cause
no service read. **Test connection** explicitly reads public discovery after
you enable the plugin and save the recipient. `/brainstorm search <public query>`
and `/brainstorm rank <hex-or-npub> [more keys]` work without a model key. The
exact query or canonical public keys go to the displayed recipient; no files or
conversation text are added. Esc cancels a lookup, and disabling refuses queued
or stale dispatch snapshots. Headless chat returns the current lookup's bounded
JSON observation or typed failure in `finished.reply`. Demo uses labeled fixtures
without requests.

Brainstorm result rows separate search relevance, raw continuous influence,
unknown zero coverage, and unavailable scores. They retain source, the separately
discovered house identity, response algorithms, times, expiry, and body digests.
These are unsigned HTTP observations; the API does not bind its effective
observer atomically to the discovered key. Following local and OpenRouter turns
receive the newest observation or typed failure within an 8 KiB context allowance.
Earlier observations remain in the existing transcript, without another query
history store. Lookups stay in the selected live conversation, including a
delegation, and following turns use that conversation's observation. Observation
recovery across chat reopening and automatic local-model lookup remain separate work.

OpenRouter receives `brainstorm_search_people` and `brainstorm_rank` only for an
enabled native binding. A model-proposed input waits for confirmation of the
exact outbound query or keys and HTTPS recipient. In the terminal, review the
complete disclosure with PgUp/PgDn, then press Y to confirm or N to reject;
Esc cancels. Headless callers use the existing `--approvals stdin` desk and its
structured `approval` event. Without a desk, the call refuses before a read.
The host keeps opaque `input_ref` values in a bounded, in-memory admission book;
references expire after at most five minutes. Identical queries and separately admitted key lists
can reuse it; a fresh admitted search permits ranking only its returned keys.
Changed settings, expiry, disablement, or cancellation refuse stale dispatch.
References grant no other effects, and response strings remain observations.
Model-owned companion CLI prompts retain their provenance; a nested slash
lookup cannot grant itself direct-input admission. Use the native tools for
model-proposed Brainstorm reads.

Enabled plugins register their tools and usage instructions for OpenRouter
chat. Tool calls run on the background worker, appear in the transcript, and
return their results to the model. Each turn allows at most eight model rounds
and 32 plugin calls. **Coder loop** (the `microcoder` tool and engine) uses Coder's own coding loop, Jev judgments
when configured, and commands bounded to the current checkout. With OpenRouter
off or unconfigured, live chat uses Microcoder through an existing Codex or
Claude Code login. That local loop retains its structured command protocol.
**OpenAgents CLI** exposes `openagents_cli`, which accepts an argument array
and adds `--json`. [Coder's installer](../../scripts/install-coder.sh) builds
and installs the companion `openagents` binary with Coder.

**Jev** supports TypeSafe direct, Vercel AI Gateway, and custom
TypeSafe-compatible API bases. In `/plugins`, open Jev settings and choose
the gateway, enter its API key, and save. The TypeSafe preset uses
`https://api.typesafe.ai` and `jev-latest`; the Vercel preset uses
`https://ai-gateway.vercel.sh/typesafe` and `typesafe-ai/jev`.
You can edit the API base URL and model. Changing the gateway's origin
requires a key for that connection. Key checks use the draft settings and
list models without running inference. The chat tool and Microcoder's Jev
judge share the saved connection. Its `jev` tool
uses the existing Rust SDK to send state and typed Noul, Choice, or Score
questions, then returns typed answers and probabilities. The registered
instructions describe when to use those judgments and how to interpret them.
**ACP Subagents** detects installed ACP agents on `PATH` and in common local
installation directories. Its settings show a checkbox list; new agents default
to on. Up/Down selects an agent, Space or Enter toggles it, and R refreshes
detection. Live choices save immediately and persist across restarts. Agents
turned off or no longer installed are excluded from chat's tools. Claude Code
requires its installed ACP adapter. Codex uses its native CLI bridge when no
ACP adapter is installed. The Cursor agent runs as `cursor-agent acp` with its
own login or `CURSOR_API_KEY`
([`docs/coder/runtime/cursor.md`](../../docs/coder/runtime/cursor.md)).
Chat delegates through
`acp_subagent` using a registered ID; it cannot choose an executable.
Coder starts tools and subagents with full filesystem and network access.
Native Codex bypasses approvals and its sandbox, and ACP permission requests
are approved. Explicitly gated hosts retain their approval policies. The host
closes each child session.
CLI delegations made through `Run` or `openagents_cli` also appear in the agent
rail, including when command output is redirected. Nested delegations have
their own selectable rows and retained chats.
See the [Jev settings](../../docs/coder-new/jev-settings.svg) and
[ACP settings](../../docs/coder-new/acp-settings.svg) previews.

Enter `/models` to open a searchable picker, reimplemented from Grok Build's
staged model and reasoning picker. Choices include the providing plugin, so
different plugins can offer the same model ID. For OpenRouter, the initial
shortlist contains the free router, GPT-6 Luna, GPT-6.1 Sol, Claude Fable 5.1,
Gemini 3.5 Flash, DeepSeek V4.1 Flash, and Grok 4.7. Live mode refreshes only
these seven models through public single-model lookups. The picker offers the
model's supported reasoning levels and a maximum output-token limit; automatic
routers omit fixed reasoning controls. Enter advances and saves on the final
step. Esc returns to the previous step or closes without applying changes.
Live model settings persist with the plugin; demo choices remain separate.
The selected settings apply to subsequent chat requests.
See the [model](../../docs/coder-new/models.svg),
[reasoning](../../docs/coder-new/model-reasoning.svg), and
[output limit](../../docs/coder-new/model-output.svg) previews.

When OpenRouter BYOK is enabled, the composer's top-right rail shows the selected
model, such as `openrouter/free`, in muted gray. The local trusted
[plugin definition](src/plugin_definition.rs) registers a `SelectedModel`
binding at `ComposerTopRight`; the renderer consumes enabled registrations.
Registrations have a plugin-scoped ID, priority, and cell limit, and disappear
when their plugin is disabled. The host resolves values and owns their styling.

Each live reply shows its actual model slug at the top right, using OpenRouter's
response metadata. Streaming and stopped replies retain the model observed for
that reply. Changing the selected model affects the composer and subsequent
requests; earlier reply labels keep their original attribution. Missing model
metadata leaves the label absent.
See the [reply attribution preview](../../docs/coder-new/model-attribution.svg).

Removing a key takes effect when you save. Disabling retains configuration.
Live OpenRouter preferences and its key survive restarts in
`~/.openagents/coder-new/plugins.json`; the other plugins use
`~/.openagents/coder-new/bundled-plugins.json`. The directory uses `0700`
permissions and the files use `0600`. Saves replace files atomically, and failed saves keep
the previous settings and editable draft. Connection verification runs again
when you test the restored key; verification status is not saved.
The editable key draft clears on save or cancel. Keys stay redacted in the UI
and errors. Demo settings and conversations last only until you quit, and demo
mode makes no requests or settings writes. Switching modes keeps their
preferences, drafts, and transcripts separate and cancels pending network work.
Startup imports `OPENROUTER_API_KEY`, `TYPESAFE_API_KEY`, `AI_GATEWAY_API_KEY`,
`TYPESAFE_BASE_URL`, and `TYPESAFE_DEFAULT_MODEL` from the process environment,
then the working directory's `.env`. A gateway key without a TypeSafe key or
explicit API base selects the Vercel preset. Saved routing takes precedence,
and imported keys apply only to the matching origin. OpenRouter also accepts
the existing `~/.openagents/openrouter.json`
key file. Saved plugin keys take precedence. Imports do not write settings;
`.env` values are read as literals without shell execution or expansion.
OpenRouter's own upstream-provider BYOK settings are separate from
this plugin's OpenRouter API key. See [OpenRouter authentication](https://openrouter.ai/docs/api_reference/authentication),
the [API reference](https://openrouter.ai/docs/api_reference/overview), and
[OpenRouter BYOK](https://openrouter.ai/docs/guides/overview/auth/byok).

```sh
cargo run -p coder-new
cargo run -p coder-new -- --demo
cargo run -p coder-new -- --plugins
cargo run -p coder-new -- --plugin-settings
cargo run -p coder-new -- --models
```

Type a draft, move with Left/Right or Home/End, and edit with Backspace/Delete.
Alt+Enter adds a newline. Enter sends a live message or appends a demo message.
Bracketed paste
inserts text without sending it. Trackpad scrolling and PageUp/PageDown scroll the conversation;
Ctrl+C quits. Demo tools and agents remain presentation fixtures; live mode
runs enabled plugin tools and configured ACP delegations.

The composer is a plain `❯` input with a blinking block cursor between
edge-to-edge horizontal rules. In demo mode, the four rows below it show `claude-code`,
`codex`, `devin-cli`, and `grok-build`. Agent
names and current tasks occupy separate aligned columns, with elapsed time
beside the token counts on the right, such as `1h 12m 38s · 8.2k tokens ↓`.
The token-count column grows to the widest count so the separators stay aligned.
The mock timers advance once per second while the preview runs. Narrow terminals
truncate task text, shorten the token label, and omit elapsed time when needed
to preserve the agent names.
Press Down to select the first agent, then Up/Down to switch conversations.
Trackpad scrolling affects only the transcript and preserves the selected agent.
Selection loads that agent's demo messages immediately. Up from the first agent
or Esc returns to the main conversation. A subtle background spans the selected row.
Each conversation retains its own
draft, cursor, preview messages, and scroll position while the preview is open.

The working directory and branch appear at the top right. Only selected agent
conversations show a title at the top left, using the agent's name.

Live terminal conversations save automatically as private ATIF files in
`~/.openagents/coder-new/sessions/`, the same store that `openagents coder`
uses. Type `/resume` to open the 100 most recent conversations. Use Up/Down,
Tab, or j/k to choose a chat, Enter to resume, and Esc to return to your draft.
`/resume <number>` selects from the last displayed list; `/resume <id>` opens
an exact session. Resuming restores the main transcript and subagent chats
without rerunning recorded tools, then continues the same session. The current
directory and plugin settings stay in effect. Stop active work before resuming.
Streaming replies checkpoint every five seconds and save again when they end;
typing and scrolling do not write session files. Demo conversations are not saved.

Signed in (`coder login`), `/sync on` also saves chats to your openagents.com
account, where they show in the web sidebar, marked Terminal with this
computer's name (`crates/coder-sync`). While Coder is open here with sync on,
you can reply to these chats on openagents.com: Coder takes the reply when it
is free (the chat is open, or nothing is being typed), opens that chat, shows
the reply as your message, and answers it with this computer's tools; the
answer shows on the web as usual. When Coder isn't open, the web page says so
and offers no reply box. It is off by default. `/sync all`
adds your earlier chats, `/sync off` stops, and `/sync delete` removes this
computer's chats from the account. Messages that look like they hold a
password or key are left out. Deleting a chat in one place deletes it in the
other. The setting lives in `~/.openagents/coder-new/sync.json`.

Every turn reads the repository's `AGENTS.md` and `CLAUDE.md` files from the
working directory up to your home folder, nearest first, plus your own
`~/.openagents/AGENTS.md`; instruction files in other directories of the
checkout are named so Coder reads them before working there. Coder also keeps
memory across sessions: small notes about you (every project) and about this
project (all its worktrees), saved when you say "remember ..." and deleted
with "forget ...". `/memory` lists them and `/memory forget NAME` deletes one;
`coder memory list|show|forget|instructions` does the same from scripts. Notes
are markdown files under `~/.openagents/memory` with a `MEMORY.md` index, stay
on this computer, and refuse anything that looks like a password or key.
`OPENAGENTS_MEMORY=off` turns instructions and memory off
(`src/memory.rs`, #11176).

Coder Noir uses Superlogical's Static Noir surfaces, content tones, and ANSI
palette, combined with Coder's neutral accent and cursor. The terminal field is
`#0e0e0e`; active borders are neutral, models and inline code use cyan, and
red marks errors and deletions. Markdown and syntax highlighting share those
roles, with dark red and green diff bands. Use a truecolor terminal to display
them exactly. [NOTICE](NOTICE) records the palette reference and renderer
attribution.

Export the actual Ratatui buffer without an interactive terminal:

```sh
cargo run -p coder-new -- --snapshot > docs/coder-new/mockup.svg
cargo run -p coder-new -- --live --plugins --snapshot > docs/coder-new/plugins.svg
cargo run -p coder-new -- --live --plugin-settings --snapshot > docs/coder-new/plugin-settings.svg
cargo run -p coder-new -- --live --snapshot > docs/coder-new/live.svg
cargo run -p coder-new -- --models --snapshot > docs/coder-new/models.svg
```

Snapshots default to demo and never dispatch network work, including with `--live`.

The [research index](../../docs/coder-new/README.md) contains the history and
architecture inputs for the future specification.

Remote delegation runs through the shared Rust `coder-cloud` executor. Enable
Boat Cloud or GCE Cloud in `/plugins`. Each cloud settings screen saves its
execution mode, Boat size and template, admitted credential variable names, and
workspace paths. The CLI configures the same settings; credentials themselves
stay in environment variables. For example:

```sh
printf '%s\n' '{"enabled":true,"credential_names":["OPENAI_API_KEY"]}' | openagents coder plugins configure boat-cloud --stdin
openagents --json coder delegate codex --task "Review this workspace" --on boat --mode integrated --credential-env OPENAI_API_KEY
openagents --json coder delegate microcoder --task "Review this workspace" --on boat --mode coder --template YOUR_CODER_TEMPLATE
openagents coder remote list
openagents coder remote status JOB_ID
openagents --json coder remote follow JOB_ID
openagents coder remote cancel JOB_ID
openagents coder remote steer JOB_ID --message "Focus on the tests"
```

Boat integrated mode uses Boat's agent API; Coder mode runs the same headless
runtime as the terminal. Remote jobs retain their resource, task, events, cursor,
result, and usage under the Coder store's `remote` directory. Choose a stable
`--job ID` to identify a dispatch; repeating it with different arguments refuses.
Use `remote follow` after a disconnect. Selected `--credential-env` values travel
to that run and stay redacted from retained output. Each sandbox starts without
inherited account credentials. Completed jobs stop their sandbox and confirm its
meter is no longer running. `remote status` reports unresolved cleanup.

To use an existing Codex login, explicitly admit `OA_CODEX_AUTH` containing its
authentication JSON instead of `OPENAI_API_KEY`. The runtime writes it to a
private `CODEX_HOME` under `/tmp`, outside the workspace and snapshot. Tokens
inside the JSON are redacted from output and refused in changed-file artifacts.

The same runtime can run in a granted Google Compute Engine pool:

Pool updates publish versioned runtime bundles with their matching CLI. A job
keeps its prepared executable for continuation while new jobs prepare `main`.

```sh
openagents cloud up --hosts 1 --max-hosts 2
openagents coder plugins enable gce-cloud
openagents coder delegate codex --on gce --task "Review the checkout" --credential-env OPENAI_API_KEY
```

Live model tools use placement-qualified agents such as `codex@boat` or
`microcoder@gce`, and preserve an explicitly requested agent. The terminal
shows remote model, tool, usage, and failure events in that agent’s child chat.
Submitting in a completed remote child continues its retained job and workspace.
Turning a plugin off prevents new delegation; the CLI can still inspect or
cancel its retained jobs.

Jobs retain the pool grant and epoch, own a process slot, and reconnect to their
original host. Confirmed host loss ends the job without replaying it. GCE usage
is a labeled estimate of the job's share of the host list price.

Cloud delegation snapshots the selected Git revision and its tracked caller
changes. Repeat `--workspace-path PATH` to limit repository files and
`--include FILE` to admit untracked files. Paths are relative to the repository
root. Credential files and symlink traversal are refused. Large selections
fail before cloud provisioning; `--no-workspace` starts an empty workspace.
Private repositories transfer from the authorized local checkout and need no
GitHub credential on the remote agent.

`remote artifacts JOB` reports the private artifact directory and content
digests: patch, event log, ATIF transcript, and result. `remote apply JOB`
checks the original caller revision and admitted file contents before applying
the patch. `remote continue JOB --task TEXT` resumes the same remote workspace;
Boat integrated agents also keep their conversation ID.
