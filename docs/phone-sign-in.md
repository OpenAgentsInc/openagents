# Signing in to openagents.com with the phone

The fast path of the m9-phone-sign-in milestone: the OpenAgents iOS app
holds a Nostr key and approves sign-ins with NIP-98 signed requests. Full
NIP-46 (the phone as a remote signer for the page) stays in the plan for
later (`apps/openagents-web/projects/coax/work/nostr.json`).

## The flow

1. `https://openagents.com/signin` (a Coax page) asks oa-api for a
   challenge: `POST /api/auth/challenge` gives `{challenge, poll, url}`. The
   QR holds `url`, `https://openagents.com/signin#c=<challenge>`. The poll
   token never leaves the page, so a photo of the QR can't take the session.
2. The phone scans it (or opens it as a link, or has it pasted), shows "Sign
   in to openagents.com?", and on Approve posts
   `POST /api/auth/approve` with body `{"challenge":"…","kind":"signin"}`
   and `Authorization: Nostr <kind 27235 event>` whose `u`, `method` and
   `payload` tags name that URL, POST and the body's SHA-256.
3. The page polls `POST /api/auth/poll {poll}` every two seconds. Once the
   phone approved, the poll answers once with the `oa_session` cookie
   (HttpOnly, Secure, SameSite=Lax, Path=/, 30 days) bound to the phone's
   pubkey. `GET /api/auth/me` gives the npub and the projects that key
   belongs to; `POST /api/auth/logout` ends it.

Challenges are good once, for five minutes. Only SHA-256 of challenges, poll
tokens and sessions is stored. Challenge, poll and invite values travel only
in URL fragments and request bodies, never in a path or query, so they don't
reach load-balancer logs, and oa-api logs only the operation and status.
Every POST is rate limited per address (challenge and approve 10 a minute,
poll 120), approvals also 30 a minute per key. The page's POSTs need this
site's `Origin`; the approval needs the signature instead.

## A phone for a private project

A member's phone key becomes another key of that member
(`projects.member_keys`, migration `0019_phone_signin.sql`), and then acts as
the member in every private-project route: signed requests, and a site
session from `/signin`, which opens the project page.

- **LINK A PHONE** on the project page, for a member signed in there: the
  page gets a challenge of kind `link` (`POST /projects/<slug>/api/link`),
  shows `https://openagents.com/signin#link=<challenge>` as a QR, and the
  phone approves it with `kind: "link"`.
- **An invite link** (`private.sh invite`) opened on the phone: the app posts
  `POST /api/private/<slug>/claim {"token":"…"}`, signed, which spends the
  invite and links the phone to the invited member.

A key that already has another place in the project (another member, an
agent, the runner, another member's phone) can't be linked. A linked key
stops working when its member leaves the document.

The public Planetary Villa project's `owner_pubkey` can later be set from
its owner's linked phone key; its key setup is unchanged for now.

## Pieces

| Piece | Where |
| --- | --- |
| Rules and storage | `coax/web/api/migrations/0019_phone_signin.sql` (`signin.api`, the `projects.api` wrapper) |
| Routes | `coax/web/api/private.c` (`auth_route`, `claim`, `link`) |
| Checks | `coax/web/api/test_private_phone.py` (run by `coax/web/api/test.sh`) |
| Page | `apps/openagents-web/private.coax` page 2 (`/signin`) and LINK A PHONE on page 0 |
| Phone | `apps/openagents-ios` (Keychain key, scanner, approval sheet) |
