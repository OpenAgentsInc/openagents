# Bitcoin full-size P2P messages — 2026-11-03

Closes #12704's recorded chapter-65 checksum gap. The independent native
oracle was added first. It exposed an additional failure below one MiB:
the writer passed its whole output workspace to the bounded endian helper,
which refused without writing the length. Beyond one MiB, the one-shot
checksum also refused. Both reader and writer now use chapter 34's streamed
SHA-256d construction, and endian words address their exact field bytes.
The one-shot cryptographic API retains its original bound.

`p2p-large.py` computes complete frames with Python hashlib and compares
every byte with Coax, independently. Final debug, release and ASan/UBSan
runs each pass eight payload lengths: 0, 1, 1,048,575, 1,048,576, 1,048,577,
2,097,183, 4,000,000 and an oversized 4,000,001 refusal. They also check a
truncated frame, mutations at the first/middle/final payload byte, and an
untouched output sentinel. The published 61-line P2P transcript still agrees
with the independent pinned-Core/Python oracle in debug and release.
The seven new Coax boundary-check bodies also execute through the native
oracle ABI in each mode, including ASan/UBSan. Verification-only `checks-c`
emission refused the borrowed-view dependency with E0413, so that emitter
is not used as evidence; ordinary generated C runs the same check bodies.

The full root mutation run originally exposed 108 older gaps among 699
sites. Added checks cover nonempty frames and full command names, signed
word boundaries, every optional version extent, exact writer capacities,
CompactSize boundaries, IPv4/IPv6 mapping, inventory/locator/address limits,
malformed and truncated headers/addresses, and unknown addrv2 networks.
Redundant preconditions were removed where a required later read already
decides the same failure. The addrv2 writer computes the exact services
CompactSize length, accepting a buffer exactly as large as its wire result.
The locator's nonnegative subtraction uses its sign mask, avoiding an
unobservable equality branch while preserving Core's height sequence.

Final full root gate: **718 mutants, 718 killed, zero survivors**. All
imported baseline checks remain. No mutants were marked equivalent. The
sync tool's local large-checksum workaround is removed; it now calls the
shared frame reader directly.
The Bitcoin chapter registry and both changed chapters' source-linked
listings pass. The full book's existing capability-evidence mismatch is
unchanged; this focused check does not bypass it in the release gate.

Compiler: coax0 0.48.0, SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.

```sh
coax/bin/coax mutate coax/lib/bitcoin/p2p.coax --root-only --jobs 8
python3 coax/tests/bitcoin/p2p-large.py --coax coax/bin/coax --mode debug
python3 coax/tests/bitcoin/p2p-large.py --coax coax/bin/coax --mode release
python3 coax/tests/bitcoin/p2p-large.py --coax coax/bin/coax --mode sanitized
python3 -I coax/tests/bitcoin/p2p.py --coax coax/bin/coax
```
