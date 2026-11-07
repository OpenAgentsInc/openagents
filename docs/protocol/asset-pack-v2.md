# Asset pack v2

VPK2 is a separate format identity with a maximum of 128 entries. VPK1 retains
its maximum of 48. The machine-readable contract is
[`asset-pack-v2.json`](../../proto/verse/asset-pack-v2.json).

All integers are little endian. The 32-byte header contains magic, entry count,
file byte length, budget byte length, table offset (32), data offset
(32 + 48 * count), and two zero words. Each 48-byte entry contains kind and
licence bytes, two zero bytes, byte offset, byte length, a zero word and a
32-byte zero-padded name (1–31 bytes). Data spans are contiguous in table order
with zero padding to four-byte boundaries. Kinds are catalog, vertices and
indices exactly once in that order, then layers, then sounds. Existing catalog
budget/bytes agreement, licence declarations, per-entry source agreement,
ordering, reserved-byte and exact-layout checks apply unchanged.

`pack_write` continues to emit VPK1; only `pack_write_v2` opts into VPK2.
`pack_read` selects the bound from the magic. Streaming recognizes both at its
12-byte prefix and retains its 64 MiB cap. Unknown versions refuse. This change
is qualified for the typed Coax pack host; the legacy JavaScript fallback does
not acquire VPK2 support.

Pack structure admission and AssetRegistry capacity are distinct. The registry
still holds at most 64 assets and refuses an oversized batch atomically, without
partial ownership. Scratch tables cover all 128 parsed entries before the
registry applies that bound. Tests exercise full 128-entry reads and atomic
registry refusal, 129-entry refusal, VPK1's original ceiling, split streaming
headers and unknown magic. Mutation runs killed 576/576 pack and 155/155 stream
mutants with zero survivors.
