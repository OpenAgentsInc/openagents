# Larger secret-source inventory: partial profile

**Update (2026-10-30):** the profile is now `large-aggregate-source-v2`. It
admits a secret checked cast only for unsigned strict widening; see
[the contained-cast record](2026-10-30-secret-audit-contained-cast.md).
Reports naming `large-aggregate-source-v1` describe the earlier rule.

**Security correction:** this is historical evidence for the original
snapshot, which accepted a secret-dependent checked narrowing cast even
though that cast can trap. Its prior mutation and source reviews did not
establish secret-control-flow safety for casts. All three aggregate profiles
were subsequently changed to refuse secret-origin checked casts
conservatively, including widening casts; truncating casts and public inputs
retain their prior treatment. Historical conditional decisions bound to the
old tools must not be reused as current approvals. The signer refusal below
was a refusal, not an acceptance.

The experimental `tools/coax-nostr/large-source-audit/` profile admits up to
32 ordered physical source units, each at most 65,536 bytes, with an unchanged
262,144-byte aggregate source budget. Its graph-shape boundary admits at most
65,536 parsed nodes and 8,192 resolved-type rows. It verifies ordered unique
source paths and numeric node references and unit roots; shape alone does not
prove node semantics, type-row completeness or source identity. The checked
driver binds the compiler's inventory to actual source digests, tool inputs,
loaded audit artifact and explicit classification, then the inherited v2
dataflow policy analyzes reached procedures. The classification schema is
separate (`coax.secret-large-classification-views.v3`); live, stable, aligned,
disjoint views and their classifications are caller premises.

This is a **partial source-analysis profile**, not completion of #4828. The
previous 16-unit profile and its historical refusals were unchanged at the
time of this snapshot. The
new profile still retains the old 8-level type bound, fixed view extents of
1–1,024, 4,096 public interval bound, 64 reached procedures, 16 call depth,
and existing visit and guard budgets. Unsupported constructs and exhausted
budgets refuse. The exact copied dataflow/bridge changes are checked against
the pinned profile by `test_correspondence.py`; after the security correction
that check pins the corrected v2 profile, not the historical one. It is a correspondence
check, not a new proof of the inherited evaluator.

On compiler `coax0 0.22.0` SHA-256
`f335e7ec7b3f6d5966244a09aa8002b11b5ba607a1f673353c51effd9dab84a1`,
the profile passed 198 Coax checks and a full reference mutation run: 2,935
of 2,935 mutants killed, 162,449 checks and none skipped. The separate
inventory passed 43 checks and 832 of 832 mutants, 10,967 full checks and
none skipped. The compiler's aggregate startup allowance was 250,000,000;
the per-check limit was unchanged. Seven inherited public-view test groups
and two additional closure groups passed, including a secret branch in the
last of 27 units, old-profile refusal, 32/33-unit boundaries and a changed
last source after compilation. An independent review found no concrete new
false-accept path in the inspected unit, source-binding and shape wiring.
It did not rerun tests or independently prove the inherited evaluator.

The [evidence record](2026-10-29-large-source-audit-partial.json) pins the
source, compiler, test logs and a 27-unit signer snapshot. For that
uncommitted snapshot the original profile refuses with `limit`; the new
unclassified profile refuses with `unsupported`, and the classified 32-byte
case refuses with `type`. None is a signer safety acceptance. The general
signer's empty and up-to-1-MiB messages do not fit the current fixed-extent
view model. Deeper types, effectful calls, public-length loops, relevant
secret-derived guards and full source-bound revalidation remain open. The
experimental depth and void-call probes are not included in this profile.

The driver relies on a trusted compiler, parser, native bridge and stable
source/tool files. It does not establish physical ownership, secure erasure,
hidden compiler copies, object or JIT timing, or production-secret suitability.
