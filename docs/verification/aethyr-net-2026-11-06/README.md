# Aethyr networking extraction qualification

Issue: [#12845](https://github.com/OpenAgentsInc/openagents/issues/12845).
All times in this record use America/Chicago.

The delivered interfaces and actual consumer calls are listed in
[`aethyr/net/README.md`](../../../aethyr/net/README.md). The second realm is
Orbit League's additive v2 input-authoritative profile. Its existing
reported-car/server-ball endpoint remains a separate presence profile.
No production deployment was performed.

## Regressions and checks

The retained red and green logs cover these defects:

- A distant party member's health-only channel also exposed a position.
  The sender now carries HP/MAX masks to the wire. The page retains party
  health separately and does not create a visible body from that channel.
  A later full body or removal correctly changes that state.
- A live but uninitialized world could step. Both realm constructors now
  select the shared initialization gate before admission or stepping.
- The legacy Orbit endpoint accepted client transforms. The new versioned
  endpoint admits bounded controls, ignores body reports, coalesces commands
  by its server beat and acknowledges only consumed controls.
- Permission changes could use a baseline containing fields the receiver
  never received. Narrowing invalidates in-flight receipt state; widening
  sends an absolute row. A byte-budget refusal restores the previous state.
- An initial whole-raid check exceeded the existing traversal bound after
  extracting replica decisions. Skipping empty slot copies fixed it. The
  same check passes at 126,492,845 visits against 134,217,728; no traversal
  allowance was raised.
- Authoritative Orbit presentation initially retained offline spin state
  and an inaccurate lost-room message. Presentation now derives spin from
  the accepted match clock, clears lost controls and describes rejoining.

Recorded full suites: Everglade page 786/786, room 708/708 and Orbit 181/181.
The full page run finished in 14m27s. The room/Orbit runs precede the additive
diagnostics and read-only probe checks; those checks pass separately. Later
presentation regressions also have separate red/green logs. Current physical
builds run the loaded authority checks before emitting C. The focused wire
suite passed 37/37 after the extraction's occupied-slot optimization.
The final independent Orbit JSON fixtures pass through `orbit-wire-profile`.

## Mutation gates

These are root-only mutation runs with dependency checks retained. Files
were unchanged after their passing run; passing gates were not repeated.
No mutant was marked equivalent.

| Authority | Killed | Survived |
| --- | ---: | ---: |
| `aethyr/net/realm.coax` | 64 | 0 |
| `aethyr/net/command.coax` | 50 | 0 |
| `aethyr/net/replication.coax` | 106 | 0 |
| `aethyr/net/row.coax` | 32 | 0 |
| `aethyr/net/record.coax` | 154 | 0 |
| `aethyr/net/session.coax` | 22 | 0 |
| `aethyr/net/diagnostics.coax` | 68 | 0 |
| `orbit-session.coax` | 114 | 0 |
| `orbit-snapshot.coax` | 284 | 0 |
| `room-input-policy.coax` | 6 | 0 |
| Total | 900 | 0 |

## Build bounds

The first Orbit release measured 291,597 bytes, exceeding its old 290,000-byte
budget by 1,597 bytes. Its budget is now 300,000 bytes for the intentional
addition of authoritative input/snapshot codecs alongside legacy compatibility.
The build and budget change are in the same issue's commit sequence. Everglade's
page and worker budgets remain 2,150,000 and 1,060,000 bytes.

The first Everglade page built in 14m12s at 2,027,996 bytes. The first worker
built at 1,029,953 bytes. The sequential native builds were stopped before
15 minutes. Their completed room C was retained; the successful worker build
then supplied the qualified six C/header units through its existing retained-C
mode. Native compilation and linking use the strict math flags, symbol
localization order and host effects from `coax/web/room/build.sh`. The native
start check reports zero fields off their declared defaults. Artifact hashes
are in `oa-12845-final-artifacts.sha256`; retained-C keys are in `sources.txt`.

The final presentation fix built Orbit at 292,500 bytes against 300,000.
Its release build took 2m24s. Rebuilding the changed room unit and reusing
the other five qualified units took 6m9s; the worker remains 1,029,953 bytes.
The native host was relinked from those current C units and again passed its
start check. The old and current generated room headers are byte-identical;
only the room unit's retained-C closure key changed.

Native compilation uses the same qualified Linux Coax 0.51.2 output as the
worker (`088782b2838c3119eeb5b352f9ef36144d08171da8b910e1a0a79d93e401ee4e`),
with Apple's host C compiler and the room build's strict math flags. Local
Coax checks use the verified Mac 0.51.2 binary
`61bb0a8213647b7f2fdd1f169709000be3fd52c6eb55c5359684e42971c4f5a0`.

## Everglade consumer recordings

The local, 100 ms and 250 ms sessions include three background players and
the siege raid. The 250 ms run retains all 32 probes, including both stable
collapses, the level transition and hotbar transition. The separate 250 ms
forest run observes fire on both pages and converges on burnt count 1 and
settled digest 897021801. No offline clock stall or dropped beat was recorded.

| Profile | Worker p95 at session end (µs) | Largest native window p95 (ms) |
| --- | ---: | ---: |
| Local | 735 | 1.11 |
| 100 ms | 815 | 1.30 |
| 250 ms | 770 | 0.97 |

The lifecycle envelopes in the unchanged source are 8,000 µs native and
11,000 µs worker. These local window measurements do not establish the
separate optimization targets in #12952 or a deployed fleet envelope.

A signed 64-player load across four 16-seat rooms also passes. Every socket
stays open, the slowest receiver gets 30 state frames/s, ACK p95 is 34–35 ms,
and state drops, backward ACKs, stale final ACKs and blind peers are zero.
The raw load receipt retains ten gameplay refusal frames per room; it does
not claim every spell was accepted.

### Input and report audits

An initial 100 ms attempt failed the position comparison. Its key edges
landed on different input steps between pages. The added outgoing-input
trace confirms that rendered frames can batch across scheduled edges.
Exact stream equality is recorded as a diagnostic; the original position,
correction and time-window limits stayed unchanged. The second recorded
100 ms attempt passed every original comparison but failed an extra exact
input-equality audit, before that audit became informational.

The next 100 ms gameplay/raid run passed, but an added diagnostic row filled
the bounded report and displaced the hotbar result. The required hotbar
calculation still ran and its JSON receipt was saved. `raid100-replay.coax`
independently checks that real receipt: LEVEL 1 hash 222308816, LEVEL 2 hash
237252350, distinct from full-kit hash 864523051; kit ready and level 2.
Its passing replay is retained. Diagnostics now stay outside the 32-row
report, and overflow fails explicitly. The failing overflow regression and
the full 143-check passing driver suite are retained. The final 250 ms run
uses that corrected report and includes its hotbar result directly.

Input traces include their sequence origins so an auditor can normalize the
actual sent records and distinguish unequal input intervals from simulation
differences. The 100 ms final trace and 250 ms trace are retained beside the
raid receipts. Earlier unsuccessful attempts remain visible in this directory.

## Orbit browser receipts

The named-room two-page checks pass locally and with 250 ms simulated round
trip. Both receivers have the same complete tick and the same public world;
only owner fuel differs. Real keyboard controls move the authoritative car
and advance consumed ACKs. A forged legacy body report has no effect. The
displayed own pose matches the raw server frame of its exact accepted tick.
Closing the second page removes its body from the next complete snapshot.
The current native host also passes the quick-2v2 variant at 250 ms: both
server bots move, all four cars share one world, and a bot retains the seat
after its human disconnects. No page error was recorded. The saved PNG was
visually inspected: arena, car, ball, score, timer and fuel are visible.

The native host consumes the shared observation interface for its local log.
The recorded Orbit line reports 60 Hz, two substeps, four seats, 765 consumed
controls and 783 generated snapshots. Everglade's recorded lines report
30 Hz and one substep, with unavailable counters explicitly −1. The signed
room-stats format is unchanged.

### Existing offline bot-balance failure

`coax run apps/openagents-web/orbitleague-sim.coax` prints `FAILS` even though
the command exits 0. The box novice balance exceeds its existing bound:
novices score 23, bots 42 and one bot match reaches 6 goals. The unchanged
`origin/main` baseline, `f255037e9c`, produces identical output for every
match and the same failure with the same compiler. The first 50 output lines
of both logs hash to
`ff62519ab8e1d3ca2530bb0a29a868fc64c91989c3f89ec9468e60022a629895`.
Both complete logs are retained. This is a pre-existing offline balance
limitation, not a passing gameplay gate. The extraction's active-car masking
preserves those legacy match outcomes exactly; the networked v2 acceptance
uses the room/protocol checks and real two-page rehearsals described above.

## Scope

Everglade keeps its established generation, reconnect, travel, party, owner
fence and checkpoint policies. Orbit selects no durable state and no anonymous
seat resume. Complete absolute snapshots are an explicit small-room choice;
the Orbit adapter does not allocate unused delta or removal queues.
Neither the item-ledger model nor every modeled travel workflow is claimed
as a new product adoption. Books remain paused under #12610; this record and
the delivered-call inventory do not promote teaching chapters.
