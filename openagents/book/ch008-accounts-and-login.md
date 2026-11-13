# 8. Accounts and login

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. Most of the login it describes runs today in
> `coax/web/api`; the shared app flow is the plan.

## Premise

The public site needs no account. Everything after it does. This chapter
logs you in on the web and on the iPhone through one shared flow, and it
introduces the agent account and its grant, because both already exist.
Basic login depends on nothing later in the book: no wallet, no marketplace
and no plugin.

## Draft

An account is an opaque ID. An email address, a Nostr key and a passkey
attach to it in any combination, and a Nostr key alone is enough. There are
no passwords. We never collect a phone number: no SMS login, no phone
recovery, no phone field. GitHub and Google are connections for
repositories and files, not ways in. The words are "Log in", and the path is
`/login`.

The login is built ([docs/auth](../../docs/auth/README.md)). The server's
rules are Coax in [`coax/web/api/auth-policy.coax`](../../coax/web/api/auth-policy.coax).
The native apps' side is already one shared module:
[`apps/shared/app-login.coax`](../../apps/shared/app-login.coax) decides what
to post and what each answer means, and
[`apps/shared/app-login-flow.coax`](../../apps/shared/app-login-flow.coax)
says which request comes next. A host only sends the request and keeps the
record. This chapter puts that flow behind chapter 2's actions, so the web
page and the iPhone screen send `account.submit_email` and
`account.submit_code` and read the same state. Chapter 6's case then checks
both.

Levels decide what a session may do. An email code gives L1. A passkey, a
Nostr key or a recovery code gives L2. Some actions need an L2 login from the
last five minutes, and the app asks you to step up. Sessions are opaque
tokens that we can revoke at once. The iPhone app binds its login to a device
key, and its tokens live in the Keychain, never in the shared state or a
trace.

Logging out ends everything the session started. A request still in flight
when you log out is cancelled, and its late answer is dropped (chapter 5). A
held reply can't appear in the next person's screen.

An agent is an account too. It has an owner and its own way in: a Nostr key
that signs every request (NIP-98), or an `oa_key_` API key for tools that
can't sign. It never uses its owner's key, email or session. Its grant says
what it may do: scopes, sats a day, sats a payment, an expiry of 90 days by
default and a year at most, and the addresses it may call from. The owner
approves the grant at L2 and sees all of it. An agent can never add or remove
a way in, change recovery, make or widen a grant, raise a cap, export a seed
or delete an account. The owner's key signs that the agent's key acts for it
(NIP-OA), so anyone reading the agent's events can check whose agent it is.

Recovery, freeze, merge, export and delete are routes you reach from
settings (chapter 10). Losing every strong way in starts a 72-hour recovery
that you can cancel from any channel. The audit log records every grant,
change and use, refused ones too, and never a key. Limits are shown before
you reach them: 32 agents an account, and each cap on its grant.

## What we build

Built in the repository today ([docs/auth](../../docs/auth/README.md)):

- Email-code and Nostr login, sessions and the audit log (#13200 to #13206).
- Agent accounts, keys, scopes and grants (#13211).
- Step-up, linking and the 72-hour recovery (#13212, #13213).
- Freeze, merge, data export and account deletion (#13214).
- Passkeys on the web and in the iPhone app (#13215).
- Device keys that bind an app's login to its hardware (#13216).
- `name@openagents.com` Lightning Addresses in forward mode (#13208) and for
  the iPhone wallet's own address, and tips that pay a maker's own address
  (#13210).
- The native apps' login decisions and request order, with no effects
  (`apps/shared/app-login.coax`, `apps/shared/app-login-flow.coax`, #13229).

Planned:

- The login, code entry and logout as shared app actions over that flow, on
  the web page and the iPhone screen.
- The account and session records as messages in `proto/`.

### The chapter's check

- **Source:** the shared login actions in `apps/shared/`, over
  `app-login-flow.coax`.
- **Trace:** chapter 6's login case, extended with `account.logout`, on the
  web and the iPhone: route, account step and session level after each
  operation.
- **Refusal:** a reply to the code request that arrives after logout is
  dropped, and the next login starts from an empty form.
- **Independent check:** the API's auth suite (`coax/web/api/test.sh`, on a
  build host) against the same request bodies.

It builds on what exists today:

- [`apps/shared/app-login.coax`](../../apps/shared/app-login.coax)
- [`apps/shared/app-login-flow.coax`](../../apps/shared/app-login-flow.coax)
- [`apps/openagents-ios/OpenAgents/SignIn`](../../apps/openagents-ios/OpenAgents/SignIn)

- [`docs/auth/identity-and-methods.md`](../../docs/auth/identity-and-methods.md)
- [`docs/auth/sessions-and-tokens.md`](../../docs/auth/sessions-and-tokens.md)
- [`proto/openagents/account/v1alpha1/agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/web/api/agent-policy.coax`](../../coax/web/api/agent-policy.coax)
- [`coax/web/api/auth-policy.coax`](../../coax/web/api/auth-policy.coax)
- [`coax/web/api/nip98.coax`](../../coax/web/api/nip98.coax)
- [`coax/lib/nostr/nipoa.coax`](../../coax/lib/nostr/nipoa.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Grant request, grant and agent key | `openagents.account.v1alpha1` `GrantRequest`, `Grant`, `SpendCaps`, `AgentKey` ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, as binary protobuf (`application/x-protobuf`) | In use: one generated codec, `coax/web/api/agent-wire.coax` |
| The owner's attestation of an agent key | `openagents.account.v1alpha1` `OwnerAttestation`; on the network it is an `auth` tag (NIP-OA) | HTTP inside the grant request; Nostr on the agent's events | In use in the API. NIP-OA is an external format |
| A signed request | A NIP-98 event | HTTP `Authorization` header | Built. External format |
| Account, authenticators, sessions, audit log | `openagents.account.v1` and `openagents.credential.v1` in the spec ([spec 15.1](../../docs/protocol/openagents-protocol-spec.md#151-openagentsaccountv1), [spec 15.2](../../docs/protocol/openagents-protocol-spec.md#152-openagentscredentialv1)) | HTTP JSON | Hand-written JSON. The spec's sketch predates this login, and the [protocol review](../../docs/protocol/2026-11-11-protocol-review.md) orders its rewrite as `v1alpha1` |

Planned paths: none. The shared login actions go in [`apps/shared`](../../apps/shared),
and server rules stay in [`coax/web/api/`](../../coax/web/api).

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

1. Do a member's grant (chapter 17) and an agent's grant become one record, or
   stay two?
2. Which scopes come after `profile:read` and `wallet:spend`?
