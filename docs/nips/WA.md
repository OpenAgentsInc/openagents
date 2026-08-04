> Status: proposed draft from the All Work NIP program ([`PROPOSED.md`](PROPOSED.md)).
> Layer: 2 — agents and execution.
> Unlike its siblings, this NIP documents an implemented profile: the
> `openagents.signed-workroom.v2` projection and its pinned kind range.

NIP-WA
======

Workroom Activity
-----------------

`draft` `optional`

This NIP specifies the signed Workroom projection: how All Work
collaboration facts — membership, threads, assignments, delegations,
sessions, activities, code changes, reviews, decisions, evidence,
verification and receipt references, and revocations — are published as
actor-signed Nostr events into the Workroom bound to a scope (NIP-OT
`kind:32104`).

It formalizes the `openagents.signed-workroom.v2` profile already
implemented in the OpenAgents All Work contract. Its kind range is
**pinned by that implementation** and, unlike every other range in this
program, is not tentative.

## The projection rule

Everything in this NIP is projection transport:

- an event proves its **signer** and the **serialized projection bytes**,
  including the payload digest — it does not prove the undisclosed
  payload content;
- relay acceptance is transport evidence;
- no Workroom Activity event admits a command, external effect,
  verification, owner acceptance, or release. The canonical records live
  in NIP-WK/WI/EV/AD/AS/AV; the workroom event is their signed,
  portable, causally-ordered shadow. Every receipt in the implemented
  profile fixes `persistedBeforePublish: true`,
  `relayAcceptanceIsAuthority: false`, and `admittedEffect: false`.

## Kinds (pinned)

| Activity kind | Nostr kind |
| --- | ---: |
| membership | 32150 |
| thread | 32151 |
| mention | 32152 |
| assignment | 32153 |
| delegation | 32154 |
| agent_session | 32155 |
| agent_activity | 32156 |
| code_change | 32157 |
| review | 32158 |
| decision | 32159 |
| evidence | 32160 |
| verification_ref | 32161 |
| receipt_ref | 32162 |
| revocation | 32163 |

The contiguous range is versioned by the projection profile. A semantic
change requires a new profile and registry update; it cannot reuse a kind
with different bytes. `32164-32169` remain reserved for profile
expansion.

## 1. Event serialization (profile v2)

A v2 event serializes a NIP-01 event with:

- the signer public key as author;
- whole-second occurrence time as `created_at`;
- the registered activity kind;
- **empty content** — payloads travel by digest; and
- deterministic tags for: the projection profile
  (`openagents.signed-workroom.v2`), the canonical event ref, the actor,
  the Workroom, the optional Work, audience and privacy classes, ledger
  revision, generation, the exact occurrence timestamp, the payload
  digest, causal parents, evidence refs, the optional actor grant and
  grant generation, supersession, and revocation.

The receiving authority recomputes the SHA-256 event id and verifies the
BIP-340 Schnorr signature. Profile v1 (older direct-actor rows without
grant bindings) remains read/replay compatibility only; new admission
requires v2.

## 2. Actors and signer grants

Two signing classes exist:

- **Direct principals.** The actor is `principal:nostr:<signer-pubkey>` —
  the signer is the actor. Direct admission forbids an attached actor
  grant.
- **Organizational, device, and agent actors.** The signer projects on
  behalf of an actor under an authoritative, active
  `purpose:signed-workroom:project-activity` **actor grant**. The grant
  binds issuer, actor, signer key, exact Workroom and Work, permitted
  event kinds, audience/privacy classes, evidence refs, validity
  interval, and generation.

Admission fails closed when the grant resolver is absent or the grant is
missing, expired, revoked, superseded, stale, or scope-mismatched. Both
occurrence and admission time must fall inside the grant interval, and
publication rechecks the current grant before opening a socket — a
revoked grant fences later projection use without a restart. Grant
provisioning is an owner-local operation over an exact prior revision; the
client cannot attach or provision a grant through the signing lane.

## 3. The two-phase external signing lane

Keys stay in their owner's custody via prepare/commit:

1. **Prepare.** The client submits a bounded projection intent plus the
   enrolled signer public key. The authority — not the client — assigns
   the canonical event ref, ledger revision, generation, direct actor,
   profile, the exact unsigned NIP-01 JSON, a preparation expiry, and a
   digest of the configured relay policy.
2. **Sign.** The client signs those exact bytes in its own custody.
3. **Commit.** The authority recomputes the preparation ref, relay-policy
   digest, unsigned template, and event id, then verifies the signature
   through normal admission before persisting.

A substituted tag, content byte, author, kind, timestamp, event id,
signature, principal, relay policy, or an expired preparation fails
closed. Private key material never crosses the process boundary.

## 4. Ordering, causality, and revision

- Admission requires known causal parents, advancing generations, and
  unique event/idempotency identities within the Workroom ledger.
- The ledger revision reconciles with unique canonical event identities;
  a reordered child is rejected; duplicate requests replay exactly.
- Supersession and revocation are first-class: a `revocation` event (or a
  supersession ref on any kind) fences the superseded projection without
  rewriting its signed bytes.

## 5. Relays, audience, and delivery

- The authority selects a server-owned relay set per audience class
  (`workroom`, `private`, `owner_only`); the client cannot supply relay
  targets or change audience after preparation. An absent or invalid
  audience policy makes the lane unavailable — fail closed, not fail
  open.
- Events persist in a durable outbox **before** publication. The
  publisher opens bounded connections to exactly the persisted targets,
  publishes the exact verified event, accepts only a matching relay `OK`,
  and records accepted/rejected/unreachable per target. Partial delivery
  stays retryable; retries touch only unresolved targets; relay-provided
  text is never persisted, so an untrusted relay cannot inject content
  into the ledger.
- Delivery receipts remain transport evidence even when every relay
  accepts.

## 6. Privacy

Payloads travel by digest. Private text, prompts, code, and evidence
bodies never enter the event; the audience class plus the NIP-OT Workroom
Binding's relay policy bound who can even see the projection's existence.
NIP-29 room access and this profile's audience classes compose; neither
is end-to-end encryption unless the payload lane adds it.

## Security considerations

- **Byte substitution.** The commit-side recomputation of template, id,
  and signature is the defense; any drift fails closed.
- **Grant races.** Generation checks at admission and re-checks at
  publication close the revoke-then-publish window.
- **Cross-projection divergence.** Where a fact exists both here and in
  its canonical NIP (an `agent_activity` beside NIP-AV `32290`), the
  canonical record wins and divergence is a review flag.
- **Relay trust.** Multi-relay delivery with per-target acceptance
  records makes single-relay censorship visible without granting any
  relay authority.

## References

- NIP-01, NIP-29, NIP-44
- NIP-OT `kind:32104` — Workroom Binding
- NIP-WK, NIP-WI, NIP-EV (layer 0); NIP-AD, NIP-AS, NIP-AV, NIP-RC,
  NIP-CC, NIP-RV (this layer) — the canonical records this profile
  projects
- `docs/omega/2026-08-03-signed-workroom-projection.md` — the
  implemented contract this NIP formalizes

## Changelog

**v0 (2026-08-03)**

- Initial draft formalizing the implemented
  `openagents.signed-workroom.v2` profile: pinned kinds 32150-32163,
  actor grants, the prepare/commit lane, persist-before-publish
  delivery, and the projection-not-authority rule.
