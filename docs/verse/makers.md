# Realm makers

Status: built October 30 (ev-claim-realm). Every realm can have a maker: the
person its tips go to. Orbit League is Topher's, Belt 2200 Ted's and Hold the
Stair Marc's (Episode 290).

## Claiming a realm

1. An admin makes a one-time code:
   `POST /api/admin/makers/<realm>/code` (the admin's cookie or a NIP-98
   signature; `coax/web/api/makers.sh code <realm>`). The answer holds the
   link, `https://openagents.com/realms/<realm>/claim#code=<32 hex>`. Only the
   code's SHA-256 is kept; the code itself is never logged.
2. The owner sends the link. The maker opens it on their phone. The page
   (`private.coax` page 4) logs in the way `/login` does: the 12-words account
   kept in this browser, made on the spot if there is none, or a session the
   phone app approved. The code leaves the address bar at once.
3. The maker types a display name and a Lightning address and taps CLAIM
   (`POST /api/playtest/makers/<realm>/claim {code, name, lightning}`). The
   code is spent in the same transaction that names the maker; it never works
   again. A later code for the same realm hands it to whoever claims with it.
4. The realm page says "Made by <name>" along its bottom (`host.js`, from
   `GET /api/playtest/makers/<realm>`).

The maker changes their name or address with
`POST /api/playtest/makers/<realm> {name?, lightning?}`, signed by the same key.

## What is kept

The maker's pubkey, the display name (1 to 40 characters, no control
characters, `|`, `<` or `>`) and the Lightning address (LUD-16, lowercased).
Nothing else. Anyone can read the name and the pubkey; only the maker reads
the address back. The page draws the name as text (Coax glyphs, `textContent`
on the realm page), never as HTML.

## Pieces

| Piece | Where |
| --- | --- |
| Rules and storage | `coax/web/api/migrations/0072_realm_makers.sql` (`makers.claim`, `makers.edit`, `makers.of`) |
| Routes | `coax/web/api/makers.c` |
| Checks | `coax/web/api/test_makers.py`, the claim `#test` in `apps/openagents-web/private.coax` |
| Pages | `apps/openagents-web/realms/<realm>/claim/index.html` (page 4 of `private.coax`) |
