# OpenAgents Tools

OpenAgents tools are small, focused capabilities the runtime or a model can invoke with typed arguments and receive structured results from. Tools are visible in the chat timeline as ACP `tool_call` events, and are designed to be bounded, auditable, and workspace‑aware.

- [Critique](./critique.md) — Evaluate agent outputs against criteria and provide structured feedback
- [Delegate](./delegate.md) — Route concrete coding tasks to configured agent providers
- [Edit](./edit.md) — Apply structured file edits using a compact patch format
- [Grep](./grep.md) — Bounded workspace search for regex patterns
