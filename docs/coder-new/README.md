# Research for the new Coder TUI

These documents establish the history, current implementations, and
architectural recommendations for a new specification. They are research
inputs, not a finished TUI spec or claims of tested replacement quality.

The [Ratatui terminal](../../crates/coder-new/README.md) runs with
`cargo run -p coder-new`. It uses the Grok Build Grok Night palette with Coder's
shared near-black background.
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

The [plugin manager](plugins.png) and [OpenRouter settings](plugin-settings.png)
provide an enabled preference, configuration status, masked API key entry, an
optional model ID, and a direct OpenRouter endpoint. Live mode checks keys and
streams chat replies; keys stay in memory until you quit. Open plugins with F2
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

The new specification should settle task/queue/stop semantics, plugin
activation and replacement, durable recovery, and payment admission before
choosing layouts and shortcuts. Each research document separates implemented
behavior, retained measurements, projections, and proposals.
