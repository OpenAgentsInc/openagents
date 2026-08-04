> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 0 — foundation.

NIP-OT
======

Organizations and Teams
-----------------------

`draft` `optional`

This NIP defines the administrative scope of the All Work system:
**Organization** and **Team** records, **Membership** projections, **Role**
definitions, and **Workroom Bindings** that connect an Organization's scopes
to NIP-29 relay groups.

It is also the trust bootstrap for the whole program: the Organization
record is where clients learn which key is the All Work authority whose
signatures make NIP-WK, NIP-WI, and workroom projections canonical.

Two boundaries govern everything here:

- **Membership is visibility, not capability.** A membership row, role
  label, or roster entry never grants command authority. Effective
  authority comes from policy plus explicit grants (NIP-WI, NIP-AD).
- **An Organization is not a Workspace and not the Work.** It is the
  administrative scope for people, agents, Teams, policy, and Work.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32100 | Addressable | Organization Record |
| 32101 | Addressable | Team Record |
| 32102 | Addressable | Membership Projection |
| 32103 | Addressable | Role Definition |
| 32104 | Addressable | Workroom Binding |
| 32105-32109 | — | Reserved for future NIP-OT use |

All five kinds are authority-signed projections: the Organization's All Work
authority publishes them from its membership and policy state, the same way
Immortal publishes relay-signed snapshots in Block NIP-DV and NIP-IA. The
one bootstrap exception is described in 1.3.

## 1. Organization Record (`kind:32100`)

The root administrative record. Address:

```text
32100:<authority_pubkey>:<organization_ref>
```

### 1.1 Required tags

- `d`: stable `organization_ref`
- `name`: bounded display name
- `p` with marker `authority`: the All Work authority pubkey for this
  Organization (normally the record's own signer)
- `revision`: monotonic record revision
- `published_at`

### 1.2 Recommended tags

- `p` with marker `founder`: the founding owner principal
- `relay`: repeated relay hints for the Organization's event traffic
  (complementing NIP-65)
- `a` with marker `team`: each Team Record
- `a` with marker `workroom`: the default Workroom Binding
- `policy`: member/workflow/guidance policy refs
- `t`: discovery topics

### 1.3 Trust bootstrap and rotation

Clients learn an Organization's address out of band — an invitation, a
profile link, a directory — and pin `<authority_pubkey>` from that address.
From then on, only that key's signatures make Work, admissions, membership,
and workroom projections canonical for the Organization.

Key rotation publishes a new Organization Record revision that carries both
`["p", "<new-key>", "", "authority"]` and
`["p", "<old-key>", "", "retired_authority"]`, signed by the old key, plus a
matching record signed by the new key. Clients accept the rotation only when
both halves exist. A record from an unknown key claiming an existing
`organization_ref` is a different deployment, not an update.

### 1.4 Example

```json
{
  "kind": 32100,
  "pubkey": "<authority-pubkey>",
  "content": "",
  "tags": [
    ["d", "org-openagents"],
    ["name", "OpenAgents"],
    ["p", "<authority-pubkey>", "", "authority"],
    ["p", "<owner-pubkey>", "", "founder"],
    ["relay", "wss://relay.openagents.com"],
    ["a", "32101:<authority-pubkey>:team-core", "", "team"],
    ["a", "32104:<authority-pubkey>:room-hq", "", "workroom"],
    ["revision", "4"],
    ["published_at", "1786500000"]
  ]
}
```

## 2. Team Record (`kind:32101`)

A stable group with a shared Work scope and workflow policy. Address:

```text
32101:<authority_pubkey>:<team_ref>
```

### 2.1 Required tags

- `d`: stable `team_ref`
- `org`: owning `organization_ref`
- `name`: bounded display name
- `revision`, `published_at`

### 2.2 Recommended tags

- `a` with marker `parent`: parent Team for sub-teams
- `a` with marker `workroom`: the Team's Workroom Binding
- `policy` with marker `workflow`: the Team's Workflow State configuration
  (future NIP-WS)
- `key`: short identifier prefix for the Team's Issue identifiers
  (future NIP-PI)
- `t`: discovery topics

## 3. Membership Projection (`kind:32102`)

One row binding a principal to an Organization or Team scope. Address:

```text
32102:<authority_pubkey>:<membership_ref>
```

### 3.1 Required tags

- `d`: stable `membership_ref`
- `org`: owning Organization; plus `team` when Team-scoped
- `p` with marker `member`: the principal
- `kind_label`: `person`, `agent`, `service`, or `device`
- `state`: `verified`, `stale`, or `revoked`
- `generation`: the account/scope generation this row is valid for
- `observed_at`: when the authority last verified the row

### 3.2 Recommended tags

- `role`: one or more Role Definition refs
- `a` with marker `attestation`: for `kind_label=agent`, the Block NIP-OA
  owner attestation (or NIP-SA profile) binding the agent key to its owner
- `p` with marker `owner`: the agent's owner principal, for agent members

### 3.3 Rules

- A generation change fences every older row: clients MUST NOT reuse a
  membership whose `generation` predates the current scope generation.
- `revoked` rows remain published (tombstone semantics) so clients can
  distinguish "removed" from "never present".
- Agent members require an owner attestation ref; a bare agent key with no
  owner relation is displayable but MUST be marked unattested.
- Displaying a membership row grants nothing. Every command still passes
  NIP-WI admission against policy and grants.

## 4. Role Definition (`kind:32103`)

A named bundle of expected responsibilities or presentation labels.
Address: `32103:<authority_pubkey>:<role_ref>`.

Required tags: `d`, `org`, `name`, `revision`. Recommended: bounded
`content` description and `t` topics.

A Role is display and expectation vocabulary — `lead`, `reviewer`,
`operator`, `triage` — that policies MAY reference. The Role itself grants
no capability, and clients MUST NOT render controls merely because a role
label suggests them.

## 5. Workroom Binding (`kind:32104`)

Binds an Organization, Team, or Work scope to a relay-qualified NIP-29
group, so collaboration has a stable, auditable home. Address:

```text
32104:<authority_pubkey>:<binding_ref>
```

### 5.1 Required tags

- `d`: stable `binding_ref`
- `org`: owning Organization
- `scope`: `organization`, `team`, `project`, or `work`, with the scoped
  ref in a matching `team`/`work`/`a` tag
- `group`: the relay-qualified group coordinate as `<host>'<group-id>`
  (NIP-29 form)
- `relay`: the group's relay URL
- `audience`: `workroom`, `private`, or `owner_only`
- `revision`, `published_at`

### 5.2 Rules

- The binding is projection wiring: NIP-WA workroom activity
  (kinds 32150-32163) and conversational traffic for the scope flow to the
  bound group. The relay's own membership and moderation stay relay-scoped
  and never become OpenAgents authority.
- NIP-29 `private` is relay access policy, not end-to-end encryption. An
  `audience` of `private` or `owner_only` additionally requires the
  encryption and relay-set rules of the signed-workroom profile; the
  binding MUST NOT claim confidentiality the transport does not provide.
- Rebinding a scope to a new group is a revision with the old group kept as
  an `a`/`group` ref marked `previous`, so history remains discoverable.

## Composition

- **Agents.** Agent members authenticate to relays through Block NIP-AA
  using NIP-OA attestations from their owner; their personas are Block
  NIP-AP; their sovereign-agent lifecycle is NIP-SA. This NIP only projects
  their membership.
- **People.** Human profiles are ordinary kind-0 metadata; this NIP binds
  their keys to scopes.
- **Work.** NIP-WK Work Records carry `org` refs; NIP-WI intents name the
  Organization and are admitted against its policy; NIP-EV dispositions are
  checked against the owner principal recorded here.

## Security considerations

- **Organization spoofing.** Anyone can publish a kind-32100 event with a
  familiar name. Identity is the full address including the authority
  pubkey; clients pin it out of band and treat same-`d` records from other
  keys as unrelated.
- **Membership as attack surface.** A compromised authority key can mint
  membership rows. Rows are therefore projections to display, never inputs
  that bypass admission — the blast radius of a forged row is visibility,
  not capability.
- **Stale rosters.** `observed_at` plus `state` make staleness explicit.
  Clients MUST NOT present a `stale` row as current membership.
- **Relay-group drift.** The NIP-29 group's member list can diverge from
  Membership Projections. The projection is the Organization's statement;
  the group list is the relay's. Displays that merge them MUST label the
  source of each row.

## References

- NIP-01, NIP-29, NIP-65
- Block NIP-OA, NIP-AA, NIP-AP (agent identity and admission)
- NIP-WK, NIP-WI, NIP-EV (this program)
- `docs/omega/2026-08-03-organization-membership-authority.md`
- `docs/omega/2026-08-03-signed-workroom-projection.md`

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Organization, Team, Membership, Role, Workroom
  Binding, trust bootstrap, rotation, and generation-fencing rules.
