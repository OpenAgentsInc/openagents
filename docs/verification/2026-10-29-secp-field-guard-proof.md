# Conditional field guard proof

This is historical evidence for the pre-admission source graph. The [changed-source reproof](2026-10-29-secp-field-guard-reproof.md) records the later raw-peer boundary and its own reproduction artifacts.

This evidence concerns the three source assertions reached from the pinned fixed-base and variable-base multiplication roots: `secp_field.coax:239` and `secp_field32.coax:70,92`. The arithmetic proof establishes that these assertions succeed under the caller conditions below. The assertions remain in the source, and the source auditor still reports secret-derived guard obligations; this report is not an automatic audit waiver, target-code constant-time finding, erasure finding, or production-secret approval.

The exact source and query hashes, compiler and solver identities, generated-artifact hashes, counterexamples, and results are recorded in [the accompanying JSON](2026-10-29-secp-field-guard-proof.json). The model is conditional: independent manual source-to-model and caller review found no remaining arithmetic gap for the pinned roots under the stated premises. The model is not a machine-verified translation of the source.

## Preconditions and caller scope

For `sf_normalize`, each of the first nine limbs must be at most `8*(2^26-1)` and the top limb at most `8*(2^22-1)`. The `sf32_add` proof requires canonical operands in `[0,p)`, where `p=2^256-2^32-977`. The `sf32_mul` fold bound holds for arbitrary eight-word unsigned 32-bit inputs. These are preconditions of the proof, not new input checks.

The reviewed fixed-base roots start from bounded canonical generator and infinity values, generate their tables from those inputs, and use field operations that restore the required bounds. Complete zero/all-ones selections preserve an operand. The variable-base root requires bounded raw peer coordinates **before** `sg_valid_mask` or `sg_normalize` performs field arithmetic. It eagerly normalizes the peer, then selects between that normalized value and canonical infinity before converting to the 32-bit representation; subsequent 32-bit group operations preserve canonical values. An arbitrary caller of an exported raw-point interface does not establish the peer bound merely by invoking validity checking. The conversion helper alone is not a general normalizer. These conclusions do not extend to arbitrary direct helper callers.

The `sf_multiply` caller lemma was reviewed separately against its existing `SF_MUL_BOUNDS` certificate and carry sequence: its normalization input has magnitude at most two. This lemma is a manual source review and is not one of the 17 solver queries. The review also traced canonical subtraction, packing, and full-mask selection. It does not establish compiler lowering, object or JIT timing, or secret handling outside these assumptions.

## Arithmetic and controls

With `M=2^256` and `c=2^32+977`, the bounded normalization input is at most `8*(M-1)`. Its first carry is at most seven; the next folded value is below `M+7c`. If the second carry is one, the residual is below `7c`, so its last addition remains below `8c<M` and the asserted final carry is zero.

Canonical addition has `a+b<2p`. When the first addition carries, the low part plus `c` cannot carry again. The product fold has first-fold top at most `c`, leaving a value below `M+c^2`; if that fold carries, the residual plus `c` is below `c^2+c<M`. The model also checks word bounds, limb identities, canonicalization, subtraction, packing, digit carries and complete masks. Fourteen negated obligations are UNSAT; three negative controls are SAT. In particular the noncanonical all-ones add produces a guard counterexample, and the widened-magnitude control breaks the intermediate carry bound; it does not assert that the final normalization guard fails for every input of magnitude nine. A changed pinned source is refused.

## Reproduction and observations

The retained public artifacts were produced on macOS arm64 using Coax 0.22.0, Apple clang 21.0.0, Python 3.15.1 and Z3 5.3.2. The JSON records the exact compiler, provenance helper and record, solver library, source graph, scripts, source maps, emitted C and binary hashes. The test runner checks compiler provenance and compiler-recorded source hashes before and after its runs. Public generated C, maps, binaries and proof output were retained locally under `coax/bin/field-guard-proof-v1`; this ignored directory is not part of the commit. To reproduce in a new empty retained directory with a matching compiler and Z3 installation:

```sh
python3 coax/tests/nostr/field_guard_witnesses.py --retain coax/bin/field-guard-proof-repro --report /tmp/coax-field-guard-proof-repro.json
coax/bin/coax test coax/tests/nostr/field_guard_adapter.coax
```

For each checked and release build, the independent integer oracle checked 524 bounded normalizations and 548 canonical pairs for both addition and multiplication. AddressSanitizer and UndefinedBehaviorSanitizer harnesses passed. A noncanonical all-ones addition aborted at the actual `secp_field32.coax:70` assertion in both modes. An all-ones raw normalization input is outside the stated limb contract: the checked build aborted on an integer overflow, while the release build returned normally. Neither observation supplies a raw-input admission check. The adapter test passed 41 checks; the runner also rejects execution with Python assertions disabled.

The previously recorded full authority mutation runs apply to unchanged source closures: `secp_field.coax` and its `bytes.coax` dependency match commit `a14e2cccc9` (678/678 killed, [field evidence](coax-secp-field-inv-2026-10-28.md)); `secp_field32.coax` and its `bytes.coax` dependency match commit `f020e61b2c` (244/244 killed, [variable-base evidence](2026-10-29-secp-variable-base.md)). These are historical source-matched results, not a claim that those mutation runs used the compiler in this report. No public ABI, existing resource limit, or production implementation was changed by this proof harness.
