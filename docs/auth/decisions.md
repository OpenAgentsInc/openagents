# Decisions

Status: decided, 2026-11-11. Part of [OpenAgents authentication](README.md).

These are the calls only the owner can make: product direction, money,
legal exposure, spending, and deleting named resources. The owner decided
all eleven on 2026-11-11:

> no SMS for now, i dont want to collect cell numbers at all. email and
> nostr good to start. yes good to pay peoples lightning addresses but lets
> ALSO offer @openagents.com lightning addresses and let people easily opt
> to get that from us. [...] rest of stuff is fine

Each section keeps the options that were weighed, so a later change can
see what it is changing. Changing a decision changes the docs named in
its section.

## D1. Offer OAuth login (GitHub, Google) at all?

| Option | Effect |
| --- | --- |
| A. No OAuth login anywhere; GitHub and Google are connections only | No Sign in with Apple needed; one fewer identity provider in the path |
| B. OAuth login on the web only | Same on iOS; the web gets "Log in with GitHub", which developers like, but it's a second recovery path through someone else's account |
| C. OAuth login everywhere | iOS must also ship Sign in with Apple (guideline 4.8), which brings Apple's private-relay emails and another provider |

**Recommendation: A.** The owner doesn't want third parties in the login
path, and the 2026-10-17 decision already made GitHub a connection.
Passkeys give the one-tap login people want from OAuth buttons. Affects
[identity](identity-and-methods.md#oauth-connections-only).

**Decided 2026-11-11: A.** No OAuth login; GitHub and Google are
connections.

## D2. Launch methods and the mobile default

| Option | Effect |
| --- | --- |
| A. Email code first, phone one tap away, passkey offered after | Free, global, keeps Apple's exemption; collects phone numbers |
| B. Phone first | Familiar for consumer apps; costs about a cent a login, limited to allowed countries, weak to SIM swap |
| C. No login screen; a key made silently (the 2026-10-17 design) | Fastest start; the account is only as recoverable as the device until the person adds a factor |
| D. Email code and Nostr, no phone | Free, global, no phone numbers held; people without email access use a Nostr key |

**Decided 2026-11-11: D.** The launch set is **email code and Nostr**, on
the web and in the apps. The app opens on email, with "Log in with a key"
(Nostr) beside it, and right after the first login offers a passkey once
passkeys ship. Passkeys, device keys and agent keys come later in the
[build order](build-order.md). Guests can still play before logging in.

## D3. SMS

| Option | Effect |
| --- | --- |
| A. SMS login, US and Canada, through Twilio Programmable Messaging | Most mature; we generate codes, they only deliver |
| B. Same through Telnyx | Usually cheaper; smaller ecosystem |
| C. No SMS at all | No cost, no pumping risk, no phone numbers held; people without email use a Nostr key |

**Decided 2026-11-11: C.** No SMS, and **we never collect phone
numbers**: not as a login, not for recovery, not for notices, not as
optional profile data. There is no `phone` authenticator, no phone table
and no SMS provider. Revisiting this is a new owner decision.

## D4. Custody model

| Option | Effect |
| --- | --- |
| A. Self-custody, seed on device only | No backup unless the person writes words down |
| B. Self-custody plus an encrypted backup we can't open (passkey PRF, recovery code) | One-tap restore with a synced passkey; we hold only ciphertext |
| C. Collaborative 2-of-3 with us holding one key | Recoverable with our help; we can block but not move funds |
| D. Custodial balances | Simplest UX; money transmission if withdrawable |

**Recommendation: B now, C as an option for large balances once our
wallet can run it, and D only as capped, non-withdrawable service
credit.** See [wallets](wallets-and-custody.md).

**Decided 2026-11-11: as recommended.**

## D5. One seed for login and wallet

| Option | Effect |
| --- | --- |
| A. One seed for everything (the 2026-10-17 design) | One backup; whoever has the words has the identity and the coins |
| B. Separate by default, one seed as an opt-in | The often-used Nostr key and the rarely-used wallet seed stay apart |
| C. Always separate | Two backups for people who want one |

**Recommendation: B for people; one seed by default for agents,** whose
coins are protected by ch122's treasury split rather than by the seed
alone.

**Decided 2026-11-11: as recommended.** One seed is an opt-in for people
and the default for agents.

## D6. Maker tips, Lightning Addresses and money transmission

Today a player pays our wallet and we pay the maker (`makers.tips`).

| Option | Effect |
| --- | --- |
| A. Keep it, get a lawyer's opinion first | Tips keep working; we know our exposure |
| B. Change tips to pay the maker's Lightning Address directly | We never hold the money; the first-paid bonus becomes a separate gift from us |
| C. Keep it as is | Possible unlicensed money transmission as volume grows |

**Decided 2026-11-11: B, plus our own addresses.** Tips and upgrade
shares pay the maker's own Lightning Address directly. We *also* offer
`name@openagents.com` Lightning Addresses that anyone can claim in one
tap, and a maker can use one as their payout address. The money for an
`@openagents.com` address lands in the person's own self-custodial
wallet, never in ours ([Lightning Addresses](lightning-addresses.md)). We
still get counsel's opinion before any custodial feature ships.

## D7. Retire WorkOS

Nothing logs in with WorkOS today. These remain:

- the WorkOS staging and production environments in the WorkOS dashboard;
- Secret Manager secrets `openagents-web-workos-api-key`,
  `openagents-web-workos-client-id`, `openagents-web-workos-redirect-url`,
  `openagents-web-workos-staging-api-key` and
  `openagents-web-workos-staging-client-id`.

**Recommendation:** delete all of them after step 3 of the
[build order](build-order.md) removes the code that reads them.

**Decided 2026-11-11: delete them.** The owner's yes covers the two
environments and the five secrets named above, and nothing else.

## D8. Accounts with only a Nostr key

| Option | Effect |
| --- | --- |
| A. Allowed, same limits as everyone, with "this key is your only way in" shown at creation | Pseudonymous and agent-friendly, true to Nostr first |
| B. Require an email too | Recoverable, but no longer pseudonymous |

**Recommendation: A.** Free-allowance abuse is handled by per-account
limits and proof of work, not by demanding an email.

**Decided 2026-11-11: A.**

## D9. IAM: take the production database password away from agents

Today the automation service account can read the API's connection
string, and that role owns the schemas, so any agent on that account can
drop production tables. The account can't change IAM itself.

**Recommendation: yes.** The owner (as `chris@`) removes its access to
the production connection-string secrets, gives it the read-only one, and
creates the `openagents-backups` project where it has no role. Exact
commands go in `NEEDS_OWNER.md` when step 0 starts.

**Decided 2026-11-11: yes.**

## D10. Recovery when every strong factor is lost

| Option | Effect |
| --- | --- |
| A. 72-hour wait, cancellable from any channel, no staff override | Slow for the real owner; someone who took over the inbox loses to the real owner's passkey or Nostr key |
| B. 24-hour wait | Faster; less time to notice |
| C. Staff-assisted recovery | Friendly; makes our support team the weakest factor |

**Decided 2026-11-11: A.**

## D11. High availability for the database

| Option | Monthly |
| --- | --- |
| A. Stay zonal; rely on PITR, retained backups and locked dumps | About $5 added |
| B. Regional (HA) | About +$50 |
| C. Cross-region replica | About +$50 |

**Recommendation: A** until we have users whose sessions an hour's
outage would hurt. B and C buy uptime, not protection against an agent's
`DROP`, which replicates instantly.

**Decided 2026-11-11: A.**
