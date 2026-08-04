> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 1 — planning.

NIP-WS
======

Workflow States and Labels
--------------------------

`draft` `optional`

This NIP defines the configuration vocabulary that Issue surfaces render:
Team-scoped **Workflow States**, scoped **Labels** with optional hierarchy,
and **SLA Policies**.

All three are policy data. A state named `Done`, a label named `approved`,
or an SLA timer is display and routing vocabulary; the owning Work policy
admits transitions at NIP-WI admission, and no configured name creates
evidence, verification, acceptance, or authority.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32215 | Addressable | Workflow State definition |
| 32216 | Addressable | Label definition |
| 32217 | Addressable | SLA Policy |
| 32218-32219 | — | Reserved for future NIP-WS use |

All are authority-signed configuration records.

## 1. Workflow State (`kind:32215`)

One configured lifecycle state for a Team's Issues. Address:

```text
32215:<authority_pubkey>:<team_ref>:<state_ref>
```

### 1.1 Required tags

- `d`: `<team_ref>:<state_ref>`
- `org` and `team`
- `name`: display name
- `category`: one of `triage`, `backlog`, `unstarted`, `started`,
  `completed`, `canceled`
- `baseline`: the NIP-WK baseline state this maps onto (`draft`,
  `planned`, `active`, `blocked`, `in_review`, `done`, `canceled`,
  `superseded`, `archived`)
- `position`: sort position within its category
- `revision`, `published_at`

### 1.2 Recommended tags

- `color`: display color hint
- `state`: `active` or `retired` — retiring a state keeps the record so
  historical Issues still resolve it; retired states cannot be assigned

### 1.3 Rules

- The `baseline` mapping is what keeps cross-Team and cross-domain views
  coherent: a board can group ten Teams' custom states into the shared
  baseline without guessing from names.
- Which transitions are legal, and who may request them, is Work policy
  evaluated at admission. The definition records what exists, not what is
  permitted.
- Deleting a state in use is refused; migration is an explicit NIP-WI
  batch that moves Issues, then retires the state.

## 2. Label (`kind:32216`)

A scoped classification. Address:

```text
32216:<authority_pubkey>:<label_ref>
```

### 2.1 Required tags

- `d`: stable `label_ref`
- `org`; plus `team` when Team-scoped
- `name`: display name
- `revision`, `published_at`

### 2.2 Recommended tags

- `a` with marker `parent`: parent Label for hierarchies (one level of
  nesting is RECOMMENDED as the interoperable maximum)
- `color`: display color hint
- `state`: `active` or `retired`
- `exclusive`: `"true"` when children of this label are mutually exclusive
  on one Issue (a label group)

### 2.3 Rules

- Labels support discovery, filtering, views, and automation triggers.
  They grant no capability, and automation reacting to a label still
  passes NIP-WI admission.
- A Team-scoped label and an Organization label with the same `name` are
  different labels; clients display scope on collision.

## 3. SLA Policy (`kind:32217`)

Response and resolution expectations for a class of Work. Address:

```text
32217:<authority_pubkey>:<sla_ref>
```

### 3.1 Required tags

- `d`: stable `sla_ref`
- `org`
- `name`
- `revision`, `published_at`

### 3.2 Recommended tags

- `applies`: repeated selectors — a `team`, `label`, `priority`, or
  `domain` value the policy attaches to
- `response`: seconds allowed until first meaningful response
- `resolution`: seconds allowed until resolution
- `clock`: `calendar` or `business` (business calendars are
  deployment-defined and referenced by `policy` ref)
- `escalation`: policy ref describing what breach proposes

### 3.3 Rules

- SLA timers are projections computed from canonical NIP-WK Work Event
  timestamps. A timer display is never itself a breach record; a breach is
  a Work Event (`sla_breached` extension kind) the authority admits, which
  attention systems (future NIP-AT) can subscribe to.
- Breach reaction — escalation, reassignment proposals — flows through
  NIP-WI intents or future NIP-TP triage proposals, never directly from
  the policy record.

## Security considerations

- **Name-based authority confusion.** The recurring failure this NIP
  fences: treating a state or label name as a fact. `Done` is a category;
  the NIP-EV chain is the truth. Clients MUST source completion and
  approval displays from evidence records, not configuration names.
- **Config drift.** Issues referencing retired or missing definitions
  render with an explicit unresolved-config marker, not a guessed
  substitute.
- **Scope leakage.** Team-scoped configuration on public relays reveals
  team structure; private Organizations publish configuration to their
  restricted relay set.

## References

- NIP-01
- NIP-WK, NIP-WI (layer 0)
- NIP-PI (this layer) — the surfaces that render these definitions
- NIP-AT, NIP-TP (layer 3) — attention and triage reactions

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Workflow State with baseline mapping, Label with
  bounded hierarchy and groups, and SLA Policy with projection-only
  timers.
