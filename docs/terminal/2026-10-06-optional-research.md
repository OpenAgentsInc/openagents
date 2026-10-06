# Optional terminal research: results and decisions

Measured on 2026-10-06 on the owner's Apple silicon Mac, in release builds.
Each section answers one optional roadmap issue with its evidence and a
decision. A negative result closes its issue with no default feature
enabled; each section names what would reopen the question.

## Idle scrollback compression (#10692)

**Question.** Does retained host history cost enough memory to justify
compressing idle history pages?

**Workload.** The host's own bounds: 16 terminals (`coder-pty`
`terminals_max`), each keeping 1,000 history lines (`coder-host`
`TERMINAL_HISTORY`) at 120 columns and 40 rows, filled twice over with
colored build-log, listing, test, and warning lines. Run
`cargo run --release -p coder-vt --example scrollback_budget`
([source](../../crates/coder-vt/examples/scrollback_budget.rs)).

**Declared budget.** At most 128 MiB of retained history per host at its
bounds, with no change to parse, attach, or history-read latency.

| Measure | Result |
| --- | --- |
| `Cell` size | 48 bytes |
| History as cell grids, 16 full terminals | 88.4 MiB |
| The same history as encoded `HISTORY` records | 4.31 MiB (20.5 times smaller) |
| Encoded records under DEFLATE level 1 | 0.21 MiB (431 times smaller) |
| Parse per output frame | p50 0.96 µs, p99 2.4 µs |
| DEFLATE one page / restore one page | p50 59 µs / 58 µs; p99 98 µs / 78 µs |
| History read of 1,000 rows | p50 2.2 ms, p99 33 ms |
| Snapshot on attach | p50 2.3 ms, p99 2.5 ms |

Every compressed page round-tripped byte for byte; the example asserts it.

**Decision: do not enable compression.** The worst case, every terminal
the host allows full of history, stays inside the declared budget at
88 MiB, and a typical host with one to three terminals holds 6 to 17 MiB.
Compression would also have to change `Terminal::line` and
`Terminal::scrollback`, which hand out borrowed rows that renderers use,
or keep a host-only second store. Neither is worth it at these numbers.

**What would reopen it.** Raising `TERMINAL_HISTORY` or `terminals_max`
enough to pass the budget (for example 10,000 history lines puts the worst
case near 880 MiB). The cheapest first step then is storing idle history
as the encoded `HISTORY` records the snapshot writer already produces and
restores exactly, which saves 20 times without a new codec; DEFLATE adds
another 20 times at about 60 µs per page.
