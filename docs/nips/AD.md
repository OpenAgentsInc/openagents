> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 2 — agents and execution.

NIP-AD
======

Assignment and Delegation
-------------------------

`draft` `optional`

This NIP defines the structural split at the heart of agent-era work
tracking: the **Assignee** — the human who remains accountable — and the
**Agent Delegate** — the agent doing the current work under a bounded,
revocable **Delegation Grant**.

The rule the whole layer stands on:

> An agent is a first-class participant, but it is never the accountable
> human by implication. Assignment does not grant execution; delegation
> does not transfer accountability; and effective authority comes from the
> grant, not the field value.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32270 | Addressable | Assignment Record |
| 32271 | Addressable | Delegation Grant |
| 32272-32279 | — | Reserved for future NIP-AD use |

Both are authority-signed projections of the admission state, changed only
through NIP-WI intents (`work.assign`, `work.delegate`).

## 1. Assignment Record (`kind:32270`)

The accountable-owner binding for one Work object. Address:

```text
32270:<authority_pubkey>:<work_ref>
```

### 1.1 Required tags

- `d`: the `work_ref`
- `org`
- `p` with marker `assignee`: the accountable person, or an explicitly
  admitted organizational role principal
- `state`: `active` or `cleared`
- `revision`, `published_at`

### 1.2 Recommended tags

- `p` with marker `previous`: the prior assignee on reassignment
- `e` with markers `intent` / `admission`: NIP-WI provenance
- `role`: the NIP-OT Role under which an organizational principal is
  admitted as assignee

### 1.3 Rules

- Exactly one active Assignment exists per Work. Reassignment replaces the
  record; history lives in the Work Event stream (`assigned` events).
- The assignee principal MUST be a `person` (or explicitly admitted role)
  in NIP-OT membership. An agent key is refused at admission.
- Assignment is accountability, not capability: the assignee still acts
  through NIP-WI like everyone else. What assignment does grant is
  standing — the assignee is a valid decider for NIP-EV Owner
  Dispositions on this Work.

## 2. Delegation Grant (`kind:32271`)

A revocable, scoped authorization for one Agent Member to perform bounded
Work. Address:

```text
32271:<authority_pubkey>:<grant_ref>
```

### 2.1 Required tags

- `d`: stable `grant_ref`
- `org`
- `work`: the target Work (a grant is Work-scoped; broad standing grants
  are out of scope for this NIP)
- `p` with marker `delegate`: the Agent Member key
- `p` with marker `issuer`: the granting principal
- `generation`: monotonic grant generation
- `state`: `active`, `revoked`, `superseded`, or `expired`
- `exp`: expiry timestamp
- `published_at`

### 2.2 Recommended tags

- `capability`: repeated allowed action classes (mirroring NIP-WI
  operations and tool classes)
- `tool_policy`: tool policy ref
- `host`: allowed Host/placement refs (future NIP-HP)
- `budget`: budget policy ref with bounds
- `privacy`: privacy class the delegate's output must satisfy
- `evidence`: evidence policy ref (what the delegate must produce)
- `a` with markers `claim` / `lease`: the conditional Repository Work
  Claim and Lease pair (NIP-RC), present together or not at all
- `a` with marker `attestation`: the Block NIP-OA owner attestation for
  the delegate key
- `a` with marker `skill`: NIP-SKL manifests the delegate may load,
  version-pinned

### 2.3 Grant laws

1. **Explicit and bounded.** A grant names its issuer, capabilities,
   expiry, and generation. Anything not named is denied; prose anywhere
   (Issue bodies, Documents, chat) cannot widen it.
2. **Revocation fences by generation.** Publishing the grant with
   `state=revoked` invalidates the generation: Sessions under it move to
   `revoked`, and any later NIP-WI intent carrying the old `generation`
   is refused `stale_generation`. Replacement issues a new grant with a
   higher generation.
3. **One active delegate per Work.** A new grant supersedes the previous
   (`state=superseded`, `a` ref marked `successor`). Fan-out to multiple
   agents uses Child Work, each with its own grant.
4. **Claims travel with grants where repositories do.** For
   repository-domain Work, the grant carries its NIP-RC claim/lease pair
   so collision safety and delegation stay bound; both fields are
   nullable together because All Work includes non-repository domains.
5. **The delegate display is downstream.** NIP-PI shows `delegate` from
   this record; deleting the projection changes nothing. The grant is
   the authority record.

### 2.4 Example

```json
{
  "kind": 32271,
  "pubkey": "<authority-pubkey>",
  "content": "",
  "tags": [
    ["d", "grant-a41c"],
    ["org", "org-openagents"],
    ["work", "work-9f31854f"],
    ["p", "<agent-pubkey>", "", "delegate"],
    ["p", "<owner-pubkey>", "", "issuer"],
    ["generation", "3"],
    ["state", "active"],
    ["capability", "work.session.control"],
    ["capability", "work.evidence.attach"],
    ["tool_policy", "policy-tools-std"],
    ["budget", "budget-50k-tokens"],
    ["privacy", "workroom"],
    ["a", "32301:<authority-pubkey>:claim-77e0", "", "claim"],
    ["a", "32300:<authority-pubkey>:packet-77e0", "", "lease"],
    ["a", "33400:<skill-pubkey>:code-review", "", "skill"],
    ["exp", "1786586400"],
    ["published_at", "1786500000"]
  ]
}
```

## Composition

- **NIP-SA sub-delegation.** An agent delegating further uses NIP-SA
  `kind:39260` under its own grant's scope; the sub-delegation's
  `max_amount` and capabilities MUST fit inside this grant.
- **NIP-AC funding.** A grant MAY reference an AC envelope for spendful
  work; budget and envelope caps are enforced independently.
- **NIP-AS sessions.** Every Agent Session names the grant and generation
  it runs under; NIP-AV activities inherit that binding.

## Security considerations

- **Grant display is not grant state.** Clients verify `state`,
  `generation`, and `exp` at use time; a cached `active` record is stale
  the moment a higher-generation record exists.
- **Issuer standing.** Admission verifies the issuer may delegate this
  Work (owner, assignee, or policy-admitted role). A grant from a
  standing-less issuer never becomes canonical.
- **Delegate identity.** The delegate key's owner relation (NIP-OA /
  NIP-SA) is checked at admission; an unattested agent key can be
  displayed but not granted.
- **Accountability laundering.** No composition of grants makes an agent
  the assignee, a decider for dispositions, or a release authority.

## References

- NIP-01
- NIP-WK, NIP-WI, NIP-EV, NIP-OT (layer 0)
- NIP-AS, NIP-AV, NIP-RC (this layer)
- NIP-SA `kind:39260`, NIP-AC, NIP-SKL
- Block NIP-OA — owner attestation
- `docs/omega/2026-08-03-work-command-admission-authority.md`

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Assignment Record, Delegation Grant, grant
  laws, generation fencing, and the accountability boundary.
