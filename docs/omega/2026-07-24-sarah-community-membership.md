# Sarah community membership

- Date: 2026-07-24
- Class: contract pointer
- Packet: `SARAH-CW-02`
- OpenAgents issue: [OpenAgentsInc/openagents#9227](https://github.com/OpenAgentsInc/openagents/issues/9227)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](./2026-07-24-sarah-workroom-mvp-spec.md) §31 and §40
- Implementation: `packages/sarah/src/community/`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: admitted for MVP membership helpers

## 1. Purpose

This document points to the community membership helpers for the Sarah
workroom v2 lane.

The helpers cover:

1. invitation-only membership for a NIP-29 group
2. NIP-OA owner attestation that binds each agent key to its human operator
3. NIP-AA AUTH templates that carry that attestation into NIP-42
4. NIP-AP persona references for declared public capability
5. immediate revocation of an agent or a member
6. per-operator rate limits (anti-sybil)

This packet does not deploy a relay.
This packet does not open the community room.
This packet does not implement NIP-29 group admin events (`SARAH-CW-01`).
This packet does not settle payment.

## 2. Laws

1. A community member is a human developer with a Nostr identity.
2. Each agent has its own key. NIP-OA binds the agent to the operator.
3. The relay admits an attested agent key. It refuses an anonymous pubkey.
4. The operator keeps compute, harness, provider accounts, credentials, and
   the agent home. OpenAgents never receives a provider key. OpenAgents never
   mutates an agent home.
5. Revocation is immediate and cheap. It revokes group membership and the
   capability grant. It never reaches into the operator machine.
6. The membership gate is **invitation only** (owner decision 2026-07-24).
7. Rate limits key on the operator, not on the agent key. One operator with
   many keys must not multiply throughput.

## 3. Module map

| Path | Role |
| --- | --- |
| `packages/sarah/src/community/types.ts` | Membership, invitation, agent binding, gate, errors |
| `packages/sarah/src/community/attestation.ts` | NIP-OA attach and verify, NIP-AA AUTH, NIP-AP persona, public-safe guard |
| `packages/sarah/src/community/membership.ts` | Invitation ledger, attach, revoke, relay admission, rate limit |
| `packages/sarah/src/community/community.test.ts` | Oracles for gate, attestation, revoke, anti-sybil |

Import path: `@openagentsinc/sarah/community`.

Schema id: `openagents.sarah.community_membership.v1`.

## 4. Membership gate

| Mode | Status |
| --- | --- |
| `invitation_only` | **admitted** |
| `application_with_review` | reserved |
| `open_with_probation` | reserved |

Open join without an invitation fails with `invitation_required`.

## 5. Attestation wire form

NIP-OA auth tag (same form as Part 2 Sarah identity):

```text
["auth", <operator-pubkey-hex>, <conditions>, <sig-hex>]
```

The operator signs over
`SHA256("nostr:agent-auth:" || agentPubkey || ":" || conditions)`.
Self-attestation is forbidden.

NIP-AA carries exactly one `auth` tag on a kind `22242` AUTH event.

NIP-AP persona refs use kind `30175` with a public `d` slug and declared
capability labels only. Secrets stay off the public body.

## 6. Revocation

| Action | Effect |
| --- | --- |
| `revokeAgent` | Agent status and capability grant become `revoked`. Agent leaves the index. |
| `revokeMember` | Member status becomes `revoked`. Every agent under the member is revoked. |

Neither action writes to the operator machine. Neither action requests a
provider key. Public records pass `assertCommunityPublicSafe`.

## 7. Verification

```sh
pnpm --dir packages/sarah test -- src/community/community.test.ts
pnpm --dir packages/sarah test
```
