# Experimental XChaCha20 owned workspace

The experimental Coax HChaCha20 and XChaCha20 entry points retain named initial
and working words, the derived subkey and derived nonce in caller-owned
`XcWorkspace`. They do not yet replace the cipher called by AEAD. A trusted
host must validate live, stable, pairwise-disjoint views and truthful overlap
facts and erase the entire workspace, including after a successful empty input.
Admission refusal happens before initialization and leaves any previous bytes
untouched. Key, nonce, input and output remain separately caller-owned.

Scalar loads, temporary values, compiler-generated copies, stack spills,
registers, snapshots and crashes are not covered by workspace erasure. This
slice establishes no machine-code timing, physical-device result, production
secret-use approval or complete erasure guarantee.

All 22 loaded Coax checks passed and 619/619 mutations were killed without
waivers. Each checked/release native and actual Wasm path passed 513 cases
against an independent HChaCha model and OpenSSL ChaCha20, with messages up to
1,024 bytes, counters including random values, per-call workspace wiping and
output canaries. Native ASan/UBSan covered one 1,024-byte call, one derivation
and five refusals. iOS 26.5 Simulator executed the draft XChaCha vector and five
refusals in each mode, not the full corpus. Independent source review of
`6b61e3ff595a6170ec7e05498157c90ec548ed1c4252cfbf273677c2a13b3a79`
found no concrete arithmetic or in-place transition blocker within the stated
view and ownership premises; it did not verify target timing or erasure.

Checked generated C SHA-256
`44625a9785d46ee822e576e7bc9262999bbe1b33de974de54882875292da9d06`,
Wasm `bd63648ceb4ddd7279be2d4215a8e38bece7a990512a447277a38e8bc4f96262`;
release C `1f93bda645e6c20228b865d5ff347dc8b80338cbf3424102da676ff7e750cb48`,
Wasm `07b7ad5aebf68c91530c08f8490b2c21add47c5de8e7892eb5781d7741e544b6`.
Checked/release simulator executable hashes:
`7b6b6b02a8d72d1b4181cec76cc9fc03592e6ddbbd6999f860c4deaa7a4c3003` and
`e52a77beb6835d4757b4c333a5693ce318f54d025e0b16912d323ee7167a4f44`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
