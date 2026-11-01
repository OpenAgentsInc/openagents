# Supplied retained-store policy (B0)

[retain-policy.coax](policy/retain-policy.coax) classifies one descriptor-leaf
store from supplied checked facts. It is a standalone, unloaded Coax policy:
it does not traverse source, resolve types, infer aliases, issue a compiler
diagnostic or prove host lifetime safety. Its origin/path carriers come from
[borrow-summary.coax](lib/compiler/borrow-summary.coax). Compiler return
summaries remain the separate [A1 profile](verification-return-borrows.md).

## Supplied values and addressed storage

A source `RP_Term` names descriptor backing, not the aggregate holding the
descriptor. An `RP_Address` names storage addressed by a write, not the old
value stored there. `value_type` identifies the checked descriptor leaf;
`destination.storage.origin.type` identifies addressed storage and may differ.
These opaque keys are caller facts, not a second type resolver.

Static terms use frame 0. Every nonstatic term belongs to the supplied current
frame, 1..INT32MAX. IDs establish no frame ancestry or nested lexical ordering.
Parameter INLINE addresses denote callee-owned by-value storage. Parameter
DESCRIPTOR addresses require an active DEREFERENCE somewhere in the path,
including after FIELD steps; descriptor sources need not contain it. View
indexing uses wildcard DEREFERENCE, distinct from owned-array ELEMENT.
BS validates shape and canonical inactive path fields.

## Completeness, violations and pending relations

Check `status` first. On `ok`, `facts_complete` equals
`source_complete && destination_complete`. Unknown alternatives never erase
known pairs. Independently, `relation_complete` describes whether every known
pair's lifetime relation was decided:

| Known source and addressed storage | Relation |
| --- | --- |
| Static backing into any admitted address | Decided clear |
| Parameter descriptor backing into current local/inline storage | Decided clear |
| Current LOCAL/INLINE backing into external parameter/static storage | Decided forbidden |
| Current LOCAL/INLINE backing into local/inline storage | Manual; relation incomplete |
| Parameter descriptor backing into external parameter/static storage | Pending outlives obligation; relation incomplete |

Even identical formal ordinals or full symbolic origins do not discharge a
pending external-store relation. Zero known pairs have a vacuously complete
relation. Known forbidden, manual and pending alternatives coexist; the first
forbidden witness uses source-major input order. Thus forbidden plus unknown
may have `forbidden=true`, `relation_complete=true`, `facts_complete=false`.

Only `status==ok && facts_complete && relation_complete && !forbidden &&
obligation_count==0` discharges this supplied frame-level question. It grants
no allocation/reclamation guarantee or permission for foreign retention.
A complete-empty source denotes a checked default-empty descriptor, distinct
from an ordinary zero-length view of an owner and from an unknown empty known
set. A complete-empty destination is invalid for an asserted store.

## Bounds and logical publication

Source, destination and output counts are 0..64; paths have at most 8 active
components. At most 128 raw source×destination pairs are admitted, before
deduplication. All supplied metadata is validated before work/capacity refusal,
including facts unused by an empty or unknown alternative. Invalid facts do
not become unknown. Scalar count admission precedes span traversal.

Distinct pending rows sort by BS destination-storage origin, then BS source
origin, preserving frame/value keys. Output is additionally limited by caller
capacity. Invalid/work/capacity returns clear flags/count and witness indices
−1 without changing logical output. Success publishes only its logical prefix;
unused tail fields and outside guards stay unchanged. Native comparisons cover
named fields and all eight path slots, not unspecified C padding. Valid pointers,
extents, disjoint immutable inputs, alignment and lifetime remain caller duties.

## Actual verification and retained failures

The [committed evidence record](../docs/verification/coax-retained-borrow-policy-2026-11-01/record.json)
retains raw receipts, snapshots and the proof archive. Final source SHA-256 is
`ddfdfd79d76d633dfa6c97f2c11af15b61b7bea66525a3a9c1d872b8d213e00a`.

| Observation | Actual scope |
| --- | --- |
| Original and full-matrix semantic REDs | Stub assertions failed E0406 at 25:2 and 34:2 respectively, before implementation. Carrier parsing failures remain setup history. |
| First source `760958eb…c50ed` | Original 38 check claims across B0 and imported BS, plus authority coverage: all 39 evidence claims passed. This historical baseline is retained separately. |
| First root mutation | 128 mutants, 122 killed, 6 survived; retained as a failed attempt. |
| Final root mutation | 128/128 killed, zero survivors; entry-source procedures only, with imported BS baseline checks retained. No equivalence waiver. Transport receipt SHA-256 `1e0c53c72cd5d8eb65e1629dcae9ebf6fd5be17338d754c86b4250300e1d84ec`. |
| Native LP64, O0/O2/ASan+UBSan | Each profile made 1,251 cases/calls with 72 `RP_check` refusals, including all 112 independently authored policy cases and 16 literal bounds cases. Qualification SHA-256 `085b4b49277f1ff2860ccc9993e69430c753c1eb18cdf1b86546b3ffc6f91ea0`. |

Four initial scalar-guard survivors were masked by secondary metadata refusal.
Complete-empty sources and static addresses now independently exercise scalar
1, MAX and MAX+1 admission. Two other survivors arose from cutoffs that could
not distinguish the private insertion helper's reachable results: refusal −1
or a positive count. The caller now explicitly checks the −1 sentinel. The
final source keeps the same refusal semantics; no mutant was waived.

The independent oracle predates implementation review. Its 112+16 input and
expected rows are unchanged in the mechanical C adapter; only padding wording
was narrowed to logical fields and outside guards. Native evidence includes
actual generated-header sizes, alignment/offset assertions, output/input/tail
guards and artifact hashes. LP64 sizes are Term 176, Address 184, Obligation 360
and Result 32 bytes. Emitted C/header are retained; all three binary hashes remain pinned in the receipts.

All observations use the earlier qualified producer image
`5c37377a72fbba0a4e11a6ffdba2bfa37c223f8490b158aa23bc1a75488c53d9`.
Its normal receipt `cfd90bb0…75e2aef8` and fingerprint `7c749e9b…7e956f2d`
belong to the A1 compiler closure. `coax/policy/` is outside that fingerprint
and is not loaded into the compiler. This policy qualification creates no new
accepted-source/current-normal record or compiler seal.

## Remaining integration

Before B1 loads this policy into the compiler, it must move into the existing
compiler fingerprint/bootstrap inventory (for example, `lib/compiler/`) or
that inventory and snapshot closure must explicitly include it. An actually
loaded semantic compiler input must never remain outside its producer
fingerprint. B0 is outside only because it is genuinely unloaded; this is not
a way to reuse a compiler seal after a semantic change.

B1 source traversal and caller substitution, B2 actual local-carrier writes and
returned `SoPost`/`SoResult` composition, and B3 request ownership/lifecycle
remain open. B3 includes actual generated root default initialization and
storage alive through completion/cancellation. B0 alone resolves none of those
obligations. A1's retained caller-state store and lifecycle limitation remains
unchanged.
