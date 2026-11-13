# Orbit League session profile v2

The room owns every car and the ball in profile v2. Players send throttle,
steering, jump, handbrake and boost controls. The room steps at 60 Hz with
two 120 Hz vehicle/ball substeps. Sending more controls does not earn more
simulation steps. A reported car body cannot change an authoritative car.

The machine-readable contract is
[`orbit-session-v2.json`](../../proto/verse/orbit-session-v2.json). It
defines admission pins, messages, field units, recipient whitelists and
capacity. The new path is `/api/room/orbit-league-v2/NAME`.
The existing `/api/room/orbit-league/NAME` contract retains its identity:
clients report their cars, and the room owns the ball. That hybrid profile
does not qualify as authoritative play.

## Admission and lifetime

A connection says `H 2 ENGINE RULES CONTENT`. All pins must match before
controls can enter play. Each anonymous connection receives a new
server-issued generation. Connection loss ends that seat; reconnecting
starts a new body. This profile has no persistent progression or durable
resume promise. An unsupported hello receives a reason rather than a
silent fallback to the legacy profile.

Current engine, rules and content pins are all 1. Admission failures are
`R 1` (engine), `R 2` (rules), `R 3` (content), `R 4` (older wire), `R 5`
(newer wire), `R 7` (world not ready) and `R 9` (malformed hello). A malformed
or stale input is refused without changing controls or acknowledging it.
The page defaults to v2. `room-profile=legacy` explicitly selects the old
endpoint. V2 content 1 uses the box arena; the drum remains an offline or
legacy selection.

## Commands and accepted state

`I SEQ THROTTLE STEER FLAGS PAINT CLOCK HEARD` supplies one tick's controls.
The room ignores repeated or older sequence numbers and acknowledges only
controls actually consumed by its world step. The match clock, collision,
fuel, goals and car transforms remain server state. The page does not submit
a transform as evidence.
After one second without fresh controls, the room releases them.

The room steps a seat's controls one a tick, in the order sent (#13277). Up
to four wait; more than two waiting at a tick lose the oldest, so controls
are never more than two ticks late; with none waiting, the last one holds.
`ACK` names the control the tick was stepped with.

The page steps its own car with the same step as it sends each control,
keeps the controls by `SEQ`, and when a snapshot's `ACK` and row differ from
where its steps had the car after that control, starts again from the row
and steps the controls after it (`metaverse-engine/net/predict.coax`). It
draws the ball and the other cars from their snapshots a jitter buffer
behind (`metaverse-engine/net/interp.coax`). No field was added to the wire.

Records use space-separated decimal whole numbers. Inputs are at most 256
bytes; complete server frames are at most 2,048 bytes. A sequence is positive
and at most 2,000,000,000; axes are −1,000 through 1,000, flags are 0 through
7 and paint is 0 through 31. `CLOCK` and `HEARD` are nonnegative and at most
2,000,000,000. Sequence and generation exhaustion fail closed.

Each `S` frame contains one complete tick. This small-room profile selects
absolute rows (`BACK 0`) with explicit integer field masks. Public recipients receive body,
paint and life fields. Only the owner receives fuel. No struct field is
sent by default. A partial snapshot is neither applied nor acknowledged.
A body absent from the next complete snapshot is removed. This profile has
no separate removal queue or delta history. New connections advance car
life; the match's `KICKS` counter identifies kickoff discontinuities.

Both this profile and Everglade use the Coax contracts in `metaverse-engine/net/`.
Each realm keeps its rules and codec; hosts retain sockets, clocks and
other platform effects.
