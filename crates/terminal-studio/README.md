# Terminal studio adapter

This crate projects validated studio snapshots and prepares existing typed
studio operations for the shared workbench. Verse injects its admitted
source. The standalone window uses the existing same-user studio CLI;
opening or refreshing the page only reads. Every action waits for physical
Enter confirmation and the host checks its right again.

The application remains in `terminal-core`; `terminal-gfx` owns rendering
and generic native helper processes. Neither depends on this adapter.
