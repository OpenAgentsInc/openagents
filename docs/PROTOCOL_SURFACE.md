# Protocol Surface (SA/SKL/AC)

This document freezes the in-repo protocol surface used by runtime code.

Authoritative specs:

- `crates/nostr/nips/SA.md`
- `crates/nostr/nips/SKL.md`
- `crates/nostr/nips/AC.md`

## NIP-SA

Kinds:

- `39200` Agent Profile
- `39201` Agent State
- `39202` Agent Schedule
- `39203` Agent Goals
- `39210` Agent Tick Request
- `39211` Agent Tick Result
- `39220` Skill License
- `39221` Skill Delivery
- `39230` Trajectory Session
- `39231` Trajectory Event

Skill linkage tags expected on SA fulfillment events (`39220`, `39221`):

- `["a", "33400:<skill_pubkey>:<d-tag>"]`
- `["e", "<skill_manifest_event_id>", "<relay_hint>"]` (version pin)

## NIP-SKL (Core)

Kinds:

- `33400` Skill Manifest (addressable)
- `33401` Skill Version Log (regular append-only)

Reused kinds:

- `1985` (NIP-32 labels/attestations)
- `5` (NIP-09 publisher-origin revocation)
- `30402` (NIP-99 optional listing surface)

Core does **not** require `31337`.

Optional profile kinds:

- `5390` Skill Search Request (NIP-90 profile)
- `6390` Skill Search Result (NIP-90 profile)

Canonical IDs:

- Skill address: `33400:<skill_pubkey>:<d-tag>`
- Versioned scope: `33400:<skill_pubkey>:<d-tag>:<version>`

## NIP-AC

Kinds:

- `39240` Credit Intent
- `39241` Credit Offer
- `39242` Credit Envelope
- `39243` Credit Spend Authorization
- `39244` Credit Settlement Receipt
- `39245` Credit Default Notice

Skill scope form:

- `scope = skill:<skill_scope_id>:<constraints_hash>`
- `skill_scope_id = 33400:<skill_pubkey>:<d-tag>:<version>`

Recommended skill linkage tags on skill-scoped AC events:

- `["a", "33400:<skill_pubkey>:<d-tag>"]`
- `["e", "<skill_manifest_event_id>"]`

## Prerequisite NIPs

Runtime baseline:

- NIP-01
- NIP-06
- NIP-09
- NIP-32
- NIP-40
- NIP-44
- NIP-59
- NIP-90
- NIP-99

Optional profile/path dependencies:

- NIP-26 (delegated signing profile)
- NIP-57
- NIP-60
- NIP-61
- NIP-87
- NIP-98
