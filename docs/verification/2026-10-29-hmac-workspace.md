# Experimental owned HMAC workspace

This experimental one-shot HMAC-SHA256 retains the normalized key, inner and
outer pad, digest and `ShaOwned` state in caller-owned `HmacOwned` storage. It
handles empty, short, exact-block and long keys and propagates internal SHA
failure. It is not yet wired into the scrypt host. A trusted host must provide
valid, stable, mutually disjoint views and truthful overlap facts, and wipe
the entire workspace after every admitted call including an internal failure.
Early admission refusal leaves prior workspace and output bytes untouched.
Original key, input and output are separately owned; source scalars,
compiler/ABI copies, stack, registers, snapshots, crash cleanup, physical
devices and target timing are outside the wipe evidence. No production-secret
approval follows.

All 57 loaded Coax checks and 1,256/1,256 mutations passed with no waivers.
Each checked/release native and actual Wasm profile passed 80 independent
Python/OpenSSL HMAC vectors, with actual test-host workspace wipes. Native
ASan/UBSan covered 1,024-byte key and 4,096-byte input plus refusal; iOS 26.5
Simulator executed the RFC 4231 case and refusal in each mode. Independent
review of source SHA-256
`630ee1062b8044bed490f86fb30dbcc1c05d51d705355e8161d3f4b63c2e1f6a`
found no concrete long-key, internal-alias or refusal blocker under those host
premises; it did not rerun tests or approve target behavior.

Checked generated C SHA-256
`7ca38488cb33c9d73f1dd3c20655bed96dc8514ae0569cd45a85f4eedf7f0471`,
Wasm `3dfab1f9a1c393dc9fae8c32346f1235ad6d0b93420040fbc510c7ab98e26f1d`;
release C `3e230a498b3567f786fb10f28a95fc7c7ca9a78bddc2cb822291773ca37bb32c`,
Wasm `e5e232ad3b5bd7767c133619bfcd37b25b1617b26920092bda75eb5df7029b6a`.
Checked/release simulator executables hashed to
`efdba7ceaafd6579e970c3a67528a72f16fa535049cd4a39fef02e758f441c06` and
`ff967bc0271a856c8908a2171eead918ea05a64a2be31785a26a158c79a919f7`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
