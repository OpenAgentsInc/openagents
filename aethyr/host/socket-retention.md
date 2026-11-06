# Application socket retention companion

`socket-retention.coax` composes the existing `hiq` and `hsq` authorities.
It does not define another slot, byte or protocol policy. Input uses at most
64 retained events and four measured inboxes of payload; output uses at most
64 commands, 8 MiB per command and 32 MiB retained in total. Terminal callback
ownership remains `transport.coax`'s decision.

```coax
hsr_open(inbox_capacity:int) -> HostSocketRetention
hsr_offer(state:[]HostSocketRetention, output:bool, bytes:int) -> HostSocketRetained
hsr_release(state:[]HostSocketRetention, output:bool, lease:HostInboxLease) -> bool
hsr_reset(state:[]HostSocketRetention) -> bool
hsr_info(state:[]HostSocketRetention, output:bool, field:int) -> int
```

`offer` returns an accepted exact lease or explicit error/close event numbers.
An oversized input requires one empty error (no partial text); full/retired
input storage requires error plus close because a browser callback cannot
pause the platform's inbound reads. Outbound refusal retains the existing
`hsq_send` error/close result. Allocation failure uses `hsq_failure`. Release
only after the immutable copy is no longer retained. Reset invalidates both
directions, including when one epoch has already exhausted. Info fields are
count, bytes, per-message capacity, aggregate byte limit and epoch (0–4).
The root mutation gate kills all 39 mutants without equivalents.

`socket-carrier.coax` plus `coax/web/socket-face.c` exposes these typed
operations to an independent browser reactor. The scalar face only owns
storage and ABI projections. It also exposes the existing shared zero-length
packet span, drain budget and `hsq_plan` descriptor. The unchanged Wasm memory,
stack and compiler admission gates apply. Build it with
`coax/web/tool.sh coax/web/build-sockets.coax ROOT OUT.wasm`.

`socket-queue-host.js` performs bounded `encodeInto` staging, reserves before
copying immutable event/command bytes, and retains output leases while actual
`bufferedAmount` is nonzero. Full input storage owns at most four separate
scalar terminal cells; no payload is stored in those cells. It preserves
existing admitted messages before error/close, fences replaced owners, and
does not deliver into a blocked callback consumer. Retirement during a
current synchronous delivery keeps that copy's lease until the callback
returns. Browser, SDK and operating-system buffering remain separately
observed effects; these counters do not measure or bound those internals.

The [browser component record](../../docs/verification/host-contract-2026-11-05/socket-retention-component.md)
qualifies actual JavaScript copies and real Coax Wasm leases with explicitly
mocked/stalled send and close effects. Ordinary `host.js` adoption and
physical peer pressure remain separate exits.
