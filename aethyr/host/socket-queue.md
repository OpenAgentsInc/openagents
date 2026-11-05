# Bounded socket command storage

`hsq_plan(inbox_capacity)` admits the existing0–8MiB inbox and describes four
native workers, 64 retained events, four inboxes of inbound payload bytes,
8MiB per outbound command and32MiB/64 commands retained in total. A refused
descriptor has no usable capacities. These are application-owned storage
limits; they do not measure browser, SDK or operating-system socket buffers.

Initialize the command `HostInboxQueue` with the descriptor's `output_bytes`.
`hsq_send(queue,bytes)` returns an accepted exact lease or the explicit error3
and close2 receipt. Reserve before allocating/copying a complete command.
Keep its lease while queued and while the worker is transmitting it; release
only after its immutable storage is freed. Empty sends consume a ticket.
Refusal closes the offending concrete socket and discards its unsent commands,
releasing their leases. It never silently drops a command or grows another
retry/closure queue. Already executing I/O retains its allocation until that
effect returns. Other sockets keep their own callback ownership.

`hsq_failure()` supplies the same error/close receipt for observed SDK allocation
failure. `hsq_control(command)` admits idle0, send1, local stop2 and SDK failure3;
an invalid foreign command also requires error+stop. `native-socket.coax`
interprets those decisions through its existing Coax RFC6455 stream/fragment
implementation. Ordinary0/1/2 behavior is preserved. A worker receiving3 emits
an error, closes and emits its final close callback; callback ownership remains
the transport authority's separate decision.

The 35-mutant root gate has no survivors or equivalents. It checks complete
byte/event boundaries, empty commands, malformed descriptors, reuse after
release and every worker control result. The native facade emits and compiles
with the original admitted598 CPU compiler. This component does not yet
qualify native/browser/Linux storage adapters or physical peer backpressure.
