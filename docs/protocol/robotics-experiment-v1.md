# Robotics experiment record v1

Registry identity: `psionic.robotics.experiment.v1`. Owning context: Psionic.
One codec: `psionic/lib/robotics/experiment-wire.coax`. This inert record carries
the decoded CPU query profile in `experiment-policy.coax`; reading it performs
no simulation, training, device or network effect.

All integers below are little-endian u32 except `sign`, which is little-endian
two's-complement i32. Floating values are exact IEEE binary64 words, little
endian, and must be finite. Signed zero is preserved. Byte identities have no
endianness. No Coax or C struct layout is part of the format.

| Offset | Bytes | Field |
| --- | --- | --- |
| 0 | 4 | ASCII `RXE1` (version 1); unknown versions refuse |
| 4 | 4 | complete record length, exactly the received byte count |
| 8 | 4 | joint/action count `n`, 1–16 |
| 12 | 4 | observation count `m`, 1–32 |
| 16 | 4 | frame |
| 20 | 4 | execution target |
| 24 | 4 | physics period, integer nanoseconds |
| 28 | 4 | control period, integer nanoseconds |
| 32 | 4 | world count |
| 36 | 4 | horizon |
| 40 | 4 | cycle budget |
| 44 | 4 | seed |
| 48 | 192 | model, assembly, calibration, task, partition and evaluator IDs, 32 bytes each |
| 240 | 52 × n | active joints, in action-coordinate order |
| 240 + 52 × n | 32 × m | active observations, in observation-coordinate order |

A joint is five integer words (`joint`, `motor`, `sign`, `unit`, `mode`), then
four binary64 words (`home`, `scale`, `low`, `high`) in radians. An observation
is four integer words (`joint`, `signal`, `unit`, `frame`), then two binary64
words (`offset`, `scale`). The complete length is `240 + 52*n + 32*m`, at most
2,096 bytes. There are no optional fields, padding, trailing bytes or inactive
array slots. Decoding starts from a fresh descriptor; inactive slots retain
the declared type defaults and never retain prior caller memory.

All conventions, ranges, unique mappings, periods and identity requirements
are the decoded profile's admission rules. The reader preflights finite words
before converting them into floating values, and invokes that admission.
The expected embodiment includes the full active joint map; unchanged ID
bytes do not permit altered signs, scale, limits or coordinate order.

The artifact identity is the tagged SHA-256 of the **entire record**, with tag
`psionic.robotics.experiment.v1`. Specifically, let `t = SHA256(UTF8(tag))`;
the identity is `SHA256(t || t || record)`. The reader requires this expected
32-byte identity from its caller and compares it before publishing any decoded
state. It is not embedded recursively in the record. The writer returns it
with the byte count. This provides exact artifact binding, not a signature,
source-rights verification, or authorization to operate hardware.

Writing requires a valid decoded experiment and expected embodiment and an
exact output span. Reading requires the exact record span, expected artifact
identity, expected embodiment and exactly one output descriptor. Refusal
preserves all caller output. Returned failure results carry zero byte count
and zero digest; a policy refusal also identifies its `RexStatus`.

The independent golden in `proto/psionic/robotics-experiment-v1.json` is the
owned two-joint fixture. It was encoded with Python's stdlib `struct` and
`hashlib`, independently of the Coax codec. The oracle checks all fields,
the bytes and tagged identity. Published v1 bytes remain fixed; a later field
or meaning change needs a new version.
