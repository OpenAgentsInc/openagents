# Research for the new Coder TUI

These documents establish the history, current implementations, and
architectural recommendations for a new specification. They are research
inputs, not a finished TUI spec or claims of tested replacement quality.

The first [Ratatui mockup](../../crates/coder-new/README.md) runs with
`cargo run -p coder-new`. It uses the exact Grok Build Grok Night palette.
The [conversation preview](mockup.png), [welcome preview](welcome.png), and
[background-agent preview](agents.png) are exported from the same render
function as the terminal.

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
