# Experimental Poly1305 owned workspace

This experimental Coax entry keeps the explicitly named clamped key, limbs,
block, products and reduction state in caller-owned `PolyWorkspace` storage.
It does not yet replace the Poly1305 called by the AEAD operation. A trusted
host must supply live stable, disjoint views and truthful overlap metadata, and
must erase the entire workspace after use. Refusal happens before workspace
initialization and leaves any previous bytes untouched. The caller also owns
and must separately manage the key, message and output. The test-only fixture
wipes the workspace after successful and refused calls; it is not a production
adapter or proof that other hosts do so.

This narrows named local array storage, but scalar values, compiler-generated
copies, stack spills, registers, snapshots and crashes are not covered by the
wipe. No machine-code timing claim, physical-device result, production-secret
approval or secure erasure guarantee follows from these tests.

All 21 loaded Coax checks passed; all 551 mutations were killed without waivers.
The checked and release paths each passed 1,018 independent arbitrary-precision
Poly1305 vectors on native and actual Wasm, including a 1 MiB message and a
modulus-boundary input. OpenSSL independently corroborated 102 cases, including
the maximum input. The fixture verifies the full workspace is zero after each
call and preserves the output canary. Native ASan/UBSan covers one 1,024-byte
call and four metadata refusals. iOS 26.5 Simulator executed the RFC vector and
four refusals in each mode, not the full vector corpus.

Independent source review of SHA-256
`a4130603114aed66485bf688b14ad38c3cb950ccd45c964521ada0eabd3a71f5`
found no concrete arithmetic or boundary blocker under those premises, while
not independently proving every long-sequence carry bound or target behavior.

Checked generated C SHA-256
`0bdeb98cbb5b3b7e7d91e9aaeb713db86cac3c502fdb65d0bb925050b91424ea`,
Wasm `e7079bce5853a9816eec21132fac378b86949c7053a4d94174ab5328b0f8c743`;
release C `1c1e0e7e777f19ff270bcfc2e9fb405c8385be7d3cbb716cca7d0780989a5414`,
Wasm `a657cd66f0ff7a634552b8272ae41905c3340ecc1f6fefbbef3249e75b883e13`.
The checked/release simulator executables hashed to
`e09ec1ef6679e5c54e9d58d176532083924fc94afed6389c78c1b6e0c4d35985` and
`3b5feee4104d063ef46229a7c9379e53f3f133fd85fee0ff251a9a6d5bbfdf53`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
