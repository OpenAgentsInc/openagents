# Experimental owned streaming SHA-256

**Update:** The subsequent [scrypt owned-workspace integration](2026-10-29-scrypt-owned-integration.md) supersedes this snapshot’s integration status. Its stated hidden-copy and production limits still apply.

This internal Coax SHA-256 path retains its chaining state, block and 64-word
message schedule in `ShaOwned` caller-owned storage. It supports streaming
updates and a single finalization. It checks phase, lengths and the structural
`used == total % 64` relationship before effects; the host must still preserve
state provenance and supply valid, stable, disjoint views and truthful alias
facts. The complete workspace requires a real wipe after use. Pre-admission
refusal leaves previous storage untouched. This path is not yet connected to
the scrypt/HMAC job or used by the compiler fingerprint implementation.

Scalar/ABI/compiler copies, registers, OS snapshots, crash cleanup, physical
devices, target timing and production-secret use remain outside this evidence.
Input and digest output are separate caller-owned regions.

All 42 loaded Coax checks and 1,063/1,063 mutations passed without waivers.
The checked/release native and actual Wasm paths each passed 146 NIST and
hashlib vectors using five stream chunk sizes, with test-host workspace wipes.
Native ASan/UBSan covered a 16 KiB stream and refusal; each iOS 26.5 Simulator
profile executed the abc vector and refusal, not the full corpus. Independent
review found a missing structural metadata check, now corrected, and found no
other concrete schedule, padding or public-length blocker under the stated
state and host premises. It did not establish target behavior. Final source
SHA-256: `8f6b5c4d0eff8d122c1db39a2c676a652f91553ce2f6f78ee68de66df136de7c`.

Checked generated C SHA-256
`0e499469430fc60618f9a351abd38d2caae2c68f6594ad32b031a7a0d935be61`,
Wasm `d7080a5609edcf15865f2f1f5775a01971134fbab2426274ed8fabf65a39cf97`;
release C `877d11a253069a6510c675d8ad95c0dc8931ed152572897bbee73434362fe452`,
Wasm `23964c079aab1c14ba1ebfaef2c80dc315d30fd2da46504af7e6cb06f3974510`.
Checked/release simulator executables hashed to
`78957da91a1ac7a501c81436865d431243fdb07f235c5cddeaddee72119da0f7` and
`abfa338a0565a91398cc2d7b200502435d07208549d6b1be6c03e9874c0a54fc`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
