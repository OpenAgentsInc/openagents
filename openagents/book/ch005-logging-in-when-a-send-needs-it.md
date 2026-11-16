# 5. Logging in when a send needs it

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.
> The account service it uses runs today in `coax/web/api`, and the GPU
> login screen runs in the iOS Simulator
> ([#13645](https://github.com/OpenAgentsInc/openagents/issues/13645)).

## Premise

Chapter 4 sends a message, and the service answers only an account. This
chapter asks you to log in at the moment a send needs it: the login card
opens over the chat page, you log in with an email code, and your message
goes out. Tokens stay in the native auth adapter and never enter Coax.

## Draft

You can type before you log in. When you press Send with no session, the
app keeps your draft and opens the login card over the chat page. The card
owns input while it is open. When the login succeeds, the card closes,
focus returns to the composer, and the send you asked for goes out once.
When you close the card, nothing is sent and your draft stays.

The card is the GPU login screen, already built on the kit
([G1 as built](../../components/gpu-components.md#g1-as-built)):
[`apps/shared/login/screen.coax`](../../apps/shared/login/screen.coax) holds
its state, layout, paint and rows, email first and then the code, with the
form set in [`components/kit/form.coax`](../../components/kit/form.coax).
It records `login.email`, `login.send`, `login.code`, `login.verify`,
`login.resend` and `login.change`, and the same rows answer VoiceOver and
the driver. The words are "Log in". The code field takes one-time-code
AutoFill through the platform editor. In this chapter the card sits over
the chat page instead of covering the whole app.

The native auth adapter owns the session.
In [`AppAuth.swift`](../../apps/shared/host/apple/AppAuth.swift),
`emailStart` and `emailVerify` call the service, the tokens go to the
Keychain, and the app learns only whether you are logged in. No token, code
handle or pending request ID enters Coax, a trace or a log. A request the
app makes with your session asks the adapter to attach it.

Logging out ends everything the session started. A pending reply is
cancelled with chapter 4's rule, its late parts are dropped, and the
transcript and draft are cleared. A held reply can't appear in the next
person's screen.

The account behind the card is already built
([docs/auth](../../docs/auth/README.md)). An account is an opaque ID. An
email address, a Nostr key and a passkey attach to it in any combination,
and a Nostr key alone is enough. There are no passwords, and we never
collect a phone number. Levels decide what a session may do: an email code
gives L1; a passkey, a Nostr key or a recovery code gives L2; some actions
need an L2 login from the last five minutes. The server's rules are Coax in
[`coax/web/api/auth-policy.coax`](../../coax/web/api/auth-policy.coax).

An agent is an account too. It has an owner and its own way in: a Nostr key
that signs every request (NIP-98), or an `oa_key_` API key for tools that
can't sign. It never uses its owner's key, email or session. Its grant says
what it may do: scopes, sats a day, sats a payment, an expiry of 90 days by
default and a year at most, and the addresses it may call from. The owner
approves the grant at L2. An agent can never add or remove a way in, change
recovery, make or widen a grant, raise a cap, export a seed or delete an
account. The owner's key signs that the agent's key acts for it (NIP-OA).
Later chapters use this grant (chapters 17, 33 and 59); this chapter's app
shows none of it yet.

## What we build

Built in the repository today ([docs/auth](../../docs/auth/README.md)):

- Email-code and Nostr login, sessions and the audit log (#13200 to #13206).
- Agent accounts, keys, scopes and grants (#13211).
- Step-up, linking and the 72-hour recovery (#13212, #13213).
- Passkeys on the web and in the iPhone app (#13215), and device keys
  (#13216).
- The GPU login screen and its native side, checked in the iOS Simulator
  (#13645).

Planned:

- The login card over the chat page, opened by a send with no session, with
  the send held until the login ends.
- Logout, with the pending-turn fence.
- The card's physical-phone checks: VoiceOver, one-time-code AutoFill, an
  input method.

It builds on what exists today:

- [`apps/shared/login/screen.coax`](../../apps/shared/login/screen.coax)
- [`apps/shared/native/NativeLogin.swift`](../../apps/shared/native/NativeLogin.swift)
- [`apps/shared/host/apple/AppAuth.swift`](../../apps/shared/host/apple/AppAuth.swift)
- [`components/kit/form.coax`](../../components/kit/form.coax)
- [`docs/auth/identity-and-methods.md`](../../docs/auth/identity-and-methods.md)
- [`docs/auth/sessions-and-tokens.md`](../../docs/auth/sessions-and-tokens.md)
- [`coax/web/api/auth-policy.coax`](../../coax/web/api/auth-policy.coax)
- [`coax/web/api/agent-policy.coax`](../../coax/web/api/agent-policy.coax)
- [`coax/web/api/agent-wire.coax`](../../coax/web/api/agent-wire.coax)
- [`coax/web/api/nip98.coax`](../../coax/web/api/nip98.coax)
- [`coax/lib/nostr/nipoa.coax`](../../coax/lib/nostr/nipoa.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Grant request, grant and agent key | `openagents.account.v1alpha1` `GrantRequest`, `Grant`, `SpendCaps`, `AgentKey` ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, as binary protobuf (`application/x-protobuf`) | In use: one generated codec, `coax/web/api/agent-wire.coax` |
| The owner's attestation of an agent key | `openagents.account.v1alpha1` `OwnerAttestation`; on the network it is an `auth` tag (NIP-OA) | HTTP inside the grant request; Nostr on the agent's events | In use in the API. NIP-OA is an external format |
| A signed request | A NIP-98 event | HTTP `Authorization` header | Built. External format |
| Account, authenticators, sessions, audit log | `openagents.account.v1` and `openagents.credential.v1` in the spec ([spec 15.1](../../docs/protocol/openagents-protocol-spec.md#151-openagentsaccountv1), [spec 15.2](../../docs/protocol/openagents-protocol-spec.md#152-openagentscredentialv1)) | HTTP JSON | Hand-written JSON. The [protocol review](../../docs/protocol/2026-11-11-protocol-review.md) orders its rewrite as `v1alpha1` |

### The chapter's check

- **Source:** the chat page's login card and logout in the lean app, over
  `apps/shared/login/screen.coax`, and
  `openagents/examples/cases/login-card.ops`.
- **Trace:** `type composer.input hello`, `tap composer.send` with no
  session; `tree` shows `login.email`; the email and the code (`123456`)
  with their held answers released; `state session` prints `logged in`;
  `state transcript` prints `you: hello` once.
- **Refusal:** logout with a reply pending: each later `clock data release
  chat.part` answers `late part dropped`, and the transcript is empty. A
  wrong code leaves the card open with `That code isn't right.`
- **Independent check:** an XCUITest that logs in through the accessibility
  tree with no `coax drive`, and the API's auth suite on a build host
  against the same request bodies.

Planned paths: `openagents/examples/cases/login-card.ops`. Server rules
stay in [`coax/web/api/`](../../coax/web/api).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): reputation, identity and payments figured out once
  - [Episode 82, *Nostr Login*](../../docs/transcripts/082.md)
  - [Episode 207, *Your Keys, Your Coins, Your Identity*](../../docs/transcripts/207.md): one seed for a Nostr key and a wallet key
- **NIPs:** [NIP-98](../../nostr/nips/official/98.md), [NIP-OA](../../nostr/nips/block/NIP-OA.md), [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md)
- **Also:**
  - [OpenAgents authentication](../../docs/auth/README.md) and its [decisions](../../docs/auth/decisions.md)
  - [Nostr From Scratch, chapter 87](../../nostr/book/ch087-owner-attestation-and-agent-authentication.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P14

## Open questions for the owner

1. Does the lean app's login card include "Use a Nostr key" in this
   chapter, or only email until the key login is rebuilt on the kit?
2. Do a member's grant (chapter 17) and an agent's grant become one record,
   or stay two?
3. Which scopes come after `profile:read` and `wallet:spend`?
