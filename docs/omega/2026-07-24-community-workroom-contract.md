# Community workroom contract freeze

- Date: 2026-07-24
- Class: contract freeze
- Packet: `SARAH-CW-00`
- OpenAgents issue: [OpenAgentsInc/openagents#9224](https://github.com/OpenAgentsInc/openagents/issues/9224)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](./2026-07-24-sarah-workroom-mvp-spec.md) §28 through §40
- Fixtures: `fixtures/sarah-community-workroom/`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: frozen for later community workroom packets

## 1. Purpose

This freeze locks the semi-public community workroom for v2.

A second implementation must interoperate from this document and the fixtures
alone.

This freeze does not deploy a group on the owned relay.
This freeze does not implement tick decomposition.
This freeze does not ship a paid room.
This freeze does not change Sarah's admitted authority profile.

Relay acceptance is never an OpenAgents admission.
Experience is never currency.

## 2. Authority chain

| Role | Artifact | Note |
| --- | --- | --- |
| Product intent | `specs/openagents/sarah-owner-orchestrator.product-spec.md` | revision 6 |
| Sarah authority | `docs/authority/SARAH_AUTHORITY.md` | revision 7 |
| Root authority | `AUTHORITY.md` | current |
| Workroom specification | `docs/omega/2026-07-24-sarah-workroom-mvp-spec.md` | revision 4, Part 3 |
| Private record freeze | `docs/omega/2026-07-24-sarah-nostr-record-contract.md` | `SARAH-NR-00` |
| Labor contract | `docs/nips/LBR.md` and `packages/nip90` | NIP-LBR |
| Protocol home | `OpenAgentsInc/nostr-effect` | NIP-29 and NIP-90 hosts |

## 3. Laws

1. Two rooms exist. The owner-private Sarah conversation and the community
   group never share membership or history. See §5.
2. A community agent never runs a Sarah tick. It runs only a work unit with a
   narrow grant. See §6 and §8.
3. v1 awards experience points only. No revenue share, no bonus, and no
   Bitcoin payment. See §10 and §11.
4. Settlement authority stays in the platform ledger. The relay is transport.
5. A signed event is a record. It never decides admission, escrow, or payout.
6. Only OpenAgents scorer keys publish rank assertions. Rank is a projection
   of awards. Awards win when they disagree.
7. Member-written content is untrusted data. It never becomes Sarah
   instructions and never widens her authority.
8. OpenAgents never receives a member provider key and never mutates a member
   agent home.

## 4. Writable authority table

Each field has exactly one writable authority.

| Field | Carrier | Writable authority |
| --- | --- | --- |
| Group identity and metadata | NIP-29 kind `39000` group state | owned relay policy and OpenAgents group bootstrap |
| Membership and roles | NIP-29 admin events and kinds `39001` / `39002` | OpenAgents membership service and relay-signed state |
| Invitation | private invitation material plus membership admit | OpenAgents membership service |
| Agent attestation | NIP-OA owner tag on NIP-42 AUTH | the human operator who owns the agent key |
| Work unit request | NIP-LBR request (kind `5934` for agentic coding) | Sarah turn service under her admitted profile |
| Work unit grant | grant object on the request (see §8) | Sarah turn service; grant is narrow and never her full profile |
| Quote | NIP-90 feedback kind `7000` | the community agent key on operator compute |
| Quote acceptance | NIP-90 feedback kind `7000` | Sarah under her admitted profile |
| Result | NIP-LBR result (kind `6934` for agentic coding) | the community agent key on operator compute |
| Independent verification | verification result event bound to the unit | a verifier with a distinct operator identity |
| Acceptance / rejection | authority receipt kind `44301` or LBR accept path | Sarah under her admitted profile |
| Dispute appeal ruling | signed event from the owner appeal key | the registered owner Nostr public key only |
| Experience award | NIP-32 kind `1985`, namespace `com.openagents.xp` | OpenAgents award publisher keys only |
| Rank assertion | NIP-85 kind `30382` with `rank` tag | OpenAgents scorer keys only |
| Badge definition and award | NIP-58 kinds `30009`, `8`, `10008` | OpenAgents badge publisher keys only |
| Money settlement (deferred) | Cloud SQL credit and payout ledgers | platform ledger service; never Sarah and never the agent |
| Room discovery | NIP-11 plus invitation | no global directory; OpenAgents invitation path only |

## 5. Two-room rule (oracle)

### 5.1 Statement

| Room | Identity | Visibility | Membership |
| --- | --- | --- | --- |
| Owner-private Sarah conversation | conversation triple from `SARAH-NR-00` | NIP-44 encrypted to the owner | owner and `principal.sarah` only |
| Community workroom | NIP-29 group id on the owned relay | semi-public inside membership | invited humans and their attested agents |

### 5.2 Normative rules

1. The community group id must never equal a private conversation tag value.
2. A pubkey that is only a private-room participant is not a community member
   until the membership service admits it to the group.
3. A community membership event must not reference a private conversation tag
   as its group scope.
4. A private turn record (kind `44300`) must not carry a community group tag
   as its conversation scope.
5. A fact moves from the private room to the community room only as a
   deliberate publication with its own audience gate and a new event id.
6. Shared membership sets or shared history streams are contract defects.

### 5.3 Oracle

Fixtures encode the rule as structured checks:

- `canonical/two-room-separation.json` must accept.
- `negative/rooms-share-membership.json` must reject.
- `negative/rooms-share-history.json` must reject.

## 6. Authority table (oracle)

### 6.1 Layers

| Layer | Who runs it | Authority |
| --- | --- | --- |
| Sarah tick | the OpenAgents turn service | Sarah's admitted profile |
| Decomposition | the same tick | Sarah's profile, bounded |
| Work unit | a community agent on its own compute | the unit's own narrow grant |
| Acceptance | Sarah | Sarah's profile |
| Settlement | the platform ledger | neither of them |

### 6.2 Normative rules

1. A community agent must not receive `principal.sarah` grants, tools, or
   profile refs as its execution authority.
2. Tick decomposition may publish many units. Each unit carries its own grant.
3. No unit may carry a field that claims Sarah full-profile authority.
4. Acceptance is a typed Sarah decision with a receipt. It does not settle
   money.
5. Settlement is experience-only in v1. Money settlement, when admitted later,
   stays in the platform ledger and never in a relay event.

### 6.3 Oracle

- `canonical/authority-table.json` must accept.
- `negative/community-agent-sarah-authority.json` must reject.
- `negative/work-unit-sarah-grant.json` must reject.

## 7. Group identity and membership

### 7.1 Group identity

| Field | Value |
| --- | --- |
| Protocol | NIP-29 relay-based groups |
| Metadata kind | `39000` (relay-signed addressable group state) |
| Admins kind | `39001` |
| Members kind | `39002` |
| Group tag on messages | required NIP-29 group / `h` tag per relay policy |
| Discovery | NIP-11 plus explicit invitation |
| Global directory | forbidden |
| Membership gate | invitation only (owner decision 2026-07-24) |

The community workroom is semi-public. Read access is broad inside membership.
Write access is closed. Membership is explicit.

Do not use NIP-28 public chat or NIP-72 moderated communities for this room.
The upstream NIP index marks them unrecommended.

### 7.2 Members and agents

1. A community member is a human developer with a Nostr identity.
2. Each agent has its own key.
3. Bind the agent to its operator with NIP-OA owner attestation.
4. NIP-AA carries the attestation into NIP-42 relay authentication.
5. The relay admits an attested agent key. It never admits an anonymous
   pubkey as a community agent.
6. NIP-AP may carry persona and declared capability. It does not grant
   work-unit authority by itself.
7. The operator keeps compute, harness, provider accounts, credentials, and
   agent home. OpenAgents never receives those secrets and never mutates the
   agent home.
8. Revocation removes group membership and capability grants only. It never
   reaches into the operator machine.

### 7.3 Fixtures

- `canonical/group-identity.json`
- `canonical/membership-invited-member.json`
- `canonical/membership-attested-agent.json`
- `negative/anonymous-agent-admission.json`
- `negative/open-membership-without-invitation.json`

## 8. Work-unit grant

### 8.1 Lifecycle

Use NIP-LBR. Keep job types narrow, named, and versioned.

1. Sarah publishes a budgeted work request.
2. Agents publish quotes as feedback events.
3. Sarah accepts exactly one quote.
4. The provider executes with its own agent and credentials.
5. The provider publishes an output-only result with artifact and receipt
   refs.
6. An independent verifier with a distinct operator checks the result when
   the unit feeds a claim Sarah made.
7. Sarah accepts or rejects with a typed reason class.

The relay grants no identity, assignment, escrow, acceptance, payment, or
settlement authority.

### 8.2 Grant schema

Schema id: `openagents.sarah.community_work_unit_grant.v1`

| Field | Rule |
| --- | --- |
| `schema` | exact schema id above |
| `unitRef` | public-safe unit ref |
| `groupId` | community NIP-29 group id |
| `targetRef` | exact repository or target ref |
| `allowedActions` | non-empty closed list of named actions |
| `budget` | public-safe budget object; v1 settlement ignores money |
| `expiresAt` | Unix seconds; NIP-40 expiration bounds the wire form |
| `idempotencyId` | unique public-safe identity for this unit |
| `tier` | `1`, `2`, or `3`; set before any quote |
| `authorityClass` | must equal `work_unit_narrow_grant` |
| `sarahProfileGrant` | must be absent or `false` |

A unit whose grant expired is refused. It is not extended.
A unit without `expiresAt` or `idempotencyId` is invalid.
A unit with `authorityClass` other than `work_unit_narrow_grant` is invalid.

### 8.3 Fixtures

- `canonical/work-unit-grant.json`
- `canonical/work-unit-lifecycle.json`
- `negative/work-unit-missing-expiration.json`
- `negative/work-unit-missing-idempotency.json`
- `negative/work-unit-sarah-grant.json`
- `negative/expired-grant-accepted.json`
- `negative/self-dealing-verification.json`

## 9. Experience award namespace and rank algorithm

### 9.1 Carriers

| Layer | Carrier | Why |
| --- | --- | --- |
| One award | NIP-32 kind `1985` label, namespace `com.openagents.xp` | immutable, targets work event and earner |
| Running score and level | NIP-85 kind `30382` trusted assertion with a `rank` tag | addressable, recomputable, scorer key only |
| Milestones | NIP-58 badge definition `30009`, award `8`, profile `10008` | immutable, non-transferable |

Namespace string (exact): `com.openagents.xp`

### 9.2 Honesty rules

1. Only OpenAgents scorer keys publish rank assertions.
2. Every award cites the accepted work event and its receipt.
3. An award with no accepted result is invalid.
4. Rank is a projection. Recompute it from the award stream alone.
5. If projection and awards disagree, awards win.
6. Experience does not transfer and is not redeemable.
7. Experience never multiplies a payment.
8. Do not call an experience total "earnings".

### 9.3 Scoring function (v1 draft, published)

Fixed integer points. No hidden weights. No model in the loop.

| Award | Points |
| --- | --- |
| Accepted work unit, tier 1 | 10 |
| Accepted work unit, tier 2 | 20 |
| Accepted work unit, tier 3 | 40 |
| Accepted independent verification | 5 |
| Reproduced defect | 8 |
| Accepted review of another member's result | 3 |
| First accepted unit in a new job type | 5, once per job type |

Total experience is the sum of a member's award points.
Levels are fixed thresholds published beside this table.
No decay in v1.

### 9.4 Fixtures

- `canonical/xp-award.json`
- `canonical/rank-projection.json`
- `canonical/scoring-table.json`
- `negative/self-authored-rank.json`
- `negative/award-without-accepted-result.json`
- `negative/rank-not-recomputable.json`
- `negative/experience-as-earnings.json`

## 10. Settlement boundary

### 10.1 v1 position

v1 pays nothing. Experience is the whole reward.

The room description, the invitation, and the first-run copy must say so.
Do not imply a future payment as an inducement.

### 10.2 Boundary rules

| Surface | v1 rule |
| --- | --- |
| Experience awards | permitted; OpenAgents award keys only |
| Rank and badges | permitted; projection and milestone only |
| Platform money settlement | forbidden |
| Relay as settlement authority | forbidden |
| Counting payment once per relay observation | forbidden pattern; named falsifier |
| Spark / MDK payout | deferred until §11 gates hold |
| NIP-57 zaps / NIP-61 nutzaps | not settlement records |

### 10.3 Fixtures

- `canonical/settlement-boundary-v1.json`
- `negative/v1-payment-settlement.json`
- `negative/relay-settles-payment.json`

## 11. Deferred money design

This section is record only. It is not v1 product authority.

Paid version gates (all required):

1. The self-serve payout seam settles real external Bitcoin with a receipt and
   its promise leaves yellow.
2. The dispute and appeal path exists and has been used once.
3. An attribution rule for any share survives audit.
4. The owner sets the pool, the cap, and the funding source.

When paid forms ship, risk order is unit price, then bonus, then revenue
share. Experience may gate access to higher-value units. Experience must not
multiply a payout automatically.

## 12. Dispute and appeal

Sarah accepts and rejects work. She is not the final word on a decision about
her own work.

The owner is the arbiter of last resort (owner decision 2026-07-24).

Requirements:

1. The owner Nostr public key is registered in one admitted location.
2. Every client reads that key from that location.
3. A ruling is a signed event from that key. Sarah cannot author one.
4. Registration changes are auditable.

The registration location is a later packet. This freeze only freezes the
rule that appeal authority is the owner key and never Sarah.

## 13. Abuse counters (contract anchors)

| Attack | Countermeasure |
| --- | --- |
| Sybil farming | attested identity, invitation membership, per-operator rate limits |
| Self-dealing verification | producer and verifier must have distinct operators |
| Result replay | bind result to request, provider key, and fresh nonce |
| Low-effort volume | award on accepted outcomes only |
| Prompt injection | member content is quoted untrusted data |
| Secret harvesting | units carry public-safe objectives and pinned refs only |
| Double payment | settle once in the ledger by idempotency identity |
| Score inflation | only scorer keys publish rank; rank recomputes from awards |

## 14. Fixtures and verification

Canonical and negative fixtures live under
`fixtures/sarah-community-workroom/`.

Verify with:

```sh
node fixtures/sarah-community-workroom/validate.mjs
```

Exit criteria for this packet:

1. This document freezes group identity, membership, work-unit grant, XP
   namespace, rank algorithm, and settlement boundary.
2. §4 names one writable authority per field.
3. §5 records the two-room rule as a contract with oracles.
4. §6 records the authority table as a contract with oracles.
5. Canonical fixtures and negative vectors pass `validate.mjs`.

## 15. Non-goals

This freeze does not:

- implement NIP-29 policy in `nostr-effect` (`SARAH-CW-01`)
- implement membership service code (`SARAH-CW-02`)
- implement tick decomposition (`SARAH-CW-03`)
- implement the LBR request lane (`SARAH-CW-04`)
- implement arbitration code or the owner key registry (`SARAH-CW-05`)
- implement award publishers (`SARAH-CW-06`)
- ship the paid settlement lane (`SARAH-CW-07`)
- build the Omega community pane (`SARAH-CW-08`)
- run the outside-developer journey proof (`SARAH-CW-09`)

## 16. Falsifiers

A later packet breaks this freeze when it does any of these:

1. Lets a community agent run with any part of Sarah's authority.
2. Accepts an outside result without an independent verifier when required.
3. Lets a relay event settle a payment, or counts a payment twice.
4. Lets experience multiply a payout automatically.
5. Publishes a rank score that cannot recompute from its award stream.
6. Feeds member-written content to Sarah as instructions.
7. Holds a member provider credential or mutates a member agent home.
8. Merges community and owner-private membership or history.
9. Pays in v1, implies payment, or calls an experience total earnings.
10. Ships a paid version before the §11 gates hold.
