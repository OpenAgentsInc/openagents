# 50. Accounts and agent grants

> **Status: outline.** This chapter lists what it will say and build. Its
> draft is not written, and paths marked planned do not exist.

## Premise

Chapter 43 says your identity is a key on the network and an account on our
service. This chapter builds the account: how you log in, how an agent gets
its own key, and the grant that says what the agent may do for you. Most of it
is built, and its grants are the first records in this book with a schema in
use.

## Outline

- **One account, many ways in.** An account is an opaque id. An email address,
  a Nostr key and a passkey attach to it in any combination. A Nostr key alone
  is enough.
- **What we don't have.** No passwords. No phone numbers, ever: no SMS login,
  no phone recovery, no phone field. No outside login provider. GitHub and
  Google are connections for repositories and files, not ways in.
- **The words.** It is "Log in", and the path is `/login`.
- **Levels.** An email code gives L1. A passkey, a Nostr key or a recovery
  code gives L2. Some actions need an L2 login from the last five minutes. A
  route names the level it needs, and the app asks you to step up.
- **Sessions.** Opaque tokens that we can revoke at once.
- **Agents are accounts.** An agent has an owner and its own way in: a Nostr
  key that signs every request (NIP-98), or an `oa_key_` API key for tools
  that can't sign. It never uses its owner's key, email or session.
- **The grant.** Scopes, sats a day, sats a payment, an expiry of 90 days by
  default and a year at most, and the addresses the agent may call from. The
  owner approves it at L2 and sees all of it. Anything not granted is refused.
- **What an agent can never do.** Add or remove a way in, change recovery,
  make or widen a grant, raise a cap, export a seed or delete an account.
- **Public attribution.** The owner's key signs that the agent's key acts for
  it (NIP-OA), so a reader of the agent's events can check whose agent it is.
- **Recovery.** If you lose every strong way in, recovery takes 72 hours. You
  can cancel it from any channel, and no member of staff can skip the wait.
- **Freeze, merge, export, delete.** Each is a route you can reach from
  settings.
- **The audit log.** Every grant, change and use is recorded, refused ones
  too, and never a key.
- **Limits are shown** before you reach them: 32 agents an account, and each
  cap on its grant.
- **What ties to the rest of the book.** A grant with `wallet:spend` is the
  allowance of chapter 20. A member's grant (chapter 7) is a different record
  today.

## What we build

Built in the repository today ([docs/auth](../../docs/auth/README.md)):

- Email-code and Nostr login, sessions and the audit log (#13200 to #13206).
- Agent accounts, keys, scopes and grants (#13211).
- Step-up, linking and the 72-hour recovery (#13212, #13213).
- Freeze, merge, data export and account deletion (#13214).
- Passkeys on the web and in the iPhone app (#13215).
- Device keys that bind an app's login to its hardware (#13216).
- `name@openagents.com` Lightning Addresses in forward mode (#13208), and tips
  that pay a maker's own address (#13210).

Planned:

- Lightning Addresses that pay a wallet the app makes for you (chapter 19).
- The account and session records as messages in `proto/`.

It builds on what exists today:

- [`docs/auth/identity-and-methods.md`](../../docs/auth/identity-and-methods.md)
- [`docs/auth/sessions-and-tokens.md`](../../docs/auth/sessions-and-tokens.md)
- [`proto/openagents/account/v1alpha1/agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/web/api/agent-policy.coax`](../../coax/web/api/agent-policy.coax)
- [`coax/web/api/auth-policy.coax`](../../coax/web/api/auth-policy.coax)
- [`coax/web/api/nip98.coax`](../../coax/web/api/nip98.coax)
- [`coax/lib/nostr/nipoa.coax`](../../coax/lib/nostr/nipoa.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Grant request, grant and agent key | `openagents.account.v1alpha1` `GrantRequest`, `Grant`, `SpendCaps`, `AgentKey` ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, as binary protobuf (`application/x-protobuf`) | In use: one generated codec, `coax/web/api/agent-wire.coax` |
| The owner's attestation of an agent key | `openagents.account.v1alpha1` `OwnerAttestation`; on the network it is an `auth` tag (NIP-OA) | HTTP inside the grant request; Nostr on the agent's events | In use in the API. NIP-OA is an external format |
| A signed request | A NIP-98 event | HTTP `Authorization` header | Built. External format |
| Account, authenticators, sessions, audit log | `openagents.account.v1` and `openagents.credential.v1` in the spec ([spec 15.1](../../docs/protocol/openagents-protocol-spec.md#151-openagentsaccountv1), [spec 15.2](../../docs/protocol/openagents-protocol-spec.md#152-openagentscredentialv1)) | HTTP JSON | Hand-written JSON. The spec's sketch predates this login, and the [protocol review](../../docs/protocol/2026-11-11-protocol-review.md) orders its rewrite as `v1alpha1` |

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): reputation, identity and payments figured out once
  - [Episode 82, *Nostr Login*](../../docs/transcripts/082.md)
  - [Episode 207, *Your Keys, Your Coins, Your Identity*](../../docs/transcripts/207.md): one seed for a Nostr key and a wallet key
- **NIPs:** [NIP-98](../../nostr/nips/official/98.md), [NIP-OA](../../nostr/nips/block/NIP-OA.md), [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md)
- **Also:**
  - [OpenAgents authentication](../../docs/auth/README.md) and its [decisions](../../docs/auth/decisions.md)
  - [Nostr From Scratch, chapter 87](../../nostr/book/ch087-owner-attestation-and-agent-authentication.md)
  - [The OpenAgents Protocol](../../docs/protocol/openagents-protocol-spec.md), section 15
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P14

## Open questions for the owner

1. Do a member's grant (chapter 7) and an agent's grant become one record, or
   stay two?
2. Which scopes come after `profile:read` and `wallet:spend`?
