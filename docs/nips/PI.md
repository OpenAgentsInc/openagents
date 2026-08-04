> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 1 — planning.

NIP-PI
======

Project Issue
-------------

`draft` `optional`

This NIP defines the **Issue projection**: the concrete planning and
tracking view of NIP-WK Work that Linear-class lists, boards, cycles,
triage, and search render.

The single most important rule:

> An Issue is a projection of Work, not a second record. It shares the
> Work's identity, revision, and Event history. There is no synchronization
> job between two writable lifecycles.

Everything an Issue displays — state, assignee, delegate, labels, planning
placement — is changed by submitting a NIP-WI Work Intent against the
underlying Work. The Issue record exists so that any Nostr client can render
a complete tracker view from relay data alone, without knowing the rest of
the Work object.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32200 | Addressable | Issue Projection |
| 32201-32209 | — | Reserved for future NIP-PI use |

## 1. Issue Projection (`kind:32200`)

Authority-signed, addressed by the same `work_ref` as its Work Record:

```text
32200:<authority_pubkey>:<work_ref>
```

### 1.1 Required tags

- `d`: the `work_ref` (identical to the NIP-WK Work Record `d`)
- `org` and `team`: owning Organization and Team refs
- `identifier`: the human identifier, `<TEAM-KEY>-<number>` (see 1.4)
- `title`: bounded title (public-safe, or omitted with an encrypted
  `content` for private Work)
- `state`: the NIP-WS Workflow State ref
- `revision`: the shared Work revision this projection reflects
- `published_at`

### 1.2 Recommended tags

- `priority`: `urgent`, `high`, `medium`, `low`, or `none`
- `estimate`: numeric estimate under the Team's estimation scheme
- `due`: due-date timestamp (planning data, never authority)
- `p` with marker `assignee`: the accountable human (display of the
  NIP-AD assignment; the grant record is authoritative)
- `p` with marker `delegate`: the current Agent Delegate
- `p` with marker `subscriber`: repeated notification subscribers
- `label`: repeated NIP-WS Label refs
- `a` with marker `parent` / `child`: parent and Child Work Issues
- `a` with marker `relation`: NIP-WR Work Relations
- `a` with marker `project` / `cycle` / `milestone` / `release`: planning
  placement refs (NIP-PG, NIP-RP)
- `a` with marker `document` / `need`: NIP-DD Documents and NIP-CN
  Customer Needs
- `a` with marker `session`: active Agent Session refs
- `e` with marker `head`: the latest NIP-WK Work Event
- `sla`: NIP-WS SLA Policy ref, when one applies

`content` is empty, a bounded public-safe description, or a NIP-44 payload
for the Work audience. Description text is data: requirements, acceptance,
grants, and verification live in their typed records, and prose cannot
widen any of them.

### 1.3 Projection discipline

- The `revision` tag MUST equal the Work Record revision the projection was
  generated from. A client holding both records with different revisions
  treats the lower one as stale.
- Every Issue mutation is a NIP-WI intent (`work.update`,
  `work.state.set`, `work.assign`, `work.relate`, …) admitted against the
  Work. The authority republishes the Issue Projection after admission.
- Archiving or deleting an Issue is a Work transition; the projection
  reflects it (`state` moves, or the record gains `["archived_at", ...]`).
  A relay-level NIP-09 deletion of the projection does not change the Work.

### 1.4 Identifiers

The authority assigns `identifier` at `work.create` admission from the
Team's `key` (NIP-OT Team Record) and a per-Team counter:
`CORE-142`. Identifiers are unique within an Organization, stable across
Team renames, and preserved on Team moves by recording the prior identifier
in an `["identifier_alias", "<old>"]` tag so old references keep resolving.

### 1.5 Example

```json
{
  "kind": 32200,
  "pubkey": "<authority-pubkey>",
  "content": "",
  "tags": [
    ["d", "work-9f31854f"],
    ["org", "org-openagents"],
    ["team", "team-core"],
    ["identifier", "CORE-142"],
    ["title", "Add lane READMEs to the NIP source directories"],
    ["state", "32215:<authority-pubkey>:team-core:started"],
    ["priority", "high"],
    ["revision", "7"],
    ["p", "<owner-pubkey>", "", "assignee"],
    ["p", "<agent-pubkey>", "", "delegate"],
    ["label", "32216:<authority-pubkey>:docs"],
    ["a", "32222:<authority-pubkey>:proj-nip-program", "", "project"],
    ["e", "<latest-32171-event-id>", "", "head"],
    ["published_at", "1786500000"]
  ]
}
```

## 2. Rendering contract

A tracker client needs only standard filters:

- board/list: `{"kinds":[32200],"authors":["<authority>"],"#team":["team-core"]}`
  grouped by `state`;
- one Issue: the address `32200:<authority>:<work_ref>`, plus its Work
  Event stream via `{"kinds":[32171],"#work":["<work_ref>"]}`;
- my work: filter on `#p` with client-side marker inspection;
- cycle/project views: filter on the planning ref tags.

Counts, groupings, and orderings computed from projections are display
facts. They inherit each projection's `revision` freshness and MUST NOT be
presented as canonical totals when any contributing record is stale or
missing.

## Security considerations

- **Dual-write temptation.** The failure mode this NIP exists to prevent:
  a client or bridge writing Issue state directly. Clients MUST treat a
  kind-32200 event whose `revision` has no corresponding Work Event chain
  as suspect and prefer the Work Record.
- **Identifier spoofing.** Identifiers are display strings assigned by the
  authority; a familiar-looking identifier from another authority is a
  different object.
- **Privacy.** Titles, descriptions, estimates, and due dates can leak
  intent. Private Work publishes opaque projections (refs and state only,
  encrypted content) to restricted relays.

## References

- NIP-01, NIP-09, NIP-44
- NIP-WK, NIP-WI, NIP-OT (layer 0)
- NIP-WR, NIP-WS, NIP-PG, NIP-RP, NIP-DD, NIP-CN (this layer)
- NIP-AD (layer 2) — assignment and delegation authority

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: the Issue Projection record, projection
  discipline, identifier grammar, and rendering contract.
