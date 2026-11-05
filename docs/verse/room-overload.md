# Everglade room overload profile

Issue #12841. This records the single-owner room's work classes and the bounds
enforced so far. A bound on storage is separate from CPU isolation; the remaining
CPU/content-warming and integrated gameplay qualification stay open until their
receipts are recorded.

| Work | Admission and reservation | Full/late/outage outcome |
| --- | --- | --- |
| Movement intents | Per-socket frame/byte token buckets, fixed input queue and authoritative time credit | Extra input is discarded/counted; sustained abuse closes that socket; a faster sender earns no faster movement |
| Cast intents | Two outstanding reservations per seat, 32 per room | Existing `r SEQ busy` before cast order/cooldown is spent |
| World ability service | Two starts and four ends per seat, 96 entries | Refuse extra starts before cooldown/resources; refuse extra ends before ending concentration; preserve movement/end ordering |
| Door/talk/boat interactions | One interaction and one release per seat, 32 entries | Existing `r SEQ busy` for extras; starting an interaction cannot consume its release reservation or another seat's work |
| World damage input | One aggregate per seat per tick, 16 reserved mailboxes | Repeated complete reports replace their seat's snapshot; resolved once after heals; no overflow-to-immediate-damage path |
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
