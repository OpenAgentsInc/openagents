> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 1 — planning.

NIP-CN
======

Customers and Needs
-------------------

`draft` `optional`

This NIP connects external stakeholder context to Work: **Customer**
records, typed **Customer Needs** linking customers to Work, and normalized
**Customer Signals** carrying source evidence.

It is the most privacy-sensitive NIP in the planning layer, so it inverts
the usual default: **the public shape of every record is opaque.** Public
relays see stable refs, typed links, and digests. Identity, contact
details, consent facts, and source material are NIP-44-encrypted to the
Organization audience or stay off-relay entirely.

The authority boundary: customer identity never creates a commitment,
grant, or Work State. Knowing who wants something is context; committing
to deliver it is a separate typed record (a Service Promise or admitted
Work), and nothing in this NIP produces one.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32260 | Addressable | Customer |
| 32261 | Addressable (unique `d`) | Customer Need |
| 32262 | Addressable (unique `d`) | Customer Signal |
| 32263-32266, 32268-32269 | — | Reserved for future NIP-CN use |

`kind:32267` is excluded because the official registry assigns it to Software
Application. NIP-CN does not claim or reinterpret that kind.

All are authority-signed, mutated through NIP-WI intents
(`customer.create`, `customer.update`, `need.link`, `signal.record`).

## 1. Customer (`kind:32260`)

One external or internal recipient/stakeholder record. Address:

```text
32260:<authority_pubkey>:<customer_ref>
```

### 1.1 Required tags

- `d`: opaque stable `customer_ref` — MUST NOT be derived from the
  customer's name, domain, or any identifying value
- `org`
- `status`: configured relationship state ref or baseline value
  (`prospect`, `active`, `churned`, `internal`)
- `revision`, `published_at`

### 1.2 Recommended tags

- `tier`: configured importance/service classification
- `p` with marker `owner`: the internal owning principal
- `policy` with markers `access`, `retention`, `consent`: the policies
  governing this record's private payload
- `x`: digest of the encrypted identity payload

`content` is the NIP-44-encrypted identity payload for the Organization
audience: name, organization, contacts, source system refs. On public
relays `content` MAY instead be empty with the payload held off-relay
behind the digest. A plaintext public Customer record is valid only for
deliberately public stakeholders (for example, an open-source consumer
project that asked to be listed).

## 2. Customer Need (`kind:32261`)

A typed connection between a Customer and Work. Address:

```text
32261:<authority_pubkey>:<need_ref>
```

### 2.1 Required tags

- `d`: unique `need_ref`
- `org`
- `a` with marker `customer`: the Customer record
- `a` with marker `subject`: the linked Work, Issue, Project, Document,
  or Comment address
- `state`: `open`, `satisfied`, `declined`, `withdrawn`
- `published_at`

### 2.2 Recommended tags

- `priority`: the customer-side importance (`critical`, `important`,
  `nice_to_have`)
- `e`/`a` with marker `signal`: the Customer Signals evidencing the need
- `p` with marker `actor`: who recorded the link
- `x`: digest of an encrypted need description

### 2.3 Rules

- A Need aggregates demand: many Customers can hold Needs against one
  Work, and tracker surfaces (NIP-PI carries `need` refs) can show demand
  counts without exposing which customers they are.
- `satisfied` is set only when the linked Work reaches an Accepted
  Outcome (NIP-EV disposition) — the need's resolution inherits the
  proof-rung discipline; shipped-but-unaccepted Work leaves the need
  `open`.
- A Need is not a commitment. Promising delivery to the customer is a
  separate Service Promise record under its own authority.

## 3. Customer Signal (`kind:32262`)

One normalized piece of source evidence: a request, complaint,
observation, or opportunity. Address:

```text
32262:<authority_pubkey>:<signal_ref>
```

### 3.1 Required tags

- `d`: unique `signal_ref`
- `org`
- `source`: typed source class (`support_ticket`, `sales_call`, `forum`,
  `social`, `interview`, `telemetry`, `intake_form`, or
  deployment-declared)
- `consent`: consent class for the captured material (`explicit`,
  `implied_business`, `public_statement`, `internal`)
- `x`: digest of the exact captured source material
- `published_at`

### 3.2 Recommended tags

- `a` with marker `customer`: the Customer, when attribution is known and
  consented
- `a` with marker `need`: Needs this signal supports
- `e`/`url` with marker `source`: the source delivery ref
- `sentiment`: `positive`, `neutral`, `negative` — a classification
  claim, not a fact

`content` is the NIP-44-encrypted normalized excerpt for the Organization
audience, or empty with material off-relay. Raw source material — full
tickets, call transcripts, emails — SHOULD stay off-relay; the signal
carries the digest and a bounded excerpt at most.

### 3.3 Rules

- Signals are append-only evidence. Correcting a mis-attribution
  publishes the record with the `customer` ref removed and a `reason`,
  never a silent edit of history.
- A signal whose `consent` class forbids a use (for example quoting a
  private interview publicly) is enforced at every projection: the
  consent class travels with every derived display.
- Signals can arrive through the NIP-WI Work Candidate intake: an intake
  candidate that is really demand evidence is triaged into a Signal
  rather than into Work.

## Security considerations

- **Re-identification.** Opaque refs defeat casual identification, not
  correlation. Need/Signal link structure itself can identify a customer
  to an informed observer; Organizations with sensitive customer bases
  keep even the opaque graph on restricted relays.
- **Consent drift.** The `consent` class is load-bearing data. Automation
  that surfaces signals into public contexts (release notes, promise
  evidence) MUST check it, and `public_statement` is the only class
  eligible for public quotation.
- **Injection.** Signal content is externally authored text — prime
  prompt-injection material. It is data everywhere it flows; it cannot
  steer tools, widen access, or create Work without triage.
- **Retention.** Deletion under a retention policy tombstones the private
  payload and republishes opaque records; the NIP cannot promise erasure
  of relay-copied ciphertext and MUST NOT claim it.

## References

- NIP-01, NIP-09, NIP-44
- NIP-WK, NIP-WI, NIP-EV (layer 0)
- NIP-PI (this layer) — demand refs on Issue surfaces
- `docs/omega/GLOSSARY.md` — Customer, Customer Need, Customer Signal,
  Service Promise boundary terms

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: opaque-by-default Customer records, Need links
  with proof-rung-disciplined satisfaction, and consent-classed Signals.
