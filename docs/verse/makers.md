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
4. The realm draws "MADE BY <NAME> · TIP" along its bottom. `host.js` reads
   `GET /api/playtest/makers/<realm>` and hands the name to the realm as
   message 608; the realm module draws it in Coax (Orbit League, Hold the
   Stair, Belt 2200). TIP, in Bitcoin orange, opens `/realms/<realm>/tip`.

The maker changes their name or address with
`POST /api/playtest/makers/<realm> {name?, lightning?}`, signed by the same key.

## What is kept

The maker's pubkey, the display name (1 to 40 characters, no control
characters, `|`, `<` or `>`) and the Lightning address (LUD-16, lowercased).
Nothing else. Anyone can read the name and the pubkey; only the maker reads
the address back. Pages and realms draw the name as Coax glyphs, never as HTML.

## Tips

`/realms/<realm>/tip` (`private.coax` page 5) asks for 100, 1,000 or 5,000
sats and shows our Spark wallet's invoice as a QR and a wallet link
(`POST /api/playtest/makers/<realm>/tip {sats}`); the page polls
`POST .../tip/<payment hash>` until it is paid. A paid tip is kept once in the
replay ledger and queued as a payout to the maker's Lightning address. The
first tip a maker gets carries a bonus from us (`OA_MAKER_FIRST_TIP_BONUS_SATS`,
1,000); if our wallet cannot cover it, the bonus waits as its own payout.

A payout keeps one invoice (quoted from the maker's address, fees included)
and is sent with an idempotency key; before any resend the wallet is asked
whether that invoice was already paid, so a maker is never paid twice. Failed
payouts back off (2^tries minutes, at most 6 hours); a day's payouts are capped
(`OA_MAKER_DAY_PAYOUT_SATS`, Chicago day). Admins: `POST
/api/admin/makers/<realm>/gift {sats}` pays a maker from our wallet, `GET
/api/admin/makers/payouts` lists payouts, `POST .../payouts/run` sends the due
ones. Tips by the team (admin or `oa_team` cookie) are flagged and left out of
usage numbers. Bitcoin only.

## Pieces

| Piece | Where |
| --- | --- |
| Rules and storage | `coax/web/api/migrations/0072_realm_makers.sql` (`makers.claim`, `makers.edit`, `makers.of`) |
| Routes | `coax/web/api/makers.c` |
| Checks | `coax/web/api/test_makers.py`, the claim `#test` in `apps/openagents-web/private.coax` |
| Tips and payouts | `coax/web/api/migrations/0073_maker_tips.sql`, `makers.c`, `coax/web/api/spark/wallet.cjs` (`/quote`, `/sent`, `/send`) |
| Checks | `coax/web/api/test_makers_tips.py`, the tip `#test`s in `private.coax`, the maker `#test` in each realm module |
| Pages | `apps/openagents-web/realms/<realm>/claim/index.html` (page 4), `.../tip/index.html` (page 5 of `private.coax`) |

## Fairy upgrades

Status: built October 31 (ev-asset-upgrades, Episode 290: "an upgrade button
next to my fairy ... the top three"). Someone makes a better fairy, lists it,
an admin approves it, and in Fairy Forest **UPGRADE** beside your fairy (U on
a computer) shows the top three. One tap buys one, with sats or with credit,
and its maker is paid like a tip. Fairies only for now.

- **A fairy** is a look code Fairy Forest draws: the nine parts of LOOK (body,
  skin, hair, style, eyes, outfit, wings, wing colour, glow), a digit each, and
  then, optionally, seven colours of six hex digits that LOOK's choices don't
  give (the hair, the bodice, the skirt, the gloves and under-skirt, the three
  wing pairs): 51 characters, e.g. `120140210f2c94c3a1f6b7b4fd6e8d8ff9be4ffc76fe0fff6c8`.
  `makers.look_of` and `ffl_code_read` (verse/fairy-forest/life.coax) check
  the same rules.
- **Listing**: a logged-in maker posts
  `POST /api/playtest/upgrades/fairy {name, by, look, sats, lightning}` (the
  site session or NIP-98): a name and their display name (1 to 40 characters,
  no control characters, `|`, `<` or `>`), the price (10 to 100,000 sats,
  `OA_UPGRADE_MIN_SATS`, `OA_UPGRADE_MAX_SATS`) and the Lightning address
  their share goes to. At most five wait at once. `makers.sh list-fairy
  ADDRESS [SATS] [NAME]` lists one made by OpenAgents with the admin's key.
- **Approval**: an admin opening UPGRADE sees WAITING FOR AN ADMIN with
  **APPROVE** and **REJECT** (`POST /api/admin/upgrades/<id>/approve|reject`;
  `GET /api/admin/upgrades` lists every listing as lines; `makers.sh upgrades`,
  `makers.sh approve ID`). A rejected listing leaves UPGRADE and the room at
  once; what was bought stays bought.
- **UPGRADE** (`GET /api/playtest/upgrades/fairy`, with the player key): the
  three approved fairies players bought most (our team's purchases don't count),
  then the newest; what you wear. **TRY** puts one on you while the pane is
  open. **BUY** (Bitcoin orange) asks our Spark wallet for an invoice for the
  price (`POST .../<id>/buy {"pay":"sats"}`) and shows it as a QR and
  **OPEN WALLET**; the pane asks `POST .../<id>/buy/<payment hash>` every
  3 seconds until our wallet's record shows it paid with a preimage that hashes
  to it. **CREDIT** takes the price from the logged-in key's inference credit
  (`inference.credit`, msat) in one transaction (`{"pay":"credit"}`; 401 not
  logged in, 402 not enough). Bought, you wear it; **WEAR** and **THE FAIRY YOU
  MADE** switch (`POST .../<id>/wear`, id 0 your own).
- **What others see**: the buyer is the playtest player (X-Verse-Player). The
  room's presence (`verse.room_seen`) carries `look` for each player wearing an
  approved fairy, and Fairy Forest draws every other fairy in their own bought
  look (`fairyroom.coax`); someone who bought nothing is drawn as before.
- **Paying the maker**: a purchase is a row of `makers.tips` with its `listing`,
  `buyer` and `paid_with` (`sats` or `credit`); paid, it queues a payout in
  the same transaction (`makers.payouts`, `source` 'upgrade') to the listing's
  Lightning address, sent by the same runner as tips (quote, keep the maker's
  invoice, send once, ask before any resend). A sats purchase goes through the
  replay ledger once. No first-tip bonus. The maker gets the price less the
  routing fee; OpenAgents keeps nothing.

| Piece | Where |
| --- | --- |
| Rules and storage | `coax/web/api/migrations/0077_fairy_upgrades.sql` (`makers.listings`, `makers.wearing`, the new columns) |
| Routes | `coax/web/api/makers.c` (`upgrades_route`) |
| Checks | `coax/web/api/test_upgrades.py` (the fake wallet), the UPGRADE `#test`s in `apps/openagents-web/fairyupgrade.coax` |
| Page | `apps/openagents-web/fairyupgrade.coax` (the button and pane), `fairy.coax` (`fairy_figure` draws a worn look), `fairy-face.c` (requests, the QR) |
