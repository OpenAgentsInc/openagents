# Research for the new Coder TUI

These documents establish the history, current implementations, and
architectural recommendations for a new specification. They are research
inputs, not a finished TUI spec or claims of tested replacement quality.

The [Ratatui terminal](../../crates/coder-new/README.md) runs with
`cargo run -p coder-new`. It uses historical USGC accents with Coder's shared
near-black background and the existing white and gray styling.
The [conversation preview](mockup.png) and [welcome preview](welcome.png)
are exported from the same render function as the terminal. Four sample agent
rows beneath the input show their current tasks, elapsed time, and token counts.
Up/Down loads their demo conversations; Esc returns to the main conversation.
The [selected conversation preview](agent-conversation.png) shows the active row.
The main transcript includes Read, Search, Edit, and Run examples and four
delegation components that match the rail. Magenta diamonds pulse on running
delegations; selected agent conversations show their own tools and command
spinners. Mock plugin calls show qualified operations, arguments, results, and
progress in the same transcript. These are local presentation fixtures.
Edit examples reuse the Grok Build diff renderer and syntax highlighter,
including line numbers and red and green change backgrounds.

The [plugin manager](plugins.png) ships Microcoder, Jev, OpenAgents CLI, and ACP
Subagents enabled by default, and OpenRouter BYOK off unless a startup key is
available. [OpenRouter settings](plugin-settings.png) and Jev settings provide
masked key entry and key checks. The [Jev form](jev-settings.png) and
[ACP editor](acp-settings.png) are also exported from the terminal renderer.
Live OpenRouter chat executes enabled plugin
tools and returns their results to the model; local Microcoder uses existing
model logins. ACP settings define named local agent executables. Preferences
and keys persist in private files under `~/.openagents/coder-new/`. Open plugins with F2
or `/plugins`, or start with `--plugins` or `--plugin-settings`.
Interactive runs start [live](live.png); `/demo` toggles the local fixtures.
The [slash picker](slash-commands.png) filters commands above the input as you
type, following the existing OpenAgents terminal. Snapshots dispatch no requests.

1. [Claude Code replacement history and current terminal survey](claude-code-replacement-history.md)
   catalogs the retained attempts and compares today's Coder and OpenAgents
   terminal surfaces.
2. [Typed plugins, Nostr, and payments](plugin-architecture-carry-forward.md)
   recommends what to keep from the TypeSafe research, plugin contracts,
   signed registry, and payment implementations, including their current gaps.
3. [DeepSeek Harness lessons](deepseek-harness-lessons.md) examines a pinned
   local clone of its plugin composition, lifecycle, sessions, inbox, and
   execution architecture.

The recommended direction is an **everything is a plugin** composition model
with explicit service interfaces, a shared task owner, typed workflows and
judgments, a Nostr registry, and host-owned payments. Built-ins and installed
extensions share interfaces while retaining their different trust boundaries.

[NIP-REG](../../nips/openagents/NIP-REG.md) specifies the proposed curated plugin
registries: exact publisher releases in inert EXT catalogs, with Nostr and
GitHub/HTTPS sources carrying the same signed content. The registry client is
not implemented. Plugin settings and keys stay in private host storage;
registry discovery does not install, enable, or admit a plugin.

The new specification should settle task/queue/stop semantics, plugin
activation and replacement, durable recovery, and payment admission before
choosing layouts and shortcuts. Each research document separates implemented
behavior, retained measurements, projections, and proposals.
