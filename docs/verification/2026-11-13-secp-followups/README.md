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
index. Owned numeric workspaces are wiped. An unreleased Schnorr signature
continues to use the fixed-work self-verifier. This does not authorize
variable-time verification of unreleased signatures.

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

## Signing and ECDH on the 5×52 field

`secp_ct_group.coax` is a fixed-work group on the shared 5×52 field for
secret scalars. Signing (`k·G`, `1/k`), ECDH (`k·P`, `1/Z`) and the BIP-340
self-verifier (`s·G - e·P` and its final sum) run on it:

- Products split each limb product at bit 52 into separate 64-bit low and
  high columns, which cannot overflow, so no carry is detected; two carry
  passes and folds by `2^260` and `2^256` (mod p) give magnitude 1.
  Inputs may have magnitude up to 8; the checks cover every magnitude.
- Addition is libsecp256k1's complete `secp256k1_gej_add_ge` formula,
  degenerate case (`y1 = -y2`, `x1 = βx2`) included, scaled by 2 instead of
  halved. Every condition is a mask and every choice a masked select.
- `k·G` is a comb of 8 blocks, 4 teeth, spacing 8 (7 doublings, 64 affine
  additions) over the generated `secp_ct_table.coax`; each lookup reads
  all 16 entries of its block. `k·P` keeps the signed radix-16 digits of
  `smc_multiply`.
- `1/Z` is Fermat's fixed chain (255 squares, 15 products). `1/k mod n`
  is `k^(n-2)` with 4-bit windows of the public exponent; its products use
  32-bit columns with three folds by `2^256 - n` and one masked
  subtraction, any 256-bit inputs.

The 26-bit comb, ladder and Montgomery inverse stay as references, and
`#forall` checks compare each new path with its reference. Tables, digits,
scalars and accumulators are erased; products and coordinates live in
by-value locals, as in every group formula (the register and copy limits
below apply).

Linux x86-64 build host, GCC 12.2 `-O2`, release build, process CPU time,
paired runs against libsecp256k1 v0.8.0 (µs):

| Operation | Before | After | libsecp256k1 |
| --- | ---: | ---: | ---: |
| ECDSA sign | 562.15 | 126.11 | 25.41 |
| BIP-340 sign | 2569.66 | 320.47 | 36.54 |
| ECDSA verify (unchanged code) | 62.27 | 62.11 | 38.28 |
| BIP-340 verify (unchanged code) | 63.69 | 63.59 | 38.75 |

Parts (`secp_bench.sh --release 0 signparts`): `k·G` 47.3 µs, `k·P`
150.9 µs, `1/k` 28.2 µs (the Montgomery chain took 86.3 µs), field inverse
6.5 µs, one HMAC-SHA256 5.9 µs. In Node v22.11.0 (`secp_bench.sh --wasm`,
which now times the signers too), ECDSA signing went from 873.39 to
235.23 µs and BIP-340 signing from 4,213.33 to 974.41 µs; the benchmark
module shrank from 226,872 to 203,069 bytes.

Wasm SIMD by compiler flag is a measured dead end: the same benchmark C
built with `-msimd128` verifies in 239.01 (ECDSA) and 238.71 µs (BIP-340)
against 245.04 and 243.57 µs without it in the same run, within the noise
between runs, and `-msimd128 -O3` in 241.98 and 244.75 µs. Wasm SIMD has no 64×64→128-bit product, so the 5×52 columns do
not vectorize; a SIMD field would need its own 26-bit lane layout
(`i64x2.extmul_low_i32x4_u`), written by hand.

What remains to libsecp256k1's signing time, measured: BIP-340 signing's
self-verification spends 151 µs in `k·P` (256 doublings at 0.2 µs, 64
additions at 0.85 µs); GLV would halve the doublings. ECDSA's `1/k` (28 µs)
would drop to a few µs with a constant-time divsteps inverse, as
libsecp256k1 uses.

Checks: every touched Coax suite (`coax test ... --library --run-steps
250000000`); `schnorr_sign_libsecp.py` (3,100 signatures equal to
libsecp256k1's) and `ecdsa_sign_libsecp.py` (1,200); the native and Wasm
suites `secp_base.py`, `secp_mult_const.py`, `ecdh_vectors.py`,
`schnorr_self_verify.py`, `schnorr_sign.py` and `ecdsa_sign.py`. The
Valgrind probes on NixOS (valgrind 3.26) pass: the ECDSA signer reports only
its `ec_declassify` line, the BIP-340 signer only `sw_key`, `sm_equation`
and `sw_publish`, and the release self-verifier no candidate-dependent
branch or address; every positive control is flagged. The ECDSA probe's
planted branch now follows `ec_seed`, which replaced `ec_candidates` in the
signer, and the self-verifier probe counts checked `coax_mul_u64` traps
with the other checked-arithmetic traps (1,758 a candidate, none outside
them; Valgrind's 1,000-report cap is lifted).

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
