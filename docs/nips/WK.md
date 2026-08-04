> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 0 — foundation.

NIP-WK
======

Work
----

`draft` `optional`

This NIP defines the root object of the OpenAgents All Work system on Nostr:
the durable **Work** record, its append-only **Work Event** stream, its
versioned **Objective**, and its **Outcome Record**.

One Work object represents one bounded objective and its complete lifecycle.
It can be a repository task, CI job, deployment, incident, operations change,
research question, security assessment, service request, data job, or design
review. Work is deliberately not a chat thread, an agent session, a process,
or a project: those attach to Work, and none of them is the lifecycle root.

The model follows the All Work spine:

> Work is the container. Sessions perform it. Blocks display and control it.
> Hosts run it. Intents request actions. Events record facts. Receipts bind
> facts to evidence.

## Authority model

Work records are **authority-signed projections**. Each deployment has an
All Work authority — a service that admits commands — and that authority
holds its own Nostr keypair. The authority key for an Organization is
declared in its NIP-OT Organization record.

- Kinds `32170`-`32173` are canonical only when signed by the declared
  authority key for the Work's Organization. An event with these kinds from
  any other key is a forgery or an unrelated deployment and MUST NOT be
  merged into the same Work identity.
- Participants never publish Work records directly. They publish NIP-WI Work
  Intents; the authority answers with admission results and then publishes
  the updated Work record and Work Events.
- Relay acceptance is transport evidence only. A relay cannot create,
  mutate, verify, accept, release, or settle Work.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32170 | Addressable | Work Record (current head) |
| 32171 | Addressable (unique `d`) | Work Event (append-only) |
| 32172 | Addressable (unique `d`) | Work Objective revision |
| 32173 | Addressable | Outcome Record |
| 32174-32179 | — | Reserved for future NIP-WK use |

Kind `32171` and `32172` events use a unique `d` per event, following the
`openagents.signed-workroom.v2` precedent: each record is individually
addressable and never replaced, so the stream is append-only while remaining
queryable through standard addressable filters.

## Common tags

- `["d", "<identifier>"]` — record identifier
- `["work", "<work_ref>"]` — the owning Work identifier
- `["org", "<organization_ref>"]` — owning Organization (NIP-OT)
- `["domain", "<work_domain>"]` — Work Domain
- `["class", "<work_class_ref>"]` — optional Work Class ref
- `["state", "<work_state>"]` — current lifecycle state label
- `["revision", "<n>"]` — monotonic Work revision
- `["seq", "<n>"]` — Work Event sequence number
- `["event", "<event_kind>"]` — Work Event kind
- `["p", "<pubkey>", "<relay>", "<marker>"]` — participant with role marker
- `["e", "<event_id>", "<relay>", "<marker>"]` — referenced event
- `["a", "<coordinate>", "<relay>", "<marker>"]` — referenced addressable record
- `["x", "<sha256>"]` — content digest
- `["published_at", "<unix>"]` / `["occurred_at", "<unix>"]` /
  `["admitted_at", "<unix>"]` — timestamps

Parsers MUST NOT treat unknown tags as invalid.

## 1. Work Record (`kind:32170`)

The current head of one Work object. Address:

```text
32170:<authority_pubkey>:<work_ref>
```

### 1.1 Required tags

- `d`: stable `work_ref`
- `org`: owning Organization ref
- `domain`: one of `general`, `development`, `ci`, `deployment`,
  `operations`, `incident`, `research`, `security`, `design_review`,
  `service_delivery`, `data`, or another deployment-declared value
- `state`: current Work State label (see 1.3)
- `revision`: current monotonic revision
- `p` with marker `owner`: the accountable owner principal
- `published_at`: first publication time

### 1.2 Recommended tags

- `title`: bounded public-safe title (omit for private Work)
- `class`: Work Class ref
- `a` with marker `objective`: current `kind:32172` Objective revision
- `a` with marker `issue`: the Issue projection (future NIP-PI)
- `a` with marker `outcome`: the `kind:32173` Outcome Record
- `a` with marker `relation`: typed Work Relations (future NIP-WR)
- `a` with marker `planning`: Project / Cycle / Milestone refs (future NIP-PG)
- `a` with marker `session`: attached Session records (NIP-AS, NIP-SA)
- `a` with marker `workroom`: Workroom Binding (NIP-OT `kind:32104`)
- `p` with markers `participant`, `assignee`, `delegate` (NIP-AD owns the
  assignment/delegation authority records; these tags are display refs)
- `e` with marker `head`: the latest `kind:32171` Work Event
- `t`: searchable topics

`content` SHOULD be empty, a bounded public-safe summary, or a NIP-44
payload encrypted to the Work audience. Private objective text, prompts,
credentials, customer identity, and provider payloads MUST NOT appear in a
public Work Record.

### 1.3 Work State

The recommended baseline vocabulary is `draft`, `planned`, `active`,
`blocked`, `in_review`, `done`, `canceled`, `superseded`, `archived`.
Deployments MAY configure richer Workflow States (future NIP-WS) that map
onto this baseline. A state label is policy data: displaying `done` proves
nothing about evidence, verification, acceptance, or release, which remain
NIP-EV records.

### 1.4 Example

```json
{
  "kind": 32170,
  "pubkey": "<authority-pubkey>",
  "content": "",
  "tags": [
    ["d", "work-9f31854f"],
    ["org", "org-openagents"],
    ["domain", "development"],
    ["state", "active"],
    ["revision", "7"],
    ["title", "Add lane READMEs to the NIP source directories"],
    ["p", "<owner-pubkey>", "", "owner"],
    ["p", "<agent-pubkey>", "", "delegate"],
    ["a", "32172:<authority-pubkey>:work-9f31854f:obj:3", "", "objective"],
    ["a", "32104:<authority-pubkey>:room-dev", "", "workroom"],
    ["e", "<latest-32171-event-id>", "", "head"],
    ["published_at", "1786500000"],
    ["t", "allwork"]
  ]
}
```

## 2. Work Event (`kind:32171`)

One admitted fact in the Work lifecycle. Address:

```text
32171:<authority_pubkey>:<work_ref>:evt:<seq>
```

The `d` value SHOULD be `<work_ref>:evt:<seq>` so events are unique,
ordered, and enumerable by prefix-free filters on the `work` tag.

### 2.1 Required tags

- `d`: unique event identifier
- `work`: owning `work_ref`
- `seq`: dense monotonic sequence within the Work
- `event`: event kind (see 2.3)
- `p` with marker `actor`: the acting principal
- `occurred_at` and `admitted_at`

### 2.2 Recommended tags

- `e` with marker `intent`: the NIP-WI Work Intent that produced this event
- `e` with marker `admission`: the NIP-WI Admission Result
- `e` with marker `parent`: causal parent Work Events
- `revision`: the Work revision this event produced
- `x`: digest of an off-relay payload
- `reason`: machine-readable reason code

### 2.3 Event kinds

Recommended vocabulary:

`created`, `objective_revised`, `classified`, `related`, `assigned`,
`delegated`, `delegation_revoked`, `state_changed`, `blocked`, `unblocked`,
`session_started`, `session_ended`, `activity_recorded`,
`evidence_attached`, `verification_recorded`, `disposition_recorded`,
`closed`, `reopened`, `superseded`, `archived`.

Deployments MAY extend this list. An unknown event kind MUST be preserved
and displayed as unknown, never reinterpreted.

### 2.4 Ordering and gaps

Clients reconstruct Work history by ordering `seq`. A missing sequence
number is an explicit gap and MUST be surfaced as one; clients MUST NOT
present a history with holes as complete. `created_at` and relay arrival
order are not the ordering authority.

## 3. Work Objective (`kind:32172`)

A versioned statement of what the Work is intended to accomplish. Address:

```text
32172:<authority_pubkey>:<work_ref>:obj:<revision>
```

Required tags: `d`, `work`, `revision`, `x` (digest of the exact objective
bytes), `published_at`. `content` is the public-safe objective text or a
NIP-44 payload for private audiences; either way the digest binds the exact
revision so plans, claims, and evidence can name the objective they were
made against. Revising the objective is a NIP-WI operation and can
invalidate earlier plans; consumers compare `revision` before trusting a
cached plan.

## 4. Outcome Record (`kind:32173`)

The current or terminal synthesis for the Work. Address:

```text
32173:<authority_pubkey>:<work_ref>
```

Recommended tags: `work`, `state` (`open`, `synthesizing`, `terminal`),
`e`/`a` refs with markers `evidence`, `verification`, `disposition`
(NIP-EV), `artifact`, `settlement` (NIP-AC / NIP-OC), plus `revision`.

An Outcome Record is an index, not a verdict. Each referenced fact keeps its
own qualification: evidence is not verification, verification is not
acceptance, acceptance is not release or settlement. An Accepted Outcome
exists only when the NIP-EV Owner Disposition it references says so.

## Security considerations

- **Authority key compromise.** The authority key can rewrite projections.
  Deployments SHOULD rotate through the NIP-OT Organization record and MAY
  countersign high-value records; clients pin the authority key from the
  Organization record, not from prior Work events.
- **Forged records.** Clients MUST check the signer of kinds 32170-32173
  against the Organization's declared authority before merging state.
- **State-label overclaim.** `state=done` is a label. The No-Evidence-
  No-Claim rule applies: completion claims require the NIP-EV chain.
- **Privacy.** Work Records are as public as their least-restricted relay.
  Private Work uses encrypted content, opaque refs, and restricted relays;
  digests and refs are the only public trace.
- **Gap honesty.** Sequence gaps, missing objectives, and unresolvable refs
  are loss-accounting facts and must be displayed, not papered over.

## References

- NIP-01, NIP-09, NIP-44
- NIP-OT: Organizations and Teams (this program)
- NIP-WI: Work Intents and Admission (this program)
- NIP-EV: Evidence, Verification, and Dispositions (this program)
- `docs/allwork/model.md` — the All Work model
- `docs/omega/GLOSSARY.md` — canonical vocabulary

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: Work Record, Work Event, Objective, Outcome
  Record, authority-signed projection rules, and state/gap semantics.
