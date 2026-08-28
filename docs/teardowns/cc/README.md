# Claude Code teardown series

Date: 2026-08-26. Status: reference analysis. Six reports comparing Claude
Code (the clone at `~/work/projects/repos/cc`) against the OpenAgents coder
(`packages/openagents-cli/src/`, and `crates/coder-lite` for the terminal
front).

These are a source of candidate work, not a plan. A gap named here becomes
real work only through the normal route: an issue, a lever with a named
suite oracle, and a measured delta (`docs/coder/autoimprove.md`,
`docs/coder/runbook.md`). "Claude Code has it" is not a reason to build
something.

| # | Report | Subject |
| --- | --- | --- |
| 01 | [Architecture, lifecycle, query loop, entrypoints](01-architecture-query-loop.md) | Bootstrap, app shell, the turn loop, tool execution |
| 02 | [Tool surface, schemas, sandboxing](02-tool-surface-sandboxing.md) | Tool contract, registry, built-ins, shell safety |
| 03 | [Subagents and fleet orchestration](03-subagents-fleet-orchestration.md) | Task state machines, spawn plumbing, execution backends |
| 04 | [Terminal UI, components, keybindings, theme](04-terminal-ui-components-theme.md) | Renderer, layout, composer, status |
| 05 | [Context window, compaction, memory, skills](05-context-compaction-memory-skills.md) | History, compaction, micro-compaction, memory |
| 06 | [Permissions, cost, telemetry, remote bridge](06-permissions-cost-telemetry-bridge.md) | Permission engine, cost accounting, telemetry egress |

## Reading notes

The largest gaps these reports establish, in the order they cost us, are
carried into `docs/coder/autoimprove.md` §2.3 as structural candidates:
compaction (absent here, ~3,700 lines there), client-side prompt history
(absent; resume replays server events instead), and shell safety (a static
regex refusal table here, a parser and sandbox runtime there).

Reports 04, 05, and 06 each open by correcting their own brief: the
counterpart filenames they were told to compare against do not exist, so
they compare what is actually in the tree. That correction is the useful
part of those three, and it is the behavior the review loop wants — a
report that had invented the missing files would have been worse than no
report.

Where a gap is deliberate rather than missing, these reports do not always
say so. The absence of telemetry egress is a design position, not a
backlog item; the thin permission engine is bounded by the Computer
policy tier it inherits. Read a gap as a question, not a verdict.

Related: the 2.1.195 snapshot identity, and a themes/evolution reading
that splits observed-at-snapshot from changelog speculation through
2.1.250, live in
[`../2026-07-10-claude-code-teardown.md`](../2026-07-10-claude-code-teardown.md)
and
[`../2026-08-28-claude-code-evolution-themes.md`](../2026-08-28-claude-code-evolution-themes.md).
Those documents are not OpenAgents gap reports.
