# R2 source stopping point — 2026-11-02

The six source modules are frozen by `stable-source-v3.json`. The checker
SHA-256 is `11d2c2a971ab664b172a3ec074502511c7f6207f095f3971f74b2fba09679fef`.
The other five hashes remain unchanged from source-v2. Sum types and layout
are owned and qualified separately by the adjacent `r2-native` record.

The generated checked-constructor oracle failed before the fix: a checked
optional sum carrying `mark=1`, reconstructed as `expr_struct`, could pass
through `CK_prechecked` without resolving its target or payload in the new
generic context. Reconstruction now clears that marker and retains a
one-field initializer containing the saved alternative name and cloned
payload, together with the cloned type target. The four nominal/optional and
marked/unmarked cases pass. The freeze record explains each phase-recomputed
metadata reset removed by the focused cleanup. Existing A1 borrow resets,
resource guards, charge positions, the 1,048,576 work limit, and the eight-step
borrow path limit remain unchanged.

`final-borrow-authority-v3` records the final standalone external suite:
**1,204 checks passed**, with authority execution coverage enabled and source
pins unchanged. Its last two groups independently check every literal cache
key component and actual FIFO partial refresh/source reuse under the remaining
work allowance. These added groups were not followed by another mutation
batch after the requested stopping point; no improved mutation score is
inferred from them.

The stock root-only scopes move exact production procedures into an entry
source, preserving all baseline checks. Relevant check declarations run first
without changing their bodies. Each scope has its source, complete dependency
hash manifest, process record and raw report. These are component scopes and
must not be combined into a claim about the full checker.

| Scope | Mutants | Killed | Survived |
| --- | ---: | ---: | ---: |
| Front: resolve, clone, declaration diagnostics, exhaustive returns, run | 617 | 617 | 0 |
| B2 sum-carrier write/opacity integration | 172 | 172 | 0 |
| Index admission probe including B2 kind5 and addressed kind3 | 34 | 34 | 0 |
| Remaining index procedures | 139 | 126 | 13 |
| Literal cache | 77 | 63 | 14 |
| Projection cache | 290 | 215 | 75 |
| Variant lookup | 46 | 38 | 8 |
| Leaf count | 74 | 69 | 5 |
| Presence discovery | 132 | 129 | 3 |
| Expression origin flow | 226 | 214 | 12 |

All completed resumed runs used four workers and finished within five minutes.
The two earlier combined scopes stopped at their 840-second limit; their
scores are **inconclusive**, not zero. The prepared finer `CB_query` scope was
not run. `record.json` records both facts and an empty process-group readback.

R2's overall mutation qualification remains **open**. The residual reports
contain valid-state witness admission/reuse gaps, active-variant/optional try
coverage gaps, and unspecified private post-fatal return variations. No
mutant was waived as equivalent; no post-fatal contract was tightened to
manufacture a kill. Continue with independent valid-state and refusal
atomicity oracles in the recorded small scopes. Do not treat the passing
baseline, the front scope, or native packaging as a full R2 mutation pass.
