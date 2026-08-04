> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 1 — planning.

NIP-WR
======

Work Relations
--------------

`draft` `optional`

This NIP defines typed edges between Work objects: the dependency,
hierarchy, duplication, and lineage structure that turns isolated Work into
a graph clients can plan, block, and audit against.

A relation is a fact about two Work objects. It never silently mutates
either endpoint: making Work a `child` does not change its owner, state, or
authority, and marking Work `blocked_by` does not stop its sessions —
policy may react to the edge, but the edge itself is only structure.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32210 | Addressable (unique `d`) | Work Relation |
| 32211-32214 | — | Reserved for future NIP-WR use |

## 1. Work Relation (`kind:32210`)

Authority-signed, one record per directed edge. Address:

```text
32210:<authority_pubkey>:<relation_ref>
```

### 1.1 Required tags

- `d`: unique `relation_ref`
- `org`: owning Organization
- `a` with marker `source`: the source Work Record address
- `a` with marker `target`: the target Work Record address
- `rel`: the relation kind (see 1.3)
- `state`: `active` or `removed`
- `p` with marker `actor`: who requested the edge
- `published_at`

### 1.2 Recommended tags

- `e` with markers `intent` / `admission`: the NIP-WI provenance
- `confidence`: `certain`, `likely`, or `speculative` — required for the
  lineage kinds in 1.4
- `reason`: bounded machine-readable reason code

### 1.3 Relation kinds

| `rel` | Meaning (source → target) | Inverse presentation |
| --- | --- | --- |
| `parent` | source is the parent of target | target shows `child` |
| `blocks` | source blocks target | target shows `blocked_by` |
| `duplicate` | source duplicates target; target is canonical | target shows `duplicated_by` |
| `related` | symmetric association | same |
| `supersedes` | source replaces target | target shows `superseded_by` |
| `split_from` | source was split out of target | target shows `split_into` |
| `merged_into` | source was merged into target | target shows `merged_from` |

The authority publishes exactly one record per logical edge, in the
canonical direction above. Clients derive the inverse for display; a second
record encoding the inverse of an existing edge is redundant and MUST be
ignored in favor of the canonical one.

### 1.4 Prior-work lineage

Two additional kinds carry the forensic prior-work model, connecting
defect and finding Work across revisions:

| `rel` | Meaning |
| --- | --- |
| `occurrence_of` | source Work is an occurrence of the target root-cause Work |
| `same_cause` | source and target share a root-cause identity |

Lineage edges MUST carry `confidence`. The authority does not invent
continuity: a rename, move, or revision change creates a new occurrence,
and only an explicit edge relates it to prior work.

### 1.5 Graph rules

- **Hierarchy is acyclic.** The authority MUST refuse a `parent` edge that
  would create a cycle, and refuse a second active `parent` for the same
  child.
- **Duplicates converge.** `duplicate` targets the surviving canonical
  Work. Chains (`A duplicate B`, `B duplicate C`) SHOULD be flattened at
  admission to point at the terminal survivor.
- **Removal is a record.** Removing an edge publishes the same `d` with
  `state=removed`; the edge's existence remains auditable.
- **Blocking is advisory structure.** Whether `blocks` gates a state
  transition or dispatch is Work policy, evaluated at NIP-WI admission —
  never a client-side inference from the edge alone.

### 1.6 Example

```json
{
  "kind": 32210,
  "pubkey": "<authority-pubkey>",
  "content": "",
  "tags": [
    ["d", "rel-4b91"],
    ["org", "org-openagents"],
    ["a", "32170:<authority-pubkey>:work-9f31854f", "", "source"],
    ["a", "32170:<authority-pubkey>:work-2277ab", "", "target"],
    ["rel", "blocks"],
    ["state", "active"],
    ["p", "<actor-pubkey>", "", "actor"],
    ["e", "<intent-event-id>", "", "intent"],
    ["published_at", "1786500000"]
  ]
}
```

## Security considerations

- **Graph poisoning.** Relations shape triage, dispatch, and dashboards.
  Only authority-signed edges are canonical; a participant-signed edge
  proposal is a NIP-WI intent, not an edge.
- **Cross-organization edges.** An edge whose target lives under another
  authority is a reference, not a claim about the foreign Work; clients
  MUST NOT merge state across authorities because an edge names them both.
- **Information leakage.** An edge between a public and a private Work
  reveals the private Work's existence. Authorities SHOULD refuse such
  edges on public relays or publish them only to the restricted audience.

## References

- NIP-01
- NIP-WK, NIP-WI (layer 0)
- NIP-PI (this layer) — relations rendered on Issue surfaces
- `docs/omega/2026-08-03-forensic-prior-work-authority.md`

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: directed relation records, canonical-direction
  rule, lineage kinds with confidence, and graph admission rules.
