# Coder terminal mockup

A standalone Ratatui screen preview for the new Coder terminal. The default
view shows Read, Search, Edit, and Run examples, a diff, four running
delegations, and a composer. Delegations use the same agent names, tasks, and
token counts as the rail. Their magenta diamonds pulse while running commands
show a rotating spinner. Each agent conversation has its own tool calls,
arguments, results, and current activity.
Press Tab to switch to the welcome view.

Edit examples reuse Coder's port of Grok Build's diff renderer and syntax
highlighter. Rust tokens keep their syntax colors on red and green change
backgrounds, with line numbers and wrapping. The main view shows a cursor
change; the `grok-build` conversation shows a larger style change.

Mock plugin calls show their qualified operation, arguments, result, and running
state. The main conversation uses `terminal-inspector.layout.inspect` and
`palette-audit.colors.check`. Agent conversations also use keyboard and
conversation audit examples. These fixtures do not load or invoke plugins.

Press F2 or enter `/plugins` to manage the example **OpenRouter BYOK** plugin.
Space toggles its enabled preference; Enter opens connection settings. The
settings screen has a masked **OpenRouter API key**, an optional model ID, and
the fixed direct endpoint `https://openrouter.ai/api/v1`. Tab moves between
fields and actions; Enter selects an action; Esc cancels or returns to chat.
Save and cancel preserve the distinction between the enabled preference and
configuration: an enabled plugin without a key shows **Setup required**;
adding a key shows **Configured**, with verification still pending. Removing
a key takes effect when you save. Disabling retains the configuration.
These screens keep preferences in memory and discard the entered key after
save or cancel. They do not validate keys, call OpenRouter, or activate a
provider. OpenRouter's own upstream-provider BYOK settings are separate from
this example's OpenRouter API key. See [OpenRouter authentication](https://openrouter.ai/docs/api_reference/authentication),
the [API reference](https://openrouter.ai/docs/api_reference/overview), and
[OpenRouter BYOK](https://openrouter.ai/docs/guides/overview/auth/byok).

```sh
cargo run -p coder-new
cargo run -p coder-new -- --welcome
cargo run -p coder-new -- --plugins
cargo run -p coder-new -- --plugin-settings
```

Type a draft, move with Left/Right or Home/End, and edit with Backspace/Delete.
Alt+Enter adds a newline. Enter appends a local preview message. Bracketed paste
inserts text without sending it. PageUp/PageDown scroll the conversation;
Ctrl+C quits. The preview makes no network requests, runs no tools, and saves
no messages. All conversation and agent values are sample data.

The composer is a plain `❯` input with a blinking block cursor between
edge-to-edge horizontal rules. The four rows below it show `claude-code`,
`codex`, `devin-cli`, and `grok-build`. Agent
names and current tasks occupy separate aligned columns, with elapsed time
beside the token counts on the right, such as `1h 12m 38s · ↓ 8.2k tokens`.
The token-count column grows to the widest count so the separators stay aligned.
The mock timers advance once per second while the preview runs. Narrow terminals
truncate task text, shorten the token label, and omit elapsed time when needed
to preserve the agent names.
Press Down to select the first agent, then Up/Down to switch conversations.
Selection loads that agent's demo messages immediately. Up from the first agent
or Esc returns to the main conversation. Each conversation retains its own
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
cargo run -p coder-new -- --plugins --snapshot > docs/coder-new/plugins.svg
cargo run -p coder-new -- --plugin-settings --snapshot > docs/coder-new/plugin-settings.svg
```

The [research index](../../docs/coder-new/README.md) contains the history and
architecture inputs for the future specification.
