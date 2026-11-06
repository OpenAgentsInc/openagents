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

## Commands and accepted state

`I SEQ THROTTLE STEER FLAGS PAINT CLOCK HEARD` supplies the latest controls.
The room ignores repeated or older sequence numbers and acknowledges only
controls actually consumed by its world step. The match clock, collision,
fuel, goals and car transforms remain server state. The page displays the
authoritative snapshots; it does not submit a transform as evidence.
After one second without fresh controls, the room releases them.

Each `S` frame contains one complete tick. This small-room profile selects
absolute rows (`BACK 0`) with explicit integer field masks. Public recipients receive body,
paint and life fields. Only the owner receives fuel. No struct field is
sent by default. A partial snapshot is neither applied nor acknowledged.
A body absent from the next complete snapshot is removed. This profile has
no separate removal queue or delta history. New connections advance car
life; the match's `KICKS` counter identifies kickoff discontinuities.

Both this profile and Everglade use the Coax contracts in `aethyr/net/`.
Each realm keeps its rules and codec; hosts retain sockets, clocks and
other platform effects.
