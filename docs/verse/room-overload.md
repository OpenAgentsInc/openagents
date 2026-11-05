# Everglade room overload profile

Issue #12841. This records the single-owner room's work classes and the bounds
enforced so far. A bound on storage is separate from CPU isolation; measured
work and startup/content-warming receipts are in the room-work record. Remaining
downstream effects and integrated gameplay qualification stay open.

| Work | Admission and reservation | Full/late/outage outcome |
| --- | --- | --- |
| Movement intents | Per-socket frame/byte token buckets, fixed input queue and authoritative time credit | Extra input is discarded/counted; sustained abuse closes that socket; a faster sender earns no faster movement |
| Cast intents | Two outstanding reservations per seat, 32 per room | Existing `r SEQ busy` before cast order/cooldown is spent |
| World ability service | Two starts and four ends per seat, 96 entries | Refuse extra starts before cooldown/resources; refuse extra ends before ending concentration; preserve movement/end ordering |
| Door/talk/boat interactions | One nonnegative-ID and one negative-ID request per seat, 32 entries | Existing `r SEQ busy` for extras; positive-ID floods cannot consume the talk release/boat nearest-toggle class or another seat's work |
| Simulation/serialization | Each room earns 45 us/ms, with a 3,000 us burst; a tick requires 1,500 us credit | Charge actual wall duration; an indebted room defers itself, counts/discards queued movement and earns no deferred movement; new casts/seats receive busy/full before order or fencing |
| Waiting cast intents | Two per seat and a 533 ms monotonic deadline | Busy refusal before start/cooldown even if simulation time is deferred; bars/flight and accepted lifecycle work are retained |
| World damage input | One aggregate per seat per tick, 16 reserved mailboxes | Repeated complete reports replace their seat's snapshot; resolved once after heals; no overflow-to-immediate-damage path |
| Raid deaths and XP | One reserved result per physical creature slot, 96; pending result fences raid reuse | Consume once before reinforcements; preserve the dead source for retry if recording cannot reserve space; explicit event clear settles results first |
| Entity state | Bounded row budget and maximum-age selection | Replace/coalesce; old slow-link state is skipped; resnapshot after catch-up |
| Lifecycle and outcomes | Separate lifecycle allowance and reserved outcome storage | Lifecycle deferred; missed town outcomes counted and repaired by complete town state |
| Presentation cues | Per-seat cue allowance | Disposable cues discarded and counted |
| Character checkpoint profile 2 | 256 reserved character entries; immutable in-flight plus latest capture; one HTTP request | Refuse entry 257; never expire/evict unsaved current-owner data; initial play waits; affected encounter pauses inside 30 s of accepted gameplay; retry until accepted |
| Legacy vitals | Historical bounded 128-entry/40-line outbox | Existing expiry/eviction counters and legacy weaker recovery policy; not a profile-2 acceptance |
| Control/publication fetch | One admitted request per lane | Keep last accepted control/world while combat continues; late results refused; failed publication remains wanted and retries at least one second apart |
| Directory/statistics | Separate replaceable telemetry lanes | Refuse/coalesce fresh reports while busy; later report replaces stale telemetry |

Native HTTP uses six independent lanes: control, world, legacy vitals, complete
checkpoints, directory and statistics. Each has one serial helper and a bounded
atomic local datagram transport; the simulation never waits for a request write
or an HTTP reply. Helpers are forked before simulation allocation. A lane admits
one request identity, retires it at 15 seconds and rejects late/replaced replies.
The SDK's curl timeout is 5 seconds, or 10 for a world fetch. A failed request
waits at least one second before another offer. Config and response sizes are
bounded; an oversized response is failure, never a successful prefix. A stalled
dependency cannot create an unbounded list of curl children or block another
request class. Socket send-buffer capacity remains bounded even if the OS stops
scheduling a helper. Each helper exits when its parent leaves.

HTTP transport success only releases that lane. The durable producer separately
requires the exact version/owner/process/revision acceptance receipt. If the
platform refuses an offer, that producer receives immediate failure and retains
its immutable capture for retry. The final-save helper remains alive during a
process-group SIGTERM; forced loss follows the declared resume profile.

No remote model runs on the simulation path: a model outage cannot make a room
wait for inference. During a control outage, the last accepted PvP/level/event
configuration remains in force; fresh administration has no finite propagation
objective until the API recovers. Directory data can become stale; it does not
acknowledge saves. Profile-2 storage outages intentionally pause only affected
encounters when their accepted-state bound is reached.

Related qualifications: `2026-11-05-cast-reservations.md` and
`2026-11-05-character-recovery.md` under `docs/verification/`; the latter includes
real storage outage, save-capacity, stale-owner and drain faults. The native
background outage consumer is `coax/web/room/background-check.coax`.

Measured work is an amortized budget, not preemption of a call. Across sixteen
rooms the refill is 720 ms of charged work per second; each room also has at
most 3 ms stored credit and can overrun by its current uninterruptible call.
Socket framing/sending and publication application are measured separately.
OS descheduling is included in wall duration. A room under sustained overload
slows its simulation rather than corrupting accepted effects or taking another
room's future credit. This is observable in `/health`'s process-wide deferred,
refused and retired counters, lowest credit and named largest last tick.
The qualified throughput/latency envelope still requires workload receipts;
the arithmetic is not a claim that every workload holds 30 Hz.
