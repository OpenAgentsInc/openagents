# Coder terminal

A Ratatui terminal with direct OpenRouter chat and a separate demo mode.
Interactive runs start in live mode. `/demo` toggles between live and demo;
`--demo` starts with the fixtures. Demo mode shows Read, Search, Edit, and Run
examples, a diff, four running
delegations, and a composer. Delegations use the same agent names, tasks, and
token counts as the rail. Their magenta diamonds pulse while running commands
show a rotating spinner. Each agent conversation has its own tool calls,
arguments, results, and current activity.
Press Tab with an ordinary draft to switch to the welcome view.

Type `/` to see commands above the input, following the existing OpenAgents
terminal's slash suggestions. The command and description occupy separate
columns. Typing filters the list; Up/Down selects, Tab completes, Enter runs,
and Esc dismisses it. The supported commands are `/demo`, `/plugins`, `/models`,
and `/help`. `/models` appears when a model provider plugin is enabled.

Edit examples reuse Coder's port of Grok Build's diff renderer and syntax
highlighter. Rust tokens keep their syntax colors on red and green change
backgrounds, with line numbers and wrapping. The main view shows a cursor
change; the `grok-build` conversation shows a larger style change.

Mock plugin calls show their qualified operation, arguments, result, and running
state. The main conversation uses `terminal-inspector.layout.inspect` and
`palette-audit.colors.check`. Agent conversations also use keyboard and
conversation audit examples. These fixtures do not load or invoke plugins.

Press F2 or enter `/plugins` to manage **OpenRouter BYOK**.
Space toggles its enabled preference; Enter opens connection settings. The
settings screen has a masked **OpenRouter API key**, an optional model ID, and
the fixed direct endpoint `https://openrouter.ai/api/v1`. Tab moves between
fields and actions; Enter selects an action; Esc cancels or returns to chat.
In live mode, **Test API key** checks the entered or saved key with
`GET /api/v1/key`. Saving a key also checks it; successful checks show
**Verified**, while errors report their cause. The enabled preference stays
separate: turn the plugin on to send chat messages. The default model is
`openrouter/free`, including when you leave the model field blank.
Replies stream from `/api/v1/chat/completions`
on a background worker, with prior live messages included. Esc stops a reply;
failed requests are not retried automatically.

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
Live plugin preferences and the API key survive restarts in
`~/.openagents/coder-new/plugins.json`. The directory uses `0700` permissions;
the file uses `0600`. Saves replace the file atomically, and failed saves keep
the previous settings and editable draft. Connection verification runs again
when you test the restored key; verification status is not saved.
The editable key draft clears on save or cancel. Keys stay redacted in the UI
and errors. Demo settings and conversations last only until you quit, and demo
mode makes no requests or settings writes. Switching modes keeps their
preferences, drafts, and transcripts separate and cancels pending network work.
OpenRouter's own upstream-provider BYOK settings are separate from
this plugin's OpenRouter API key. See [OpenRouter authentication](https://openrouter.ai/docs/api_reference/authentication),
the [API reference](https://openrouter.ai/docs/api_reference/overview), and
[OpenRouter BYOK](https://openrouter.ai/docs/guides/overview/auth/byok).

```sh
cargo run -p coder-new
cargo run -p coder-new -- --demo
cargo run -p coder-new -- --welcome
cargo run -p coder-new -- --plugins
cargo run -p coder-new -- --plugin-settings
cargo run -p coder-new -- --models
```

Type a draft, move with Left/Right or Home/End, and edit with Backspace/Delete.
Alt+Enter adds a newline. Enter sends a live message or appends a demo message.
Bracketed paste
inserts text without sending it. Trackpad scrolling and PageUp/PageDown scroll the conversation;
Ctrl+C quits. The demo tools and agents remain presentation fixtures; live mode
provides OpenRouter chat, not tool execution or agent delegation.

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

The base background uses Coder's shared near-black color (`#0a0a0a`). The other
colors are exact 24-bit RGB values from Grok Build's default Grok Night theme,
including the focused composer border. Use a truecolor terminal to
display them exactly. [NOTICE](NOTICE) records the source commit, `SOURCE_REV`,
upstream source file, and license.

Export the actual Ratatui buffer without an interactive terminal:

```sh
cargo run -p coder-new -- --snapshot > docs/coder-new/mockup.svg
cargo run -p coder-new -- --welcome --snapshot > docs/coder-new/welcome.svg
cargo run -p coder-new -- --live --plugins --snapshot > docs/coder-new/plugins.svg
cargo run -p coder-new -- --live --plugin-settings --snapshot > docs/coder-new/plugin-settings.svg
cargo run -p coder-new -- --live --snapshot > docs/coder-new/live.svg
cargo run -p coder-new -- --models --snapshot > docs/coder-new/models.svg
```

Snapshots default to demo and never dispatch network work, including with `--live`.

The [research index](../../docs/coder-new/README.md) contains the history and
architecture inputs for the future specification.
