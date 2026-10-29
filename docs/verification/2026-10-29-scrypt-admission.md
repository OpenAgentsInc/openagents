# Experimental scrypt address-range admission

This Coax policy checks metadata and pairwise disjointness for session,
scratch, password and salt at start, and session, original scratch and output
at publication. It rejects null nonempty spans, integer wrap, oversize buffers,
invalid scrypt parameters and insufficient scratch. Separate 32-bit entry points
check spans against a 32-bit address limit; the base entries use a 64-bit limit.
An empty span does not dereference its pointer and may carry any address. The
last possible byte whose half-open end would equal 2^32 or 2^64 is conservatively
refused. The 32-bit helper assumes the earlier general checks have rejected
negative lengths and nonempty null spans.

The host must select the correct target-width entry point, supply the exact
`sizeof(ScryptJob)` and actual original scratch extent, validate real
allocation accessibility and liveness including Wasm linear-memory bounds,
retain the same nonoverlapping storage, and serialize calls. The policy cannot
prove allocation size, read/write permission, aliasing through forged metadata,
valid object lifetime, memory remaining untouched, or erasure. No host adapter
uses it yet and it does not approve production-secret use.

Five Coax checks passed; the full mutation run killed 110/110 mutants (110
selective checks, 240 skipped by the scheduler). A separate integer model
compared 12,000 start and 12,000 output cases per 32- and 64-bit profile on
checked/release native and actual Wasm, including address wrap and overlap
boundaries. The emitted code compiled for arm64 iOS Simulator but was not
executed there. Python and Node are explicit test tooling only.

Independent review of the immediately preceding identical logic (source SHA-256
`8708ffe6d51687c17d7d3cf77556bfa3fb342c96635dcd295810058eeaca70fe`)
found no overlap bypass for admitted spans and prompted the explicit 32-bit
helper precondition comment. It also checked 500,000 math-level nonwrapping
span pairs separately. The final source SHA-256 is
`f874c00bba8d25477ca33bb802ef512ed78497d3208ea778a6ef7cfe6b0f61c0`.
This scoped review is not a pointer-safety or production approval.

The run used coax0 0.22.0 SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
Checked generated C SHA-256 `13ebbe63b6d649886a473efda53c5a1e7b1ae0a365ce593423e479f35065152a`,
Wasm `32c8f273e5011d76cc24746c7870a3e68ccddecbbad70e863ba6df0bf4f5390f`;
release C `8c90045a0271a29d751c77e87e33928d463bb4040206779a1a2096f4d404a6f4`,
Wasm `32c8f273e5011d76cc24746c7870a3e68ccddecbbad70e863ba6df0bf4f5390f`.

```sh
coax/bin/coax test coax/lib/crypto/scrypt_admission.coax
coax/bin/coax mutate coax/lib/crypto/scrypt_admission.coax --mutation-stats
python3 coax/tests/nostr/scrypt_admission.py
```
