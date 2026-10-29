# Experimental BIP-340 nonce hash workspace

This Coax slice computes the BIP-340 tagged auxiliary hash, XORs it with an
already parity-normalized scalar, and computes the tagged nonce hash over that
value, the supplied public key and message. It is not a signer: it does not
validate or normalize the scalar, prove its relation to the public key, reduce
the hash, refuse a zero nonce, compute a signature or perform self-verification.
The returned 32 bytes are secret and belong to the caller.

The caller provides stable, valid, mutually disjoint views and truthful alias
facts and must wipe the complete `SnNonce` workspace after any admitted call,
including failure. Early admission refusal preserves old workspace/output bytes;
an internal failure can leave old or new secret state. Source scalar values,
compiler/ABI copies, stack/registers, caller inputs, returned nonce, snapshots,
crash cleanup, physical devices and target timing are not covered by the test
wipe. This is not production-secret approval.

All 46 loaded Coax checks and 1,134/1,134 full mutations passed without waivers.
The checked/release native and actual Wasm builds each passed 115 independently
computed Python/hashlib tagged-hash inputs, including empty and 1 MiB messages,
with actual test-host workspace wipes. Native ASan/UBSan covered a 1 MiB input
and five refusals; iOS 26.5 Simulator executed the BIP-340 vector-derived nonce
and five refusals in each mode. Independent source review of SHA-256
`68812db84fab59fb5fb22fe268c9d573c44e5df2cc5a99bac008810fe8bf8ac1`
found no concrete construction or alias blocker under the stated premises; it
did not rerun tests or approve target behavior.

Checked generated C SHA-256
`6675b48ad4f2dfac88b17c7091d68c35e0059fb84f73c96b864184bb5f1a2211`,
Wasm `3e6519fb524912702c25bf8f303419bf1c421cd615adb637b7063dbeb6576f4e`;
release C `2ef0e42685ecc9a63965165170036d7949c54e574851f48af64aeb5fa02158f5`,
Wasm `dc2a9a0d937f933e70607385348d6ac8f35bc921acf9237d5c222e711517f4cd`.
Checked/release simulator executables hashed to
`773473dc3c61cf68d97876701502cede0cdc8245f3da9858190dd646e4247174` and
`b5eb884328afd1128bba824ce0a790c3335a085bba9e5ac8df87cf1ae59cd08f`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
