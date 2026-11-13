# Media packet storage

`coaxial_media(state)` returns borrowed command bytes and
`coaxial_media_len(state)` returns their logical length without draining.
Version 1 also exports `coaxial_media_capacity(state)`, the actual allocation
capacity beginning at that pointer. Everglade reports its attached growable
media view's count, or `sizeof` its inline array. The export does not parse,
copy or mutate the packet.

Before forming a byte view or calling the media parser, hosts call:

```
hmp_span(address:u64, capacity:u64, bytes:u64,
         address_limit:u64, capacity_present:bool) -> bool
```

An empty packet needs no storage. A nonempty packet requires the physical
capacity export and one complete allocation admitted by `htb_copy_span`.
The address limit is the actual Wasm memory length or native pointer bound.
Refusal supplies an empty media request set for this frame: no read, parser
call or notice from rejected bytes, while unrelated world rendering continues.
It does not turn a previously cached image into a successful new request.
Pre-v1 browser consumers keep their existing behavior.

`media-span-conformance.coax` admits eight rows of five observations:
`admitted, published_bytes, byte_sum, parser_calls, empty_result`. The ordered
offers are valid, logical length beyond physical capacity, allocation beyond
the address limit, null, pointer wrap, missing capacity, empty, and valid again.
The measured four bytes are `65,66,67,63`. The browser component uses real
Wasm memory, the Coax services reactor and `RenderHost.prepareMedia`; its
parser-call boundary is observed with a stopping callback. It does not claim
GPU or world-frame continuity. Native actual parser/frame receipts remain
separate host integration gates.
