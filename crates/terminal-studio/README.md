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
