# Fixed-base comb multiplication

Task `secp-mult-base` (#94a3). This is arithmetic/build evidence, not approval
to use the implementation with production secrets. The authored range guards,
compiler/object/JIT paths, independent security review and secret-copy custody
remain separate gates. The public variable-time verification module
`coax/lib/crypto/secp_mult.coax` stays unchanged.

## Representation and schedule

`secp_mult_base.coax` owns the public generator and the fixed-work online
algorithm. Two wrappers choose actual compile-time tables:

| Profile | Teeth | Spacing | Entries | Point bytes | Table bytes | Cap |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Page | 7 | 37 | 128 | 240 | 30,720 | 32 KiB |
| Native | 8 | 32 | 256 | 240 | 61,440 | 64 KiB |

Each entry sums the selected basis points `2^(tooth*spacing) G`. The generator
uses curve addition and doubling in Coax, with no host-generated coordinates.
It validates public dimensions and exact output length before writing caller
storage. Small public configurations exercise the same generator/storage
adapters in ordinary checks. The actual full tables must also be compared
entry by entry against the pinned independent libsecp oracle.

Online multiplication normalizes the eight-limb scalar modulo the curve
order and copies it into nine limbs. The ninth is zero padding for page bits
256–258. The first public row selects directly from infinity; later rows
always double and add. Every selection scans all entries, reads only public
indices and uses an opaque complete equality mask to select all30 coordinate
limbs. There is no scalar-dependent branch, loop bound or table index in
this source schedule. This property is distinct from machine-code timing.

`smb_pick_generated` is private to the generated-table path. Its inputs must
be canonical, valid, magnitude1 points from the actual Coax generator. It
does not validate arbitrary external points. A missing digit or empty span
produces canonical infinity; real wrapper digits contain exactly the declared
number of bits and are in range. Checks compare it independently with the
general validating `sg_pick`, exercise every digit bit and verify coordinate
bounds/infinity. Group kernels normalize their results, including infinity.
Zero and order-multiple scalars therefore produce canonical infinity; this
mathematical API does not establish that a signing key is valid.

Wrappers assert the actual `SgPoint` size, scalar coverage and table-byte
caps. The current mutable-view ABI copies each immutable table into a local
public scratch array; this is additional stack use, not a hidden second
private-key table. Exact emitted object/stack observations belong to each
recorded target/build below.

The Wasm test build explicitly reserves a 1 MiB stack: an initial provisional
run with the linker's default stack trapped on the full table path. This test
setting is a build requirement for that harness, not evidence that an
unconfigured browser host has sufficient stack. The provisional native arm64
`-O2` compiler recorded 30,800 and 61,520 bytes for the page and native
wrapper frames respectively, plus nested calls; final target stack budgets
remain to be established.

## Explicit build resources

The unchanged default100,000 aggregate compile-time steps cannot evaluate even
one generator-point doubling: the real compiler refuses it with E0400.
Pure public fixtures without a `#run` declaration measured:

- Page generator plus two equality assertions: **63,698,174 steps**.
- Native generator plus two equality assertions: **103,614,101 steps**.

These measurements used `coax test --steps1000000000` solely as cost probes.
They do not satisfy actual compile-time generation, mutate admission or an
ordinary check gate. The separately reviewed compiler resource profile uses
the explicit build input `--run-steps250000000`, with the default unchanged.
Both actual tables, including the combined adapter, must compile under that
bounded input. Check/mutation budgets stay unchanged at20,000,000 steps per
baseline check; a full online call fits that existing budget for either
profile. Pre-folded constants do not count as generator execution coverage.

## Reproduce the affected evidence

Normal compiler/application/bootstrap builds do not gain a Python dependency.
Python here is explicit test acquisition and an independent arithmetic oracle.
The libsecp artifact is reused only after its source/adapter/artifact pins
are validated. No test loads a real identity or production key.

```sh
coax/bin/coax test coax/lib/crypto/secp_mult_base.coax
coax/bin/coax mutate coax/lib/crypto/secp_mult_base.coax
coax/bin/coax test coax/lib/crypto/secp_mult_base_web.coax --run-steps 250000000
coax/bin/coax test coax/lib/crypto/secp_mult_base_native.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/crypto/secp_mult_base_web.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/crypto/secp_mult_base_native.coax --run-steps 250000000
COAX_BASE_REPORT=/tmp/coax-fixed-base.json python3 coax/tests/nostr/secp_base.py
python3 coax/tests/nostr/secp_base_timing.py
```

The correctness runner requires100,000 random scalars per variant in checked
and release native/Wasm, plus scalar/order/bit boundaries; it also checks all
generated table entries against libsecp, canonical output, short byte streams,
guard bytes and ASan/UBSan. Its source/compiled-object hashes, exact build
input and stack-usage records are retained in its report.

Timing uses the existing Coax Welch classifier with independent recomputation:
one million balanced fixed-zero versus seeded256-bit public observations per
profile, random adjacent class order,20,000 warmup calls and a predeclared
absolute-t threshold10. It retains every raw CT01 observation, reports the
actual clock/object/target and includes fixed-work and planted-branch controls.
Alarms are retained, never filtered or searched away. A no-difference-detected
result would apply only to that declared experiment; browser/JIT, other
microarchitectures, higher-order leakage and production approval stay open.

## Current execution

Work in progress: the72 core check groups pass under the unchanged check
budget. An initial mutation run killed1,164 of1,168 mutants; profile-boundary
and first-entry checks were strengthened, and the rerun killed all1,168.
Wrapper mutation, the compiler-resource prerequisite and declared timing
acceptance are still pending. Cost-probe success has not been reported as an
actual `#run` success.

The updated source passed the full correctness runner with resource-tested
compiler SHA-256 `4942904896b469e77840506319d5f1d09b5a35016a5c63f66fac6a022d0cbde0`:
100,000 seeded random scalars and517 boundaries per profile in checked and
release native and Wasm, all768 table entries across both native modes
against pinned libsecp, and sanitizer and guard checks. The
[correctness report](2026-10-29-secp-fixed-base-correctness.json) retains
source, compiler and artifact hashes, compiler resource counters, native
stack and object sections, and Wasm engine and stack settings. The combined
table build took166,665,133 evaluator steps, with a peak38,376 tracked slots
and7,264 containers. The compiler's complete bootstrap, mutation and timing
gates are separate and remain pending.

A provisional compiler with the proposed resource input generated both real
tables before the subsequent profile-check refactor. Its checked and release
native smoke compared all768 table entries across both modes and517 boundary
scalars per profile/mode with pinned
libsecp; both wrapper check suites passed72/72. After the explicit stack
setting above, checked and release Wasm boundary smokes passed too. This is
preliminary: the compiler's resource-bound validation and the full random
and timing runs are still outstanding. The provisional reports are retained
at `/tmp/coax-fixed-base-provisional-{smoke,wasm}.json` on the host.

As an early arithmetic probe, public tables generated at runtime passed an
independent affine oracle for all128 page entries and all256 native entries,
and for1,515 boundary and seeded scalar cases per profile. A wasm32/WASI
probe using Node V8 passed728 comparisons across364 cases for both profiles,
including limb bounds, canonical infinity and output guard bytes. A native
ASan/UBSan probe passed all256 single-bit scalars for both profiles. The
probe sources and outputs are retained in `/tmp/coax-fixed-base-runtime-*`
on the host. These preliminary observations do not exercise the actual
compile-time tables or satisfy the independent libsecp, timing or target
acceptance gates above.
