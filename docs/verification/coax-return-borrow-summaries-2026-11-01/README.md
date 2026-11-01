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
