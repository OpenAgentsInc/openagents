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

## Read-only auto-run (#10695)

**Decision: implemented, off by default.** `terminal_core::autorun` keeps a
per-workspace opt-in in `~/.openagents/terminal/autorun.json` (owner-only,
replaced atomically, never inherited from pairing, shares, or world
membership). The prefix, then `R`, turns it on for the focused pane's
directory or off where it is on. `Book::auto` runs a proposal without Enter
only when it is still pending, bound to the pane's current binding, inside
an admitted root, classed `read_only` by the shared effect boundary, and one
plain command without substitution, expansion, redirection, pipes, or
sequencing. Anything else stays pending for Enter, and recovery never
replays an execution. Coder worktree policies and studio merge approval do
not read the setting.

## Local ambiguous-line classifier (#10693)

**Question.** Does a local decision model beat, or usefully augment, the
structural fallback in `terminal_core::route::classify` for prompt lines
whose first word the shell resolves?

**Contract and cases.** The held-out set
[`line-kinds.json`](../../crates/terminal-core/fixtures/line-kinds.json)
(`openagents.terminal-line-kinds.v1`) holds 68 lines, each with the shell's
report on its first word and what the person meant: 36 ambiguous, 6
quoted, 6 alias or function, 4 long, 5 malformed, and 11 adversarial
cases. It was written for this measurement and never used to tune the
rules. The question asked the model is one `choice`, `command` or
`request`, over the line and its first-word kind; the line never leaves
the computer.

**Declared budget.** A judgment must arrive within 150 ms, the preview's
debounce, and must beat the structural answer on the lines the rules mark
unsure; it never approves a command.

| Decider | All 68 | The 24 the rules mark unsure | Warm latency |
| --- | --- | --- | --- |
| Structural rules | 57 (84%) | 20 | under 1 µs |
| Laya, English checkpoint | 43 (63%) | 14 | p50 2.8 s, p99 3.9 s |
| Laya, typed-decisions checkpoint | 41 (60%) | 13 | p50 2.6 s, p99 3.5 s |

Laya ran in process on the CPU
([`line_kind`](../../crates/laya/examples/line_kind.rs) example); its
confidence never passed 0.71 and was mostly under 0.2. Lev, Apple's
on-device model, already measures 305 ms to 1.7 s per answer
([Lev surface notes](../lev/apple-fm-surface.md)), and Kev's decoders are
larger than Laya's encoder, so neither can meet the budget either; they
were not run.

**Decision: no model in the route.** Every local model tried is less
accurate than the rules, on all lines and on the unsure ones, and slower
than the budget by an order of magnitude or more. The structural fallback
stays the whole decision for rule 4, the `?` marks on unsure routes stay,
and the explicit `# ` prefix still wins. Remote Jev stays off.
`crates/terminal-core/tests/line_kinds.rs` holds the rules to their
measured floor of 57.

**What would reopen it.** A local checkpoint fine-tuned on line kinds that
answers within the budget on the owner's computers.

## Superlogical and libghostty interop (#10696)

Sources were read on 2026-10-06. Only public pages were used; nothing
unpublished was inspected or copied.

**Public facts.**

- Superlogical announced its company on 2026-07-29 to build a server-side
  terminal multiplexer whose sessions hold several terminal blocks, outlive
  the client, and reconnect from the web, macOS, and iOS, with live sharing
  ([Runtime Wire, 2026-07-29](https://runtimewire.com/article/mitchell-hashimoto-superlogical-terminal-multiplexer);
  [The Register, 2026-07-31](https://www.theregister.com/a/5281970)).
- Its site publishes no protocol, API, SDK, documentation, or source. It
  offers a waitlist for the multiplexer beta and promises to announce "any
  OSS releases along the way" ([superlogical.com](https://superlogical.com)).
- Superlogical says it builds on the MIT-licensed Ghostty components and
  contributes terminal work upstream (Runtime Wire, above).
- libghostty-vt was announced as "public alpha (not promising API
  stability)" on 2025-09-22
  ([libghostty is coming](https://mitchellh.com/writing/libghostty-is-coming)).
  Its current documentation lists a terminal snapshot module ("encode and
  incrementally restore terminal state") and an idle scrollback compression
  example, and states that "the API is not yet stable. Breaking changes are
  expected"
  ([libghostty documentation](https://libghostty.tip.ghostty.org/)). It
  publishes no versioned snapshot wire format.

**Unresolved.** How Superlogical clients attach, what crosses its wire,
whether that wire carries libghostty snapshots, its concurrent input and
resize policy, and which side effects its server owns are all unpublished.

**Contract map.** Each part of the publicly described model already has a
Rust counterpart here; the right column is the only interop claim this
page makes.

| Public model | This repository | Compatible direction |
| --- | --- | --- |
| Durable sessions of several terminals | NIP-TERM sessions extension; host session records (#10652) | Shape only |
| Raw bytes to every client, each client parses | NIP-TERM base frames with sequence numbers; `coder-vt` in every client | Shape only |
| Restore parsed state on join, then history | NIP-TERM snapshot extension (`TERMINAL`, rows, `CONTINUATION`, `READY`, history, `FINISH`, each record tag, length, and CRC32C) | Shape only; libghostty's byte format is unpublished |
| Server-owned side effects | NIP-TERM effects extension; `coder_vt::Authority` | Shape only |
| One person types at a time | NIP-TERM typist extension | Shape only |
| Live sharing | NIP-TERM shares extension (#10676) | Shape only |

**Inference.** Because every contract matches in shape, a translator would
be an adapter at the transport edge, not a redesign, if a public protocol
appears.

**Decision: no adoption and no spike.** No published protocol carries
libghostty snapshots, and libghostty's snapshot encoding is unstable and
unspecified, so there is nothing a pure-Rust codec could target and no
public fixture to measure against. Wire compatibility stays unavailable;
no Zig build, C library, or proprietary protocol enters the product. This
blocks no roadmap work.

**What would reopen it.** Superlogical publishing its protocol, or
libghostty publishing a versioned snapshot format with fixtures. Then
measure a Rust decoder against those fixtures for memory, build cost, and
latency before proposing an adapter.
