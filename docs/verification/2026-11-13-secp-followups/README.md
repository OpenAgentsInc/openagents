# secp256k1 follow-ups (#13454)

This change follows #13450. Public verification, secret arithmetic and
qualification are separate concerns; a numerical oracle is not a timing or
secure-erasure approval.

## Implementation

`secp_modinv.coax` implements public Bernstein–Yang inversion with signed
62-bit limbs, 62-divstep matrices and explicit two-word signed products.
Both the field and scalar public inverses use it. The prior Fermat field
chain and binary scalar inverse remain independent references. Fixtures
include Python modular inverses and matrix/update/batch traces from pinned
libsecp256k1 v0.8.0; its MIT notice is retained.

The secret Sf32 group interface shares the 5×52 product, square and reduction
schedule through `secp_fe_ct.coax`. `secp_fe_columns_gen.coax` generates
static public and fixed-work variants from one authoring schedule. Public
carries use comparisons; secret callers never reach those `_vartime`
helpers. There is no runtime arithmetic-profile switch. Carries use bit operations, normalization uses
masked subtraction, and column selection depends only on a public loop
index. Owned numeric workspaces are wiped. Signing and ECDH retain their
existing fixed-work group formulas and table selection. An unreleased
Schnorr signature continues to use the existing self-verifier. This does
not authorize variable-time verification of unreleased signatures.

The public G table has width 10 and 256 packed affine points. Its immutable
2,048 words are indexed directly: disassembly exposed, and this change
removes, a 16 KiB automatic-array copy on every lookup. The Coax generator
reproduces the checked-in table. The generator's extra denominator is
combined before squaring, saving one field product per addition.

Public scalar multiplication uses a 512-bit product folded by `2^256−n`.
BIP-340 challenge hashing starts from its tagged SHA-256 midstate. Batch
verification, recovery, Taproot, public BIP-32 derivation, MuSig2 and FROST
public operations use the fast group; the earlier field/group remain their
references. Relay and script entry files have explicit compile-time budgets.

## Qualification limits

The fixed-work source schedule does not prove the target's timing, checked
runtime guards, optimizer behavior, register erasure or compiler-created
by-value copies. Existing production-secret approval gates remain in force.
The aggregate source audit currently cannot compile its existing
`AG_fixture` test because of E0381 storage accounting. No aggregate source
audit or target-security approval is claimed here.

The Wasm implementation continues to use the portable shared `mul_wide`
lowering. There is no new SIMD instruction dependency or separate handwritten
Wasm field. Width 10 spends additional immutable data; the affected module
sizes and measured timings are recorded with the final checks.

## Checked results (2026-11-13)

Against starting revision `d7f0dfd3e1b32e242eea9c0f9e39caad217eebf5`,
a paired Linux x86-64 GCC 12.2 checked `-O2` run agreed with pinned
libsecp256k1 v0.8.0 on all 1,536 decisions. The benchmark uses 256 seeded
fixtures, seven passes, and sixteen signing fixtures. Process CPU time
excludes preemption by unrelated host jobs. These are host-specific times,
not Apple M5 measurements.

| Operation | Starting Coax µs | Changed Coax µs | libsecp µs (changed run) |
| --- | ---: | ---: | ---: |
| ECDSA verify | 100.26 | 68.55 | 38.15 |
| BIP-340 verify | 83.75 | 70.02 | 38.76 |
| ECDSA sign | 615.68 | 558.01 | 24.29 |
| BIP-340 sign | 2999.28 | 2605.52 | 36.26 |

The public field inverse measured 3.96 µs versus 6.37 µs for its Fermat
reference; scalar inversion measured 4.03 µs versus 23.99 µs for the binary
reference. Inlining the shared small field sum/scale/negation helpers removes
repeated by-value call copies in native point doubling.

A paired Node v24.22.2 wasm32-wasip1 run, with 2,048 untimed warmup calls
and thread CPU time (excluding background tier-compilation CPU), measured
187.50 → 187.50 µs for ECDSA and 203.07 → 187.49 µs for BIP-340. The test
module grew from 83,944 to 100,961 bytes. No Wasm SIMD or separate field is
claimed: this change improves BIP-340 and keeps ECDSA level on this target.

Complete scoped mutation results: divsteps 283/283 killed; wide public scalar
product 193/193; tagged challenge 89/89; regenerated field columns 404/404;
fixed-work adapter 211/211; total field adapter 88/88; ECDH 148/148.
The field root killed 358 mutants with zero survivors and its one pre-existing
equivalence annotation. The public fast group killed 275 with zero survivors
and fifteen pre-existing equivalence annotations. BIP-32 killed 654 with zero
survivors and forty-nine pre-existing annotations. No new equivalence
annotations were added. Batch's eight completed root shards killed 368/419;
its 51 survivors are on unchanged sites and pass the changed-lines gate.
Taproot's thirteen survivors are likewise in unchanged code; this does not
claim a full zero-survivor result for those older units.

Independent checked/release native and Wasm transcripts cover recovery,
BIP-32, every pinned MuSig2/FROST vector and seeded additional cases, besides
retained parity checks against the original group. Nostr and Bitcoin book
structure/link checks pass (819 and 865 linked listings).

On GCC 16.2/Nix, Valgrind's release signer probe reports only `sw_key`,
`sm_equation` and `sw_publish`, its three existing declared decisions. The
fixed-work self-verifier reports no candidate-dependent branch or address
for valid or invalid candidates; both variable-time positive controls are
flagged. Checked self-verification reports 79 checked-arithmetic/bounds
traps per candidate and zero sites outside those declared trap categories.
The probes now refuse Valgrind startup failures and the signer harness
handles an empty report set without skipping its positive control. These
observations do not remove the target/erasure qualification limits above.

Release Wasm sizes pass their budgets: nip01 105,557 bytes, nip-guide
140,527, pylon 117,283, status 75,903, valleycanyon 118,419,
valleyworld 158,711 and splice-bots 715,384. The frozen homepage build is
1,175,780 bytes and retains its existing 1,180,000-byte budget; a full website
build also refreshes unrelated baked Nostr content, which is not part of
this change. Raised budgets describe the measured wider-table/shared-field
cost. Existing near-limit compile-time notes remain warnings.
