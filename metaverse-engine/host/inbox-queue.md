# Bounded receive retention

`inbox-queue.coax` admits application-owned native callback storage. It does
not measure browser, URLSession, Network.framework or kernel buffers, and
does not decide whether a callback's transport owner is current.

```coax
hiq_open(capacity:int) -> HostInboxQueue
hiq_reserve(queue:[]HostInboxQueue, bytes:int) -> HostInboxGrant
hiq_release(queue:[]HostInboxQueue, lease:HostInboxLease) -> bool
hiq_reset(queue:[]HostInboxQueue) -> bool
```

One fresh queue accepts an observed inbox capacity of 0–8 MiB. It permits at
most 64 events and four inboxes of payload bytes. Zero-length events still
consume a lease. Each individual payload fits one inbox; oversized wire
messages must first become the stream authority's empty error event.

A grant reports `ok`, `full`, `invalid` or `retired`, and only `ok` includes
a lease. Allocate/copy/retain the complete event only after `ok`. A native
reader encountering `full` must wait for capacity before retaining another
event or reading another chunk. A browser adapter that cannot apply read
backpressure must implement an explicit failure/close outcome; this module
does not authorize silent loss or an unbounded retry queue.

The lease identifies one slot, monotonic serial and queue epoch. Release it
only when its immutable event is no longer retained. A duplicate, replaced
or previous-epoch lease cannot alter counters. Reset invalidates all leases
before a replacement engine is allowed to retain events; SDKs must free
their old storage and wake stopped readers separately. Serial exhaustion
retires until reset; epoch exhaustion retires without wrapping.

The 106-mutant root gate has zero survivors and no equivalents. It covers
exact event/byte boundaries, empty frames, invalid capacities, slot reuse,
double release, stale epochs and exhaustion. Physical SDK backpressure and
loaded-main-thread receipts remain separate integration requirements.
