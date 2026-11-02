# Fresh B3 witness after the upstream rebase

[The record](record.json) binds the actual System One/Lab consumer after root
`975b94412d` rebased onto `8ed49c9b6a`. The earlier
[consumer qualification](../README.md) and its receipts remain unchanged.

Upstream added `SoClient.key_value` and child-environment setup, changing the
client ABI and emitted transport. It also added opt-in PTY fields and handling
to the shared native process host. The Lab uses the existing keyless, pipe-only
path, but changed producer inputs still require fresh native observations.
These checks do not qualify the new key-injection or PTY feature itself.

All five focused groups pass on the recorded coherent R2 image `67e753ae…`:

| Check | Fresh result |
| --- | --- |
| [Typed API](typed.json), [checks](typed-checks.json), [mutation](typed-mutation.json) | 42 groups plus authority coverage; 32/32 root mutants killed, zero survivors; exact E0907/E0904/E0383 diagnostics; five-line real-host transcript at O0/O2/fatal ASan+UBSan. |
| [Raw owner](owner.json) | 150 named assertions in each native mode: capacities, terminal snapshots, stale tickets, reuse, cancellation and all scripted failures. |
| [Actual Lab and real host](lab.json) | Six requests on the same canonical owner in each native mode; all transcript borrows retained, current child cancelled/reaped and stale tickets leave a reused unrelated slot alone. |
| [Factory/defaults/attachment](factory.json) | 16 native runs against complete current generated types and actual host/factory/quit effects. |
| [Actual Lab entry](entry.json) | 14 RED/GREEN runs, eight active/ready/invalid/exhausted cases per native mode; named state, descriptors and bytes preserved. |

No product or test-oracle change was needed. Every runner verified its complete
source inventory and tool identities before and after. These are fresh passes;
no older C/header or native result was reused across the changed transport and
host. The [Lab emission receipt](lab-emission.json) identifies the exact actual
production declarations and explicit downstream-effect test boundaries.

[The archive](proof.tar.xz) retains 227 text source, generated-code, diagnostic
and raw receipt members. All were read back against [the manifest](manifest.json).
Binary/object/dSYM bytes are excluded; their observed hashes remain in receipts.

The canonical-owner exclusivity, destination disjointness and caller snapshot
lifetime preconditions are unchanged. Full desktop readiness retains its
separate E0381 application gate. This record does not claim general compiler
linearity, remote exactly-once effects or compiler normal-release/cache
qualification; the overarching R2 record owns that producer qualification.

The [final coherent compiler binding](final-image/README.md) subsequently verified
all 11 emitted C/header artifacts byte for byte, confirmed the complete native
input closure, and passed the final image's focused checks, two zero-survivor
policy mutation roots and exact misuse diagnostics. Native runs were retained
through that measured artifact equality, not repeated or relabeled.
