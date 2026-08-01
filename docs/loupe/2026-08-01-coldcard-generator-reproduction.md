# Coldcard generator and owned-fixture reproduction

OFR-015 implements an independent TypeScript reproduction from the pinned
Coldcard, libngu, and MicroPython semantics. The implementation does not import
or execute target source. The target revisions remain those in the frozen
Coldcard reproduction manifest.

## Reproduced behavior

The evaluator models uint32 overflow at every Yasmarang state update, libngu's
independent public initial state, the selected provider stream, XOR combination,
little-endian partial-word copies, the adjacent-provider-output health check,
and `mp_obj_get_int_truncated`'s 32-bit reseed behavior. Its uniform sampler
retains the target's mask and retry behavior, and the keypad trace applies the
target's reverse Fisher-Yates loop.

The frozen corpus contains vulnerable, 32-bit-reseeded, approved-provider, and
five semantic mutation classes: guard, provider, initialization, call trace,
and reseed truncation. Each vector binds initial state, output, full digest-only
call trace, keypad permutation, retained width, and admitted worker profile.

## Entropy and work estimates

Work factors are derived rather than copied from prose. Each dimension names an
assumption and correlation group. The calculator takes the maximum dimension
inside a correlated group and multiplies across independent groups, preventing
timer and RTC ranges from being treated as independent without saying so. It
then divides the exact candidate count by a measured OpenAgents Cloud
throughput receipt with ceiling division. Hardware class, firmware, assumptions,
profile, throughput, candidate count, and projected duration remain reviewable.

## Secret-safe fixture boundary

Reproduction accepts only `synthetic` or `owner_authorized` fixtures. It derives
BIP39 and BIP32 material in memory across BIP44, BIP49, and BIP84 fixture paths,
compares digests of the expected xpub/address material, clears mutable entropy
and seed byte arrays, and returns only a receipt. The receipt contains no
mnemonic, xprv, or address, and records `liveValueLookupAttempted: false` and
`retainedSecretMaterial: false`. There is no network or value-oracle argument
in the reproduction API.

Artifact selection, generator behavior, entropy/work factor, and owned-fixture
matching retain separate references. A match at one rung does not promote
another rung.

The implementation is in
`packages/forensic-contract/src/coldcard-generator.ts`; frozen inputs are in
`fixtures/forensics/coldcard/generator-vectors.v1.json`.
