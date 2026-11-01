# Bounded returned-borrow qualification

The checker follows returned view/string descriptors through resolved helpers,
selected aggregate fields and concrete generic calls. Known local backing is
E0383 even when another arm is unknown. Inline storage and descriptor pointees
stay distinct. Per-declaration memo chains keep the 1,000-return regression
within the existing work limit. The merged compiler also repairs query
traversal/cache-mode handling and honors authored #steps in ABI2 domains.

Qualified source: `44b25a4b3e83797ed8d6d3ffd9ca885aab8c1449`, based on
`6225954fae6e81935dae3d19d9024b3c24cda218`. Compiler fingerprint:
`7c749e9b2c4f487d74a0b506393d104104cafcbe058ec6c39d4af1fb7e956f2d`.
Actual image: `5c37377a72fbba0a4e11a6ffdba2bfa37c223f8490b158aa23bc1a75488c53d9`.

Passed checks:

- All 165 strict/full/fresh normal statements, exact seeds and both fixed
  points; all 8,147 full checker mutants killed with zero survivors.
- 39 original source cases and the 1,000 independent-return regression.
- Nine native layout/type/status methods with generated-header guards; actual
  System One return helpers at O0/O2/ASan+UBSan, with 129 digit and 45 stderr
  cases per profile.
- Four default-construction methods including Wasm, with nine emitted-C
  reports and zero skips.
- Whole query and step-budget drivers, plus two E0220 negatives. The warm
  compiler query measured 248 ms, best of three.
- Seven diagnostic, three ordered-lowering, one wrapping and 18 release tests.

[Integration record](integration.json) names actual outer transport records.
[Archive record](record.json), [manifest](manifest.json) and [scope](scope.json)
bind [proof.tar.gz](proof.tar.gz), including the self-verified completed-normal
package, all 10 final gate receipts, raw observations and selected artifacts.
Private receipt paths are original locations; manifests map retained bytes.
The exact source reconstruction patch requires the admitted base Git object.
Main11/release internal temporary variants are not all individually retained.

The initial defaults setup invoked physical lld instead of its wasm-ld alias.
That failed probe and its outer transport remain failed. The continuation
retained six successful probes, corrected only argv0 dispatch, and ran all four
previously unexecuted methods once. No product code or test assertions changed.
V11's 61/163 prefix remains incomplete in historical precondition metadata.
[Earlier v10 evidence](../coax-return-borrow-normal-2026-10-31/README.md) stays
at its original source/image; it is not this qualification.

A complete origin relation is not a general lifetime-safety verdict. Unknown
recursive/helper-store paths remain manual. System One digits/stderr return
execution does not establish so_post composition or request/cancel ownership.
Existing default-fixture byte comparisons establish no general C-padding
contract. Retained-state writes and request ownership are the next linked
child; the borrow umbrella remains open.

This record qualifies the implementation. Fresh changelog renders and website
publication follow the pushed release identity and are recorded separately.

Runtime commit `6784112fb02d3d418747e63923e22c6831c68c86` is pushed. The actual
rebase base was `5426f4af89bcbe17b6e3c39132fc4de0d06efb9e`; the shared remote
reference advanced after the earlier incoming-book review. No compiler
fingerprint input changed, and post-push provenance verification passed. The
first book comparison against the older incoming ledger stopped and is not
counted as a pass. Its newer Gaussian/book evidence receives a separate static
follow-up. The qualification retains its original source/image attribution.

The [follow-up static review](books/review.json) now verifies actual pushed
`6784112`: 136 registered Psionic programs, 483 source listings and nine matching
capability bindings. Its [post-rebase proof](books/post-rebase-proof.json)
confirms the incoming Gaussian evidence and the preserved A1/0.46 book prose.
No programs, models or native gates were executed by this static review.

## Follow-up compiler (#12611)

This qualification binds compiler fingerprint `7c749e9b`. That compiler stops
the myrealm, realm, Fairy and Everglade pages with E0381, and it accepts a local
array returned through a helper that slices its view parameter's pointee
(`ft(field(a[..]))`). The follow-up in `coax/lib/compiler/checker.coax` reads a
call's argument for its storage when the formal's place is an array, and
collapses static origins that alone overflow a set. It changes the compiler, so
the record above doesn't cover it.

Follow-up compiler, rebased onto `ax-library-check-cache` (33dceac432):
fingerprint `9cedc78bc1512694ef3d2720332b76a6b728bafaa50686aff816497828c97970`,
binary `0dfbbc17347d78314a53022b79280752e0671f46061a3d78e4bb0e6fcf61c4d1`,
built by a full `coax/build.sh` bootstrap (29 min, every gate; the stage0
`checker.coax` mutation gate took 1,371 s with no survivors). The checks below
ran on the same change before that rebase (fingerprint `1c12c8f8`, 23-minute
bootstrap, checker mutation 1,134 s). On the rebased compiler, the 44 source
fixtures pass and the four pages check again.
Checks with that compiler:

- `borrow_summaries.py`: 44 source fixtures pass, the original 39 and five new
  ones: the issue's six lines, its string form and a long table of literals
  compile, and a local array returned through the helper chain (slice and
  string forms) is E0383.
- `borrow_query_work.py` (1,000 independent returns), `borrow_summary_native.py`
  and `borrow_summary_systemone.py` pass.
- Two new `checker.coax` checks, which fail on the `7c749e9b` checker source:
  storage behind a passed-on helper slice, and static-origin overflow.
- `coax/seed-portability.sh --linux`: every seed matches Clang and Debian GCC.
- `tests/provenance.py` passes. `coax/tests/run.sh` passes except
  `ct_strings.py`, which fails the same way with the `7c749e9b` compiler: its
  `make` returns a string viewing a local array, which is E0383.
- The four pages check with their declared step budgets: myrealm 152 of
  152, realm 86 of 86, Fairy 163 of 163, and Everglade 342 of 438 run with none
  failing (the run was stopped at the owner's wrap-up). Of 216 entry and
  module files, 28 that 0.46.0 stopped with E0381 now check. `chat.coax` still
  stops: its `PkCatalog` return has 122 string leaves, over the analysis's
  64-leaf bound, which `reject-borrow-leaves-65` keeps as E0381 by design.
