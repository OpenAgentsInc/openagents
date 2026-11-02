# B2 early-exit correction

**Publication follow-up:** the owner explicitly accepted the measured **4,251
spare steps (0.41%)** and authorized rebasing and pushing the tested fix. The
checker and recorded compiler inputs remain unchanged after rebase. The budget
remains **1,048,576**. See [publication.json](publication.json) for the accepted
margin and rebase continuity; the original measurements below are retained
without relabeling them as newly executed checks or a normal bootstrap.

The bounded raw return-origin implementation, its canonical seed and scoped
capability documentation are included in this publication. Retained-only manual,
foreign-retention, B3 and normal release/bootstrap boundaries remain explicit.

## Original pre-approval stopping-point report

The retained-only fallback now preserves its original short circuit: once a
result is already incomplete, the opacity-only scan is skipped. Complete
results still run that scan. Raw B2 helper-write composition still runs even
when incomplete, preserving known unsafe local-backing witnesses. The shared
1,048,576-step budget and its counter implementation are unchanged.

The final full compiler source **compiles successfully**, using **1,044,325**
analysis steps. That leaves **4,251 steps (0.4054%)**. Before measurement, this
continuation set a publication requirement of at least **10,486 spare steps
(1%)**. It misses that margin by **6,235 steps**, so **nothing was pushed** and
no further optimization pass was started.

### Focused evidence

- A new paired-graph regression fails before the fix on the extra work, then
  passes both cases after the fix. The complete/default case requires opacity
  analysis; the already-incomplete case requires no additional opacity scan.
- Three deliberately broken guards are all caught: unconditional scan,
  unconditional skip, and `||` changed to `&&`.
- The same new regression passes six native observations across O0, O2 and
  ASan+UBSan. No leak qualification is inferred from Apple ASan.
- The focused B2 helper mutation initially found six untested helper cases
  after the prior candidate's inline refactor. Two existing check groups now
  directly exercise the helper on their existing graphs: sixteen amended
  cases pass, and all **220/220 mutants are killed**, with zero survivors.
- Twenty-one source/internal-origin cases pass O0/O2/ASan+UBSan: **63 native
  observations**. Only check bodies changed afterward; exact runtime and load
  continuity retains those results and the early-exit native/guard results.
- The newly linked development compiler passes **21 physical source fixtures**
  and the **two affected existing helper-carrier fixtures**.
- Canonical changed-unit C/header generation succeeds with implicit authority
  checks. The generated checker header remains byte-identical. The development
  link rebuilds the changed checker and reuses seven unchanged, hash-validated
  native objects. This is not a normal release/bootstrap qualification.

The full-input observation includes a successful compiler exit, generated
header, observed final counters, and unchanged source/producer identities.
Its elapsed 39.1 seconds is an observation on a shared host, not a throughput
benchmark. The final source SHA is
`759e1739e64fa447d8f5b2b48d2a499e3ed7180c566de6af08fdde48f1f82fd9`.

[record.json](record.json) gives the decision and exact identities;
[full-compiler.json](full-compiler.json) retains phase counters and process
outcome. The earlier failed candidate and its immutable history remain in
[the preceding record](../coax-helper-write-returns-2026-11-01/README.md).

### Local handoff

The fix, checks and canonical generated seed are in this private candidate
working copy. No main branch, GitHub issue, public release or historical
verification record was updated by this continuation. The six prospective
capability docs were saved separately, then restored to their original content;
they do not claim that B2 has shipped.

[source-and-tests.patch.gz](source-and-tests.patch.gz) contains the current
candidate source/tests and canonical seed. [proposed-current-docs.patch.gz](proposed-current-docs.patch.gz)
is a prospective documentation draft with pending-gate markers; it must not be
applied as a delivered-capability claim. Both gzip payloads were decompressed
and compared byte for byte. Regenerate/requalify after any source rebase.

[proof-manifest.json](proof-manifest.json) maps original observation paths to
`sha256/HASH` members in `proof.tar.xz`. Every archived member was read back and
hash-verified. Native executables and objects have hash identities only.
Failed authoring/setup attempts, the initial six mutation survivors, final
passing checks, canonical generation, native link and full-input measurement
are all retained with their original receipts.

The remaining publication gap is analysis headroom. R2 and broader B3 request
ownership remain separate work. This continuation is stopped; no other issue,
chapter, cache redesign or additional optimization was started.
