# Retained physical room effects

Issue #12841, downstream effects part. The earlier 24-shot presentation pool
could replace a live shot before its world damage landed. The regression
compares simultaneous and serial impacts against actual breakable pieces and
fails on that implementation. Physical impacts now have their own reservations:
four per player, two per physical raid slot and eight for the house, 264 total.
Already accepted waiting and staged work counts against admission. Player
overload returns `busy` before cast order, cooldown or resources; a full NPC
reservation defers its turn before cost. The SDK acknowledges only a consumed
prefix and leaves the remaining producer entries queued.

The staging bound follows the producers: 64 reserved player transfers, 392 raid
entries, 96 forge entries and five house blasts. Non-player work cannot consume
the player region. Released raid casts capture source, origin, seed and harm
when offered. Removing the caster, changing the event's harm dial or clearing
the raid cannot erase that already released world impact. New occupants of a
physical NPC slot account for outstanding work before using its reservation.

Classic spells retain their individual damage and all six meteors, or their
bolt/orb, until the corresponding deadline. Completion releases once. Individual
breaks retain their actor and origin through the later collection pass, including
immediate forge breaks and NPC impacts. Existing visual pools remain separate.
Fall results reserve every seat/person pair rather than stopping after sixteen
copies. The valid fall regression initializes the active spell world; the first
uninitialized fixture was corrected before recording the old-path failure.

Independent authority mutation kills 63/63 mutants, zero survivors and no
equivalents. The final policy uses direct overflow-safe admission arithmetic;
an intermediate redundant guard version had survivors and was simplified.
The full room suite before the concurrent Thunderwave change passes 630/630;
after rebasing that change it passes 632/632. The changed world ability group
passes 12/12 and town/ownership group 7/7.
Earlier compile mistakes, the ownership regression discovered by the town
group, the old-path failures and the corrected passing runs are retained in
`room-physical-effects-2026-11-05/`.

The release worker is 1,029,169 / 1,060,000 bytes, SHA-256
`08d1f81fa3f3fa072f2f9d3c2bcae0ca0e50c3023c12b2e0f106ce834d4aaac3`.
The native room passes defaults/startup and has SHA-256
`8e013eb68d96b85cbf7349e8825c9a6ed4f82cc2839e16ef99cb75306ad6ce4e`.
Both use the six retained C/header units in `sources.txt`, with the existing
0.48.0 compiler `cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
This is an explicit room component profile, not graphics-compiler admission.
The generated units contain `6eeb5c18de` plus this effects patch, before the
concurrent `9201cc27ba` Thunderwave change. The later rebase's complete room
suite is separate evidence. Four unchanged unit objects are reused only after
byte-identical C/header comparisons; room and world objects are compiled anew
with the native build's strict float flags. The catalog compares identical too.

The actual native cast proxy answers 30/30 under 250 ms RTT, 30 ms jitter and
2% loss: 24 taken, six expected target-gone refusals, slowest 935 ms, none past
2,000 ms. The initial integrated stress attempts are retained as failed gates:
the first did not reconnect a correctly closed slow socket; the next reconnects
but places summons inside simultaneous friendly areas and observes no surviving
pets. Neither is represented as a completed pets qualification.

The corrected 75-second native stress passes with eight observed wolves, four
areas, 37 raiders, 210 frames containing destruction and 24 containing raid
casts. It sends 1,040 casts and observes 58 explicit busy refusals and five
transport rate limits. Two ticks defer under the room work budget. The separate
moving room receives 2,250 frames, maximum state gap 77 ms. All 150 health
probes answer during a 35-second HTTP outage, with at most four of six lanes
pending; recovery clears them. The consumer which stops reading for 35 seconds
queues 254,489 application bytes, closes, then rejoins and advances from tick
450 to 1,650. No other socket closes. The final measured tick p95 is 1,400 us;
startup prepares sixteen worlds in 299,765 us, largest 19,686 us, and the later
publication applies in 454 us. These are component/run observations, not peak
memory or fleet capacity claims. The fixture's world document is 416 bytes.

Event-cap exhaustion is covered separately by the passing complete room suite:
200 cues exceed the 96-event class; twenty critical breaks and the receiving
seat's targeted hit still arrive, while discarded cues are counted. The
whole-raid allowance test also retains own state, bounds bytes, repairs a lost
multipart join and does not starve pets. The previous work-budget and background
records retain their unchanged authority gates and scratch checkpoint outage/
final-drain evidence; this stress uses legacy tickets, not profile-2 saves.
The admitted profile intentionally slows an indebted room and may close a slow
socket; it promises neither unbounded input nor uninterrupted 30 Hz everywhere.

This completes the #12841 qualification across the declared component profiles.
Current-build raid/fleet capacity remains #12843. No production service or
visible browser is used.

Rerun in an isolated worktree, with each command bounded separately:

```sh
nice -n 15 env COAX_JOBS=1 coax/bin/coax mutate aethyr/session/effect-reservation.coax --jobs 1
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/bin/coax test coax/web/room/room.coax
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/bin/coax test apps/openagents-web/everglade.coax --only 'room abilities:'
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 COAX_ROOM_KEEP_C=/tmp/room-effects-c coax/web/tool.sh coax/web/room/worker.coax /tmp/everglade-room.wasm
nice -n 15 env COAX_JOBS=1 COAX_CHECK_JOBS=1 coax/web/tool.sh coax/web/room/background-check.coax stress /tmp/oa-room 75
```
