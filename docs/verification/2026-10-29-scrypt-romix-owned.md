# Experimental ROMix owned intermediates

**Update:** The subsequent [scrypt owned-workspace integration](2026-10-29-scrypt-owned-integration.md) supersedes this snapshot’s integration status. Its stated hidden-copy and production limits still apply.

The existing experimental ROMix now retains its next block and the owned
BlockMix/Salsa workspace inside `RomixState`. The registered scrypt host wipes
its full `Nip49Scrypt` secret state, which includes those fields, before
publication. The validity helper now reads metadata through a view rather than
explicitly passing the entire secret-bearing state by value. The host and every
consumer must rebuild with matching generated headers: the state and host ABIs
have grown. The input, large scratch and output remain separately owned, and
existing admission, lifetime, disjointness and trusted-effect premises apply.
A pre-admission refusal does not erase pre-existing caller storage.

This is only a partial KDF cleanup slice. In particular the surrounding
PBKDF2, HMAC and SHA paths and NIP-49 start/finish still contain explicitly
named password-derived local arrays. Compiler/ABI copies, scalar values,
registers, snapshots, crash handling, target timing, physical devices and
production use are not proven erased or safe here. ROMix memory indexing still
depends on password-derived data.

ROMix's 21 loaded Coax checks and 880/880 complete mutations passed without
waivers. The enclosing job's 77 loaded checks passed. The checked/release native
and actual Wasm ROMix test independently compared 27 cases to the byte model,
and exercised its native sanitizer fixture; that run preceded the addition of
an empty-view check only, with unchanged implementation. The current
checked/release NFKC-to-scrypt host fixture passed native sanitizer, Wasm and
iOS 26.5 Simulator normal and injected-failure cases. Each iOS profile also
ran the full N=65536, r=8, p=1 64 MiB normalized scrypt vector against
`hashlib.scrypt` and checked scratch and workspace erasure. That is simulator,
not physical-device evidence; it is not a proof of hidden-copy erasure.

Independent source review of SHA-256
`b0238ad8341262e91885aed745ed4f7ee5cb48c9f82ac713c03c736f2cdc10f3`
found no concrete alias, state-transition or initialization blocker under
valid stable disjoint host allocations and untampered state. The reviewer did
not rerun tests or approve target timing or production use. The checked and
release large simulator executables hashed to
`864ef28b3799c4aa934d749607f4858062d8e7919430ff7bc5e28d45a9cfd360` and
`659eb5ce9477be2f3349cef043f7928291d0bb31803ef4096afc74b805c0bc69`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
