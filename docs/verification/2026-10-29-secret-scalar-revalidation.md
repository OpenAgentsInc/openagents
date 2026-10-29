# Scalar secret-source audit: whole-policy revalidation (#0af2)

**Scope:** the scalar-only profile (`unsigned-scalar-source-v1`,
`coax/lib/crypto/secret-flow.coax`) after the
[checked-arithmetic](2026-10-29-secret-audit-checked-arithmetic.md) and
[checked-cast](2026-10-29-secret-audit-checked-cast.md) corrections. The
three aggregate profiles (legacy, public-view, large) are not revalidated
here. Their tasks (#79d5, #f07b and the guard-binding tasks) keep their own
gates. This is not target timing, object-code, optimizer, JIT, speculation or
production-secret approval, and it does not approve signing, ECDH, encryption
or key custody.

The reviewer (claude-nostr) did not write the policy or its earlier fixes.

## What was run

All runs used the current main compiler, rebuilt from source in a clean
worktree: `coax0 0.22.0`, binary SHA-256
`6e385ae9e0d92cca16a8818ca11a9f5c594b19299958963c1812b272f1cc68b8`.
The policy is unchanged from the reviewed arithmetic correction:
`secret-flow.coax` SHA-256
`3e7385219a694808e7d55caca47a0abe631de07911062c4a1c29605d79c9bba8`, with
shared tests `coax/tests/nostr/secret_flow.coax`
`17fb7be86bed9c44bb15cff5a7eae76c9bc98eb8657c2846ece2dc82a2512cd0`.

| Check | Result | Record |
| --- | --- | --- |
| `coax test coax/lib/crypto/secret-flow.coax` | 41/41 | [scalar-checks.log](data/secret-scalar-revalidation-2026-10-29/scalar-checks.log) |
| `coax mutate … --mutation-reference --mutation-stats` | 749/749 killed, 8,891 full checks, 0 skipped, borrowed or unstable, no waivers | [scalar-reference.log](data/secret-scalar-revalidation-2026-10-29/scalar-reference.log) |
| `test_secret.py` (real frontend and closure, checked and release masked select, lost-origin control, ASan/UBSan) | 7/7 after the cb_mask correction below | [source-cases.json](data/secret-scalar-revalidation-2026-10-29/source-cases.json) |
| `test_revalidate.py` (new; generated programs and two independent oracles) | 3/3 | [revalidation.json](data/secret-scalar-revalidation-2026-10-29/revalidation.json) |

## Finding: cb_mask is no longer accepted

`cb_mask` (`coax/lib/crypto/bytes.coax:139`) computes
`value_barrier((bit & 1) * CB_MAX64)` with checked multiplication. The
arithmetic correction refuses any checked arithmetic on a secret operand. The
corrected policy therefore refuses `cb_mask` (`unsupported`, line 139). The
emitted checked C calls `coax_mul_u64` on the secret, and that call has an
overflow check. The product cannot overflow, because `bit & 1` is 0 or 1. The
refusal is conservative and correct for this profile.

`test_secret.py` still asserted that `cb_mask` was accepted, so it failed on
main before this change. The earlier correction recorded Coax checks and
mutation, not this real-source test. The test now expects the refusal, and the
README no longer lists `cb_mask` as accepted. `cb_ct_select32` and
`cb_ct_select64` are still accepted, and their emitted checked and release C
has no guard that depends on an operand.

`bytes.coax` is unchanged. An admissible rewrite would be
`value_barrier(mul_wrap(bit & 1, CB_MAX64))`, which returns the same values.
It is an `#authority` file that the signer, ECDH and #092b depend on, so that
rewrite needs its own authority checks, full mutation and caller revalidation.
It is not part of this task.

## Independent oracles (test_revalidate.py)

The test generates 400 well-typed programs (seed 20261029). They mix the
admitted grammar with the refused constructs nearest to it:

- admitted: masks, shifts by literals, wrapping operations, rotations,
  `value_barrier`, public `ifx`, truncating casts and helper calls;
- refused: checked arithmetic, checked casts, secret shift and rotate counts,
  and secret conditions.

Every program goes through the real audit tool. None fails to compile.
Results: 257 programs were accepted. Refusals were 70 `unsupported`, 48
`secret-shift` and 25 `secret-branch`. Each of the following must hold for
every accepted program:

1. **Emitted-C guard scan.** The scan covers the entry and every procedure it
   reaches in the emitted checked and release C. It allows no `if`, `while`,
   `for`, `switch`, `goto` or index. It also allows no `?:`, `&&` or `||` whose
   condition or left side reads a parameter, a local or a secret emitter
   temporary. Calls are limited to the wrapping, rotation and barrier
   lowerings, shifts by an in-range literal count, and runtime helpers whose
   arguments are all public. The scan shares no code with the Coax policy.
2. **Execution.** Each program is compiled checked and `--release`, both at
   `-O2`. On 32 inputs, including all-zero, all-ones, high-bit and random
   values, it must return the value that independent Python big-integer
   evaluation gives. A trap fails the test. That is 8,160 cases per mode.
   Two accepted programs trap the same way on every input, from checked
   arithmetic on constants only. They are recorded and not executed.

Twelve refused controls must fail the guard scan, so the scan cannot pass
vacuously. The controls are: secret checked add, subtract and multiply,
division, compound addition, narrowing and widening checked casts, a secret
shift count, a secret `ifx`, a secret short circuit, a secret index and a
secret `if`. They include the shapes of both earlier false acceptances. A
real masked select passes the scan.

The review also checked each admitted construct by hand against the
compiler's emitted helpers. Value shifts discard high bits, and only the count
traps (`coax_shl_*`, `coax_shr_*`). Rotations reduce the count without a
branch. Unsigned unary minus, mixed widths, implicit widening and narrowing at
calls and assignments, and `cast,trunc` to or from `bool` fail type checking
before the audit. No other bypass of the scalar profile was found.

## Gaps that remain

- Both oracles are source-level and C-level only. A C compiler can still turn
  comparisons, selects or masks into branches. Object timing and platform
  instruction timing are not checked.
- The guard scan is a lexical check of this compiler's emitter output. A new
  emitter shape needs the scan to be reviewed again.
- The 400 generated programs are a sample, not a proof. Programs have one
  binding and a depth of at most four, over `u64`, `u32` and `bool`.
- Arrays, loops, aggregates, conditional build selection and descriptor
  imports are still refused. Public loops get no exception.
- Aggregate-profile acceptances are still suspended, as the correction
  advisories state.
- No `#secret` syntax exists or is advertised.
