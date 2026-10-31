# First compiler-owned transitive effect inventory

The scoped [receipt](coax-effects-inventory-2026-10-31.json) qualifies
`coax deps --effects [--proc NAME]`, described in
[the inspection contract](../../coax/EFFECTS.md). It is a compiler prerequisite
for robotics resource contracts, not completion of R11 or admission of R14
`#bounded`.

Coax classifies raw operation facts, finds transitive callees, detects reachable
recursion and chooses deterministic shortest witness paths. The native host
observes the resolved checked graph and prints results. It does not parse call
names from text or own an alternate closure/effect policy. Source paths and
instance keys keep generic instantiations and loaded declarations distinct.

Seven authority checks kill all 199 root mutants. An independent oracle uses
Floyd-Warshall reachability and enumerated simple paths, rather than the
implementation's BFS/topological removal. All 247 graph/refusal cases pass in
debug, release and ASan+UBSan, with identical output bytes. Thirteen focused
CLI methods pass; twelve also pass under the host-ASan+UBSan compiler. A seeded
private extractor that omits defer bodies is rejected by the foreign-path
regression. An owned program's ordinary product C is byte-identical to the
existing compiler output.

Actual consumer inspections are retained:

| Consumer | Reachable procedures | Recursion | Reported obligations |
| --- | --- | --- | --- |
| [Panda `pc_step`](coax-effects-2026-10-31/panda-pc-step.json) | 35 of 53 | No | Trap logging/I/O, may-trap, borrowed read/write, intrinsic implementation and other unknown effects |
| [Verse `verse_orbit_follow`](coax-effects-2026-10-31/verse-orbit-follow.json) | 19 of 81 | No | Intrinsic implementation and unknown effects |

The Panda report is a useful refusal to call this existing fixed-array function
compiler-proven bounded: its actual native trap paths can print, intrinsic
resource implementations remain unqualified, and borrowed region/lifetime facts
are incomplete. The Verse path has no inferred region access through borrowed
bases, but its math implementation still has unfulfilled resource obligations.
Neither inspection ran the robot, simulator or learned network.

This is a private focused compiler build, bound to the actual generated C,
header, object, binary, authored source hashes and compiler-input fingerprint.
It is not a production-release publication. No full bootstrap or unrelated
explicit mutation suite was rerun; ordinary compiler emission retained its
mandatory authority checks. Each native command remained below the 15-minute
limit.

Foreign/resource summaries, exact aliases/regions, scratch escape, loop and
stack bounds, typed units/clocks and observations of actual allocation/blocking
remain outstanding. Unknown never means absent. The current source hashes do
not substitute for descriptor or model artifact authentication.
