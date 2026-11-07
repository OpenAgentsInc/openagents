# Retained source observation v1

`recorded-source.coax` admits portable exception payloads for the bounded
streamed-walk replay observer. This is a separate optional diagnostic
profile, `aethyr.recorded-source.v1`; it changes no `aethyr-host-v1` ABI,
VPK/RCM bytes or canonical simulation state profile.

A trace opting in supplies both fields:

```json
{
  "sourceSchema": "aethyr.recorded-source.v1",
  "sourceSnapshots": [
    {"owner": 5, "generation": "10", "epoch": "1", "hex": "01000000"}
  ]
}
```

The abbreviated payload above illustrates the shape; a real payload must
fill its manifest's entire encoded source allocation. The profile permits
at most 64 snapshots, each at most 4,096 bytes. Hex is lowercase, two
characters per byte. The completion's full `(owner, generation, epoch)`
selects exactly one snapshot. The expected source SHA-256 comes from all
eight words of that completion record; the separately authenticated scene
and RCM1 manifest retain the resource's intended content identity.

The native observer refuses unknown/missing profile versions, a missing
or non-array snapshot table, oversized counts, duplicate matches and
unconsumed snapshots. `recorded_source` checks exact length, spelling and
SHA-256 in private bounded scratch before copying any output. Failure
preserves the destination, including its tail. The original SDK copy still
requires the current physical allocation's row, generation, epoch and
exact byte extent. No new allocator, ticket or publication path is added.

The observed bytes are the **retained allocation**, not a network packet
recording. For an oversized or cancelled transfer, that allocation can
still contain only zeros even when a complete valid prefix arrived at the
transport. A corrupted received allocation has a different source digest
from the intended asset. Such a snapshot is not asset admission: canonical
decoding and the existing owner/readiness gates still refuse it.

The headless Coax recorder observes the existing Wasm completion export
before it calls the unchanged SDK face. It records only failed
completions. The observation borrows the actual retained source span; the
hex string is diagnostic recorder storage outside the dynamic resource
budget. The recorder reports captured source-byte counts. String/JSON
backing and GC overhead are not claimed as engine allocations. Existing
successful histories without this extension retain their original replay
format and remain readable; unversioned prototype exception tables do not.

Identity words use checked ASCII signed-decimal parsing, with a 20-byte
spelling bound and the full signed-64 range. They do not pass through
binary64. Fractional/exponent spellings and overflow refuse; coordinates
have their separate floating-point path. Native edge observations are
generated with `coax/web/tool.sh aethyr/tools/recorded-source-words.coax`
and independently checked by the receive-fault oracle.

The [receive-fault verification record](../../docs/verification/aethyr-streaming-12990/walk-receive-fault-replay.md)
binds the measured source identities, allocation traces and per-paid-step
native comparison. This profile currently qualifies that bounded fixture,
not arbitrary larger assets or every SDK failure.
