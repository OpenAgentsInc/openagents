# Coder runtime

Implemented execution, supervision, terminal behavior, and evidence contracts.
Each page states the supported path and its limitations. For deployment status
and unfinished suite acceptance, use the [migration tracker](../migration-status.md).

[Documentation index](../README.md)

| Document | Topic |
| --- | --- |
| [delegate](delegate.md) | Delegation |
| [delegate-door](delegate-door.md) | Shared turn delegation, Microluna and CLI executor choices, and explicit fallback policy |
| [Cloud fallback](cloud-fallback.md) | The OpenAgents cloud as the no-setup last provider: its path, limits, and typed refusals |
| [repository-evidence](repository-evidence.md) | Repository source references |
| [shell-loop](shell-loop.md) | The shell loop |
| [subprocesses](subprocesses.md) | Subprocess supervision |
| [Privacy prompts](privacy-prompts.md) | Why nothing OpenAgents runs makes macOS ask for music, photos, or documents |
| [terminal](terminal.md) | The terminal's lifecycle, lanes, and scrollback |
| [traces](traces.md) | Traces |
| [Task ownership](task-owner.md) | Explicit execution, recovery, corrections, independent checks, retained artifacts, and paged views |
| [Knowledge evidence](knowledge-evidence.md) | Complete intake, exact comparison identities, missing costs, and historical screening limits |
| [Knowledge bundles](knowledge-bundles.md) | Immutable EXT snapshots and explicit private model disclosure |
| [Knowledge studies](knowledge-studies.md) | Frozen assignments, per-attempt receipts, and prospective report bookkeeping |
| [Repository Microcoder](microcoder-repository.md) | Explicit model admission through the shared task owner |
| [The Devin route](devin.md) | A repository turn handed to the local Devin CLI over ACP |
| [The OpenCode route](opencode.md) | A repository turn handed to OpenCode over ACP, and OpenCode as a delegate executor |
| [The Grok Build route](grok.md) | A repository turn handed to the local Grok Build CLI over ACP |
| [Free agent labor](free-labor.md) | Recoverable free orders, bounded provider execution, separate buyer checks, and acceptance |
| [Frozen task context](frozen-task-context.md) | Exact knowledge bytes, source lineage, scoped instructions, and protected independent checks |
| [Portable host](portable-host.md) | Digest-pinned installation, rollback, and one-shot task service packaging |
| [Host service](host-service.md) | The resident host as a launchd agent or systemd user unit, trial updates with snapshot rollback, and the host descriptor |
| [Chat load benchmark](chat-load-benchmark.md) | How fast the phone's Coder chats load and answer, phase by phase, and where the time goes |
| [Nostr task control](nostr-task-control.md) | Owner-installed task mapping, scoped pairing and commands, bounded private views, and replay |
