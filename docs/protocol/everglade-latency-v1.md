# Everglade latency observation v1

Implementation status: the codec, bounded server observation kernel, clock
intervals, native socket effects and browser application/render/GPU effects
are integrated. The browser explicitly opts in with `roomtrace=1`. Ordinary
sessions do not allocate a client observer or send this profile.

This optional profile is separate from the published Everglade session wire.
Its fixtures and bounds are in
[`proto/verse/everglade-latency-v1.json`](../../proto/verse/everglade-latency-v1.json).
It changes no gameplay message, admission rule or ACK meaning. A client must
explicitly opt in; peers which do not recognize `@1` ignore these frames.

All clocks are integer monotonic microseconds, limited to exact JavaScript
integers. The sync exchange echoes the client's send time and records server
receive and response-encoding times. With client receipt time `c3`, server
offset lies in `[server_encode-c3, server_receive-client_send]` under a
constant-offset model and nonnegative transport delays. Quantization and a
declared drift allowance expand that interval. A stale, inconsistent or absent
sync cannot establish a one-way latency. Midpoint estimates must retain their
uncertainty and model; wall clock and simulation tick time are not substitutes.
A signed lower bound can cross zero when clock uncertainty exceeds the transit.
That expresses uncertainty, not a physically negative transfer time. An interval
entirely below zero is inconsistent and refused.

The browser's declared model uses 100us timestamp quantization, 100ppm drift
and a five-second maximum sync age. These are explicit assumptions, not
measured bounds on arbitrary remote clocks. The server response timestamp is
taken at the SDK call into the response codec, before the encoded bytes enter
the socket queue; its remaining encoding/queue delay stays inside the interval.

The input observation request precedes an ordinary `I` message. Only a newly
accepted input and the exact successfully stepped sequence qualify. The room
records receive, movement-step begin/end, simulation end, serialization
begin/end and final complete `D` queue acceptance. A partial, refused or missing
snapshot cannot complete a sample. Per-socket limits are one in-flight input
sample, four requests per second and a two-second deadline. Sync is at most
once per second. There is no telemetry retry queue.

The browser retains distinct times for WebSocket receipt, application delivery,
render submission and GPU completion for the same engine/socket generation,
input and complete state tick. Its first completed render after consuming that
ACK is a measured render boundary, not proof of physical display scanout.
Interpolation delay is reported separately. Engine replacement, socket reuse
or missing GPU completion invalidates a pending sample. The history is bounded
to 64 entries and exposes missing, expired and refused observations.

Telemetry is observational. It cannot change gameplay placement, queues, step
credit or simulation state. An exhausted/late/invalid observation is omitted
and counted as such, never relabeled as zero latency or durable acceptance.

`coaxial.roomTrace()` exposes completed raw timestamps, interval bounds,
expired/refused/missing counts and up to four retired socket observers.
Each concrete socket has a fresh observer. A GPU completion for an old engine,
device or socket cannot finish its successor's sample. The native encoded
counter describes report encoding; actual queue bytes remain in the native
frame observer (optional `@1` frames use its existing `?` message class).

The headless driver and raw-record reader are
`coax/web/room/latency-browser-check.coax` and `latency-summary.coax`; see the
source/profile and network-fault record in
`docs/verification/2026-11-05-room-latency.md`.

The independent fixtures run with
`coax/web/tool.sh coax/web/room/latency-profile.coax proto/verse/everglade-latency-v1.json`.
