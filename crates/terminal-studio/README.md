# Terminal studio adapter

This crate projects validated studio snapshots and prepares existing typed
studio operations for the shared workbench. Verse injects its admitted
source. The standalone window uses the existing same-user control socket;
opening or refreshing the page only reads. Every action waits for physical
Enter confirmation and the host checks its right again.

The application remains in `terminal-core`; `terminal-gfx` owns rendering
and generic native helper processes. Neither depends on this adapter.

The sheet shows studio questions and tool approvals separately from shell
proposals. `/answer DECISION TEXT` and `/always DECISION` bind the displayed
decision revision and exact host rule. `/review TASK` reads the exact base,
HEAD, and tree; `/merge`, `/changes TEXT`, and `/reject TEXT` bind those
revisions and require Review independently of Operate. Merge lands a studio
change locally and pushes nothing. The host reports stale or refused actions.

The `studio-acceptance` example (feature `acceptance`) drives this sheet through
a scripted scratch host and retains its receipt, trace, and artifact bytes.
See the [acceptance receipt](../../docs/verse/verification/2026-10-06-studio-workbench/README.md)
for results, isolation, limits, and reproduction.

Declared Studio requests can use `route::LocalHost` with the shared
`openagents_chat::studio` dispatcher and its private locked journal. The
versioned extension in `route_contract::studio` pins the admission, placement,
and exact operation. Direct console intents keep their existing path.
Run the acceptance example with `--routed` to retain goal, decision, review,
stale-review refusal, and merge receipts through the route adapter. Shell
approval grants no Studio right; host policy still owns execution and spend.

The [declared-route receipt](../../docs/verse/verification/2026-10-06-studio-route/README.md)
retains the admitted route and binding beside the host result.
