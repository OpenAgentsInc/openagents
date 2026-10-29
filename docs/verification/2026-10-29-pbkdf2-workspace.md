# Experimental owned PBKDF2 workspace

**Update:** The subsequent [scrypt owned-workspace integration](2026-10-29-scrypt-owned-integration.md) supersedes this snapshot’s integration status. Its stated hidden-copy and production limits still apply.

This experimental one-iteration scrypt PBKDF2 stages its salt/counter input,
HMAC workspace, digest and output in caller-owned `PbkdfOwned` storage. It
publishes output only after all blocks succeed. It is not yet the shipped
scrypt-job path. The host must provide valid, stable, mutually disjoint views
and truthful overlap facts, and erase the complete workspace after every
admitted call, including internal failure. Early admission refusal preserves
prior workspace and output bytes. Password, salt and published output remain
separately caller-owned. Scalar, compiler/ABI, stack and register copies,
snapshots, crash handling, physical devices and target timing remain outside
this evidence; this is not production-secret approval.

All 63 loaded Coax checks and 1,342/1,342 mutations passed without waivers.
Each checked/release native and actual Wasm profile passed 104 independent
OpenSSL-backed `hashlib.pbkdf2_hmac` vectors including maximum 1,024-byte
password, salt and output, with actual test-host workspace wipes. Native
ASan/UBSan covered a maximum case and five refusals. Each iOS 26.5 Simulator
profile executed the RFC vector and five refusals, not the full corpus.
Independent review of source SHA-256
`11a4c563003aabb604734f04ce43e81301f0e1cf3d1690e0034683e6d4035ec9`
found no concrete alias or transactional-output blocker under the stated host
premises; it did not rerun tests or approve target behavior.

Checked generated C SHA-256
`3dfe83b2971083d9d389bba29cd079f73db909a8f669f00c8952ccb79720fae0`,
Wasm `2792bfe5c1c7d41b49b9c65c784024dfb013c0219fbabc8b3688ee0cdf59f4b9`;
release C `2da4477aec0750fe87f2491d5d59a166e11e38bcc42167e85db097782badf39d`,
Wasm `31e5a296327832f47e5548323b9d864406091d7501207e58ba7dfba4aa340533`.
Checked/release simulator executable hashes:
`71f82d3ac159be44b506ea757cdb707a561f49377f0eee7f24776314517a666a` and
`2b01b85582432b2cc9f753ac41441af1a66de9a03c062d82425f47400b126fb9`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
