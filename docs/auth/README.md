# OpenAgents authentication

Status: design, 2026-11-11. Docs only: nothing here is built yet.

This is a greenfield design. We have about zero users, so we replace
today's login code instead of migrating it: the words account, the phone
QR sessions, `verse.accounts`, `signin.*`, `auth.identities` and the
WorkOS routes are deleted when the new system lands
([build order](build-order.md)). The invite gate in
[realm invites and accounts](2026-10-31-realm-invites-and-auth.md) §4
moves onto the new accounts; the rest of that doc is superseded.

## What the owner asked for

> We need to start designing our own authentication system, docs only for
> now. [...] I just generally don't want to outsource our security or let
> other third parties have our user table. We want to keep good track of
> that ourselves with all needed backups and stuff. [...] We also want
> first-class support for Nostr using our nostr lib. People or agents need
> to be able to auth with any combination of these things. We need
> appropriate backups in case some agent like accidentally nukes our
> production database [...] And we need to think about this working in
> conjunction with our plans for Bitcoin.

## The design in one page

- **One account, many authenticators.** An account is an opaque id in our
  own Postgres. Email, phone, passkeys, Nostr keys, device keys and agent
  keys attach to it, in any combination. Any one strong authenticator logs
  you in.
- **No passwords.** We never store one, so there is nothing to stuff and
  no hash to crack.
- **No third-party login on the apps.** GitHub and Google are
  *connections* (repository access, files), not ways in. Because no app
  uses a third-party login for the primary account, Apple's guideline 4.8
  does not require Sign in with Apple.
- **Mobile default: email code, then phone.** The app then offers a
  passkey, and makes a hardware-bound device key in the background.
- **Nostr is first class.** NIP-98 signed requests, NIP-07 in the browser,
  NIP-46 remote signers and the phone-approves-the-web QR, all on our own
  [nostr library](../../coax/lib/nostr/).
- **Agents get their own keys, not their owner's.** An agent is an account
  owned by a person. Its Nostr key or API key carries a grant: scopes,
  spend caps and an expiry. An agent can never do a human-only action.
- **Sessions are opaque tokens we can revoke at once.** No JWTs. Native
  refresh tokens are bound to a non-exportable device key.
- **Backups an agent can't delete.** Point-in-time recovery, plus logical
  dumps into a bucket with a locked retention policy in a separate project,
  plus database roles that can't drop tables, plus a monthly restore drill.
- **Wallets stay self-custodial.** We store encrypted seed backups we can't
  open. Custodial balances stay small, non-withdrawable service credit.

## Principles

1. **We own the user table.** Accounts, authenticators, sessions and the
   audit log live in our Cloud SQL Postgres, in our GCP project. No
   identity provider holds a copy.
2. **No third party holds identities or sessions.** Outside services only
   deliver messages (email, SMS, push). They see an address at the moment
   of delivery and nothing else. Each one sits behind a small interface so
   it can be swapped (see [build order](build-order.md#outside-services-we-cant-avoid)).
3. **Every secret is a digest or ciphertext at rest.** Tokens and codes are
   stored as SHA-256. Emails and phone numbers are encrypted, with a keyed
   hash for lookup.
4. **Agent-safe by construction.** Agents can't log in as people, can't
   raise their own limits, can't drop production tables and can't delete
   backups. The database and IAM enforce this, not good behaviour.
5. **Greenfield, built right once.** No compatibility shims for today's
   login code. We build the full system, then delete the old one.
6. **Plain and honest UI.** "Log in", never "Sign in"; the path is
   `/login`. Every limit is shown before you hit it. No walls of text.
7. **Built by us.** The crypto and protocol code is Coax or the API's C,
   checked against published vectors. Outside code is limited to the
   unavoidable platform APIs (WebAuthn, Keychain, Keystore).

## Why we're not using WorkOS

The previous plan made a WorkOS User the auth object for a person
(`docs/auth/2026-10-20-workos-source-of-truth.md`, deleted in the nuke
`0b401561a0`; read it with `git show 0b401561a0^:docs/auth/2026-10-20-workos-source-of-truth.md`).
The deleted Rust `coder` service ran it in production from 2026-10-21 until
the service was removed on 2026-10-28
([coder deprecation](../2026-10-28-coder-service-deprecation.md)). In the
Coax site, the WorkOS start route exists behind the `auth_v2` flag, but the
code exchange was never built, so no one logs in with WorkOS today.

WorkOS reported unauthorized access to an internal sales and support
application, found on 2026-10-30. Its bulletin lists exposed CRM contact
details, account histories, call and Slack summaries, and metadata about
customers' product usage
([WorkOS bulletin](https://workos.com/security/bulletins/october-30th-2026-data-security-incident),
[update](https://workos.com/security/bulletins/update-for-october-30th-2026-data-security-incident)).
Press reports say WorkOS found no access to production app data or
authentication infrastructure, and attribute the access to an autonomous AI
penetration-testing agent
([BleepingComputer](https://bleepingcomputer.com/news/security/runaway-ai-hacking-agent-behind-workos-breach-after-escaping-sandbox)).
We have not verified those details beyond these sources.

The breach did not reach auth data, but it makes the owner's point: any
vendor that holds our user table is one more company whose internal tools
can leak it. We already have the hard parts (Schnorr, NIP-98, AES-GCM,
scrypt, BIP-39/32, sessions, Resend). Passkeys and SMS are the only real
additions.

## History we build on

| When | Design | Where |
| --- | --- | --- |
| 2025 | Better Auth in the old website: "keeps user data within our chosen database" | `git show acc72ba4ca^:apps/projects/docs/better-auth-integration.md` |
| 2026-02 | WorkOS Magic Auth (email code) on our own `/login`, not the hosted page | `git show 388473626c^:apps/web/docs/authkit-effect-service-plan.md`; `~/work/autopilot3/docs/2026-05-13-workos-first-party-auth-audit.md` |
| 2026-06 | OpenAuth for Khala chat and Pylon ownership | `git show d613b8ea22^:apps/openagents.com/docs/chat/2026-06-28-khala-chat-openauth-pylon-ownership-audit.md` |
| 2026-10-08 | "Own it ourselves, in Rust; no third-party auth service in the path" | `git show 0b401561a0^:docs/auth/README.md` |
| 2026-10-17 | Nostr-only identity: 12 words, no sign-in screen, Apple removed, GitHub a connection | `git show 0b401561a0^:docs/audits/2026-10-17-nostr-only-identity.md` |
| 2026-10-20 | WorkOS as the auth object, Nostr keys linked | `git show 0b401561a0^:docs/auth/2026-10-20-workos-source-of-truth.md` |
| 2026-10-30 | Words account and passkey-PRF login on the Coax site | [web-log-in.md](../web-log-in.md), [phone-sign-in.md](../phone-sign-in.md) |
| 2026-10-31 | Realm invites, `auth.identities`, CLI device approval | [realm invites and accounts](2026-10-31-realm-invites-and-auth.md) |

Lessons we keep from that history:

- Sign in with Apple was built and removed (#11980, #12005) once no app
  used a third-party login. This design keeps it that way.
- The account must be an opaque id with many principals, so removing a key
  or a provider never moves data (2026-10-17 audit).
- Nine stores held identity truth before the nuke. This design has one:
  the `auth` schema.
- Handing the 12 words to every linked computer made every computer a
  holder of the person's identity and wallet
  (`git show 0b401561a0^:docs/audits/2026-10-17-spark-wallet.md`). Devices
  get their own keys here.
- A session minted from a key sign-in must still be revocable from the
  server. Per-request NIP-98 stays for agents and tools.

## Documents

| Doc | What it covers |
| --- | --- |
| [Identity and methods](identity-and-methods.md) | Accounts, every authenticator, linking, recovery, merging, step-up, per-platform flows, agents |
| [Sessions and tokens](sessions-and-tokens.md) | Session model, cookies and native storage, token formats, middleware, rate limits, threat model |
| [Data, storage and backups](data-and-backups.md) | Schema, encryption at rest, audit log, backups, database roles, the "agent nukes prod" runbook, export and deletion |
| [Wallets and custody](wallets-and-custody.md) | Self-custody, encrypted backups, custodial credit, one seed, agent treasuries, spend step-up, regulation |
| [Build order](build-order.md) | What to build first, the crypto we must write in Coax, the outside services we can't avoid, and the old code to delete |
| [Decisions](decisions.md) | Every open owner decision, with options and a recommendation |
| [Realm invites and accounts](2026-10-31-realm-invites-and-auth.md) | The invite gate (kept, moved onto new accounts); its account model is superseded |

## Owner decisions

Each one is argued in [decisions.md](decisions.md). The recommendation is
what this design assumes.

| # | Decision | Recommendation |
| --- | --- | --- |
| D1 | Offer OAuth login (GitHub, Google) at all? | No. Connections only, so no Sign in with Apple |
| D2 | Mobile default | Email code first, phone code second, passkey offered after |
| D3 | SMS: offer it, and which provider | Yes, US and Canada first, through a delivery-only provider behind our own codes |
| D4 | Custody model | Self-custody with encrypted backups we can't open; custodial only as non-withdrawable credit |
| D5 | One seed for login and wallet | Not by default for people; opt-in. Default for agents, with the ch122 treasury split |
| D6 | Money-transmission review of maker tips | Get counsel before the tip flow scales |
| D7 | Retire WorkOS | Delete the WorkOS environments and five named secrets |
| D8 | Nostr-key-only accounts (no email or phone) | Allowed, with the same limits as everyone, shown up front |
| D9 | IAM: take the prod database password away from the automation account | Yes, and add a separate backup project |
| D10 | Recovery delay when every authenticator is lost | 72 hours, cancellable from any channel; no staff override |
| D11 | High availability for the database | Not now; locked backups and drills first |
