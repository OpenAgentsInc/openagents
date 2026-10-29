# Experimental owned BlockMix workspace

This experimental Coax BlockMix/Salsa20/8 implementation places the named
working and feed-forward words in caller-owned `BlockmixWorkspace`. It is not
yet connected to ROMix or the NIP-49 host. A trusted host must provide valid,
stable, mutually disjoint views and truthful alias facts, wipe the entire
workspace after admitted use, and separately manage password-derived input and
output. Refusal precedes writes and leaves prior workspace bytes untouched.
Scalar values, compiler/ABI copies, registers, target timing, crash cleanup,
physical devices and production-secret approval are outside this evidence.

All 18 loaded Coax checks and 691/691 mutants passed with no waivers. An
independent Salsa/BlockMix model checked 128 inputs spanning r=1..8 on each
checked/release native and actual Wasm path, with a full test-host workspace
wipe after every call. ASan/UBSan covered all r values and five refusals. iOS
26.5 Simulator executed one independently computed r=1 vector and those five
refusals in each mode. A read-only source review of
`b51b29f8acaa21b0ac1d727fc5920d6af3bb2ed885ef13af8d61213c657505b7`
found no concrete Salsa feed-forward, permutation or boundary blocker under
the documented view and host premises; the reviewer did not rerun tests.

Checked generated C SHA-256
`8abdc8021736a849b86bf329db7f966159083ec3ad9f3bfb0b58ecbd2fbcd272`,
Wasm `f90a721c91906de678139ca41ecf9709e99999617b714962d943c1831687ee8a`;
release C `ced2f21d8a13352df98b8193626725194de2eafef5557876087f413ddcc20d48`,
Wasm `371cc06c0c884e2cf41a4a1b5f76441961d3615f02dfbe8817387426ba615221`.
Checked/release simulator executable hashes:
`1a2d39a3e7ca8055cf7cad9f5dbb6ae1b74f8150cf9bbde7bfc6f2a3c9476065` and
`b1b00c2ac502dd1328f68ad86d5cfc91f9b386b59e220727b235d73c929d0e77`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
