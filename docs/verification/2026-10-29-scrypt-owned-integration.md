# Experimental scrypt job with owned intermediates

The NFKC-to-scrypt host now routes both PBKDF2 stages through the owned
PBKDF2/HMAC/SHA workspace. The explicitly named first PBKDF result, mixed
ROMix result, password, ROMix state, BlockMix/Salsa state, HMAC pads and SHA
schedule reside in the host's registered secret state. After success, failure
or cooperative cancellation, the existing host wipes its complete secret state
and the separately registered large scratch before publication. A failed wipe
remains pending or produces the existing explicit output quarantine. The
normalization workspace has its own cleanup. Original password/salt and any
published output remain caller-owned.

An invalid admission occurs before state reset; after admission, an internal
PBKDF failure can leave the reset/partially populated owned state requiring
cleanup. The host marks that failure for cleanup. The direct primitive alone
does not perform any erasure. Live stable disjoint allocations, accurate
extents, serialized calls, state integrity and honest erase acknowledgements
remain host obligations; arithmetic range checks do not establish allocation
liveness or actual Wasm memory bounds. Consumers must rebuild against matching
headers because the secret and host state layouts changed. ROMix remains
password-dependent in its memory indexing.

Named workspaces do not prove erasure of scalars, compiler or ABI copies,
stack/register spills, snapshots or crash remnants. This is not a physical
iPhone test, machine-code timing approval, complete NIP-49 backup, or approval
to use production secrets.

The enclosing job passes 86 loaded Coax checks and all 2,474 mutations without waivers; `coax check` passed. Checked/release native and
actual Wasm scrypt paths passed 16 independent `hashlib.scrypt` cases, and
native each passed a full N=65536, r=8, p=1 64 MiB vector. The normalized
host path ran the same full-size vector on checked/release iOS 26.5 Simulator,
checking the entire scratch and normalization workspace. Checked/release native
ASan/UBSan, Wasm and simulator host fixtures exercised success, cancellation,
invalid admission and injected wipe failures; the host fixture checks all bytes
of the enlarged secret state. The simulator is not physical-device evidence.
Independent source review of `scrypt_nip49.coax` SHA-256
`a0eb03aae6c68011e3125706ec727124728ef0dac57fb181116e55555ff8f8a9`
found no concrete internal-alias or transition blocker under the stated host
premises, without rerunning tests or approving production use.

Two pre-existing optional large native vector runners had mirrored old state
layouts in Python; their bounded test-only C helpers now use generated header
types. Checked/release ROMix and scrypt large vectors passed with those helpers.
The helper's public test inputs are not an erasure test. The checked/release
full normalized simulator binaries hashed to
`b5afada34ffc320bd924efb7269b6b415ae45fc84ff56747bd489152ccd9a983` and
`565a2c03c1ed9df77a2bed065c1167948ec34b94fff8788ae4a91369e17ef27a`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
