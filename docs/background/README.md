# Background processes

Background processes are durable, user-defined rules that the OpenAgents host
runs on its own: a trigger, a condition, and typed actions, defined in
conversation and then executed deterministically, with Jev (System One) only
for bounded judgments.

| Document | What it covers |
| --- | --- |
| [Background processes spec](2026-10-02-background-processes.md) | The rule model, safety, surfaces, the disk cleanup monitor as the first built-in, other useful processes, and the phased plan. |
| [Disk cleanup as a plugin](2026-10-02-disk-cleanup-plugin.md) | Background plugins: what a plugin may ask for, what the host enforces, off until turned on per computer, and how the disk cleanup plugin was made. |
| [The kache compile cache](kache.md) | Why kache's cap needs `gc_evict_shared`, what "Another GC is already running" means, and `openagents background kache`. |
