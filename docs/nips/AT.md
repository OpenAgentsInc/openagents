> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 3 — automation and attention.

NIP-AT
======

Attention and Notifications
---------------------------

`draft` `optional`

This NIP defines the attention layer: principal-scoped **Attention Items**
— derived records that ask a person or agent to review, answer, approve,
recover, or notice something — and **Notification Subscriptions** that
tune what reaches whom.

Boundaries:

- An Attention Item is a projection with source refs. It is not Work
  State, and dismissing it changes nothing about the underlying record.
- Triggering a notification never triggers Work: acting on attention is
  a NIP-WI intent like any other action.
- Attention is private to its recipient. Public relays never learn what
  a principal is being asked to look at.

## Kinds

This NIP reserves:

| Kind  | Type        | Description |
| ----- | ----------- | ----------- |
| 32360 | Addressable (unique `d`) | Attention Item |
| 32361 | Addressable | Notification Subscription |
| 32362-32369 | — | Reserved for future NIP-AT use |

Attention Items are authority-signed, encrypted to the recipient.
Subscriptions are **principal-signed** self-records (the Block NIP-RS
pattern): the recipient owns their preferences.

## 1. Attention Item (`kind:32360`)

Address:

```text
32360:<authority_pubkey>:<recipient_hint>:<item_ref>
```

where `<recipient_hint>` is an HMAC-blinded recipient token (the NIP-AE
slug-blinding pattern), so the address itself does not reveal who is
being notified.

### 1.1 Required tags

- `d`: `<recipient_hint>:<item_ref>` (unique)
- `p` with marker `recipient` — MAY be omitted on public relays when the
  blinded hint suffices for the recipient's own filters
- `reason`: the attention reason (see 1.3)
- `state`: `open`, `read`, `snoozed`, `done`, or `dismissed`
- `published_at`

### 1.2 Content

`content` is NIP-44-encrypted to the recipient and carries the typed
payload:

- the stable subject ref (Work, Session, activity, proposal, review,
  candidate, Loop);
- the grouping key (so twelve events on one Work collapse to one row);
- bounded display text;
- the required-action ref where one exists (the elicitation to answer,
  the proposal to dispose, the review to verdict); and
- source and freshness facts.

State changes (`read`, `snoozed` with a wake time, `done`, `dismissed`)
are recipient-submitted NIP-WI intents; the authority republishes the
item. `done` on an item does not complete its subject — only the
subject's own records do that.

### 1.3 Attention reasons

Recommended vocabulary, aligned with the Inbox groupings the program's
surfaces need:

`assigned`, `delegated_active`, `waiting_answer`, `waiting_approval`,
`mentioned`, `blocked`, `stale`, `agent_failed`, `budget_exceeded`,
`verification_completed`, `verification_disagreed`, `review_requested`,
`checks_failed`, `decision_required`, `proposal_open`,
`loop_paused`, `candidate_pending`, `sla_at_risk`, `degraded`
(signer/relay/sync/runtime/repository health).

Unknown reasons are preserved. Clients SHOULD support "by people,"
"by agents," "high-risk," and "accepted-outcome" filters so automation
volume does not bury human-required decisions.

## 2. Notification Subscription (`kind:32361`)

The recipient's own preference record. Address:

```text
32361:<recipient_pubkey>:<subscription_ref>
```

Signed by the recipient; `content` is NIP-44 self-encrypted and carries
the preference set:

- the subscribed source: a resource ref (Work, Project, Team, Customer,
  Label, view) or a reason class;
- delivery classes (`inbox`, `push`, `digest`, `mute`) per source;
- quiet hours and digest cadence.

The authority reads subscriptions (the recipient grants it decrypt
access per the deployment's key arrangement, or mirrors preferences
through an admitted settings channel) and filters Attention Item
generation accordingly. A subscription can suppress delivery; it cannot
subscribe a principal into records their authorization does not cover —
audience filtering runs first, always.

## 3. Composition with the Block lane

This NIP deliberately owns only *what needs attention*:

- **Read state** across devices is Block NIP-RS (self-encrypted CRDT
  read positions) — `state=read` here is coarse item state, not the
  per-thread read frontier.
- **Reminders** are Block NIP-ER — a self-scheduled `not_before` record;
  an Attention Item with `state=snoozed` SHOULD be implemented over the
  same due-signal machinery.
- **Push wake-ups** are Block NIP-PL push leases — the wake payload is a
  fixed reconnect constant, and no Attention Item content ever transits
  a platform push service.

## Security considerations

- **Metadata leakage.** Who is told about what is sensitive even when
  content is encrypted. The blinded recipient hint, encrypted payloads,
  and restricted-relay publication together bound the leak to traffic
  shape; deployments with stronger needs route attention over NIP-59
  gift wrap or private relays only.
- **Attention flooding.** Grouping keys, subscription mutes, and the
  reason vocabulary are the defense against an automation storm drowning
  the one `decision_required` row that matters; clients MUST keep
  human-required reasons visually distinct from informational ones.
- **Action confusion.** The required-action ref points at the record to
  act on; the item itself accepts only state intents. A client that
  submits substantive actions from a notification row still submits
  them against the subject, under the subject's checks.
- **Stale attention.** Items carry source freshness; an item whose
  subject has since resolved renders as stale rather than re-urging
  action.

## References

- NIP-01, NIP-44, NIP-59
- NIP-WI (state intents, candidates), NIP-AV (elicitations), NIP-TP
  (proposals), NIP-RV (reviews), NIP-WS (SLA), NIP-AL (loops)
- Block NIP-RS, NIP-ER, NIP-PL — read state, reminders, push leases
- Block NIP-AE — the HMAC blinding pattern
- `docs/omega/GLOSSARY.md` — Attention Item, Attention Inbox,
  Notification, Notification Subscription

## Changelog

**v0 (2026-08-03)**

- Initial proposed draft: encrypted recipient-blinded Attention Items,
  principal-owned Subscriptions, the reason vocabulary, and Block-lane
  composition.
