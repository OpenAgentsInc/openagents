# Everglade latency observation v1

Implementation status: the codec, bounded server observation kernel and clock
intervals are checked components. SDK adapters and the complete browser latency
flow are still being implemented; ordinary sessions do not send this profile.

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

The independent fixtures run with
`coax/web/tool.sh coax/web/room/latency-profile.coax proto/verse/everglade-latency-v1.json`.
