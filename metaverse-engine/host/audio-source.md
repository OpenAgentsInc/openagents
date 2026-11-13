`hasb_bytes(current:int, incoming:int, limit:int) -> int` is exported to C.
It admits the next complete application-owned compressed-source or companion
copy before that copy is allocated. Limit must be 1..8,388,608 bytes;
current/incoming must be nonnegative and their sum must fit that limit.
The result is the exact new total or -1. Subtraction preflight avoids
overflow; refusal does not admit a prefix. Zero-length SDK chunks are
allowed and require no retained allocation.

The caller chooses the limit from its checked source/companion descriptor,
discards all retained chunks on refusal, cancels the concrete reader and
keeps ownership until the reader/decode completion has settled. A successful
coalescing copy needs its own reservation while chunk copies remain live.
PCM-word tickets additionally admit those actual owned allocations. SDK
response chunks, codec private workspace and OS buffering remain separately
scoped observations. This byte counter does not claim to bound them.

Check-first root mutation kills 19/19 mutants without equivalents. Boundary
checks cover both exact endpoints, zero offers, negative extents, an already
invalid current total and a maximum signed incoming extent.
