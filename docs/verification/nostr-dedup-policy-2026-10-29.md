# Nostr duplicate representation policy: partial evidence

This is a checked policy primitive for choosing between **one** stored original
and **one** candidate. It is not a verified event store or an admission API.
`coax/lib/nostr/store_dedup_policy.coax` chooses the lexicographically lowest
exact UTF-8 byte representation and, for a winning replacement, accounts for
its change in payload bytes before returning a decision. A losing duplicate
requires no additional record or payload bytes. Refusal writes no store state:
the function returns a value and owns no storage.

The caller must establish from the immutable originals that their canonical
IDs and signatures are valid and their IDs match. An `ok` result does not prove
any of these properties. Valid disjoint buffers, lifetime, exclusive access,
correct old quota accounting, and atomic publication belong to the future store
owner. A caller-supplied ID or admission flag is not proof. The code is not yet
connected to a store, and no deletion, tombstone, snapshot recovery, rollback
protection or durable-commit guarantee follows from it.

**Multiple pre-existing copies are not covered.** A store may already have
more than one original with the same verified ID. Integration must verify and
account for *all* such records, reduce to the lowest original, correctly
reclaim their count and bytes, preserve deletion/tombstone behavior, and stage
and publish the complete new state atomically. Comparing only the newly
received event with one selected old representation does not establish these
properties.

## Evidence and remaining gates

- `coax test coax/lib/nostr/store_dedup_policy.coax`: 18 checks passed with the
  default evaluation budget. Checks include byte-order prefixes and UTF-8,
  malformed accounting, both exact original-size boundaries, full quotas,
  shrinking, and a failed growing replacement.
- `coax mutate coax/lib/nostr/store_dedup_policy.coax`: 239 mutants, 239 killed,
  zero survived. This covers the loaded policy/snapshot/layout test graph, not
  verification or owner admission.
- Independent read-only review found no concrete ordering or arithmetic defect
  under the one-old-original, equal-verified-ID and immutable-input assumptions;
  it identified the multiple-old-original integration requirement above.
- In a separate probe, two actual successful `ns_admit_one` calls in one check
  exceed the unchanged 20,000,000-step budget (E0400). One valid admission in
  the existing test uses 18,688,237 steps. An empty or unverified positive test
  does not establish that the real duplicate path meets this budget.

Task #f539 remains open. It still needs real verified duplicate admission,
independently signed reordered and escaped/Unicode originals, native and Wasm
checked/release and sanitizer evidence, full authored mutation, and integration
with the actual owner including quotas, deletion and tombstones. No check limit
was raised for this slice.
