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
   A realm nobody has claimed answers 200 `{realm, state: "unclaimed"}`
   (migrations/0084); its tip page says so and links back to the realm.

The maker changes their name or address with
`POST /api/playtest/makers/<realm> {name?, lightning?}`, signed by the same key.

## What is kept

The maker's pubkey, the display name (1 to 40 characters, no control
characters, `|`, `<` or `>`) and the Lightning address (LUD-16, lowercased).
Nothing else. Anyone can read the name and the pubkey; only the maker reads
the address back. Pages and realms draw the name as Coax glyphs, never as HTML.

## Tips

Status: since November 11 a tip goes from the player's wallet straight to
the maker's own Lightning Address (owner decision D6, GitHub #13210). It
never enters our wallet.

`/realms/<realm>/tip` (`private.coax` page 5) asks for 100, 1,000 or 5,000
sats (`POST /api/playtest/makers/<realm>/tip {sats}`). Our server asks the
maker's Lightning Address for an invoice, so the player's network address
never reaches the maker's wallet service. It checks the invoice before the
page shows it: the exact amount, the description hash of the service's own
metadata, the expiry, and that the maker's address is still the one it
asked. The page shows the invoice as a QR and a wallet link. A wallet in
the browser (WebLN: the app's wallet, an extension) pays it in place.

A tip counts only when it is confirmed:

- The maker's wallet service says it is settled, with the preimage (LUD-21
  `verify`). The page asks `POST .../tip/<payment hash>` every 3 seconds.
- Or the paying wallet reports the preimage
  (`POST .../tip/<payment hash> {preimage}`).

Either way SHA-256 of the preimage must be the invoice's payment hash. The
page then says **PAID**. A confirmed tip is kept once in the replay ledger;
confirming it again changes nothing. When the maker's service has no
`verify` and the wallet reports nothing, the player taps **I PAID** and the
page says **SENT FROM YOUR WALLET**. Nothing is counted.

The first confirmed tip a maker gets carries a bonus from us
(`OA_MAKER_FIRST_TIP_BONUS_SATS`, 1,000). The bonus is its own payment from
our wallet to the maker's address: our money, not the player's.

Our wallet sends only our own money: a bonus, or an admin's gift
(`POST /api/admin/makers/<realm>/gift {sats}`). A payout keeps one invoice
(quoted from the maker's address, fees included) and is sent with an
idempotency key; before any resend the wallet is asked whether that invoice
was already paid, so a maker is never paid twice. Failed payouts back off
(2^tries minutes, at most 6 hours); a day's payouts are capped
(`OA_MAKER_DAY_PAYOUT_SATS`, Chicago day). `GET /api/admin/makers/payouts`
lists payouts with what each is (`bonus`, `gift`, or `tip` and `upgrade`
for money paid into our wallet before November 11); `POST .../payouts/run`
sends the due ones. Tips by the team (admin or `oa_team` cookie) are flagged
and left out of usage numbers. Bitcoin only.

Before November 11 a player paid our wallet's invoice and a runner paid the
maker. Those rows stay in `makers.tips` and `makers.payouts`
(`direct = false`); a payout of theirs not yet sent is still sent, and
nothing new is made that way.

### Limits

| Limit | Value |
| --- | --- |
| A tip | 10 to 100,000 sats (`OA_TIP_MIN_SATS`, `OA_TIP_MAX_SATS`), within what the maker's service takes |
| Invoices asked of makers' addresses | 60 a minute, all players |
| The maker's service's answer | 5 seconds, 64 KB, no redirects |
| An invoice's life | at least 1 minute left when shown, at most a day |

### Threats and their checks

| Threat | What stops it | Check |
| --- | --- | --- |
| A swapped invoice | The page shows only the invoice our server checked: its signature recovers, its description hash commits to the maker's service's own metadata, and a payment hash seen before is refused | `lnurl.coax` "a forwarded invoice"; `test_makers_tips.py` "an invoice seen before (a swapped one)", "another description hash" |
| A wrong amount | The invoice must be for exactly the msat asked | `lnurl.coax` `lu_check_invoice`; `test_makers_tips.py` "a wrong amount" |
| A fake preimage | SHA-256 of the preimage must be the payment hash, in Coax (`mp_proof`) and again in SQL (`makers.direct_confirm`, the `tips_direct_proof` constraint) | `makerpay-policy.coax` "a preimage proves a payment"; `test_makers_tips.py` "a fake preimage", "another payment's preimage" |
| A service that lies about settlement | LUD-21 `settled` without a matching preimage reads as not settled | `lnurl.coax` `lu_verify_read`; `test_makers_tips.py` "a service that says settled" |
| SSRF through a maker's address domain | https only; the host must be a public DNS name (no IP literal, `localhost`, `.local`, `.internal`, `.onion`); every address connected to must be public (private, loopback, link-local and metadata ranges refused at connect time, so a name that resolves to one is refused); no redirects; no proxy; 5 seconds; 64 KB | `lnurl.coax` `lu_url_ok`, `lu_ip_public`; `test_makers_tips.py` "is never fetched", "a redirect", "an answer over 64 KB"; staging: a maker at `localtest.me` (resolves to 127.0.0.1) |
| A replayed confirmation | A paid row stays paid and counts once; the payment hash goes through `inference.used_payments` once; one bonus a maker, under a lock | `makerpay-policy.coax` "a check: it counts once"; `test_makers_tips.py` "confirmed again", "already in the replay ledger" |
| A maker changing address mid-payment | The invoice is kept only while the maker's address is the one it came from; otherwise the page asks again and gets the new address's invoice | `makerpay-policy.coax` "showing the invoice"; `test_makers_tips.py` "the maker changed address" |
| The player's network address reaching the maker | Our server makes the request with its own headers and nothing of the player's | `test_makers_tips.py` "it saw our server and nothing of the player" |
| Player money in our wallet | No invoice of ours is made for a tip or purchase; a direct payment can't have a payout of the player's money (`makers.payout_wall`) | `makerpay-policy.coax` "our wallet pays out only our own money"; `test_makers_tips.py` "our wallet made no invoice", "can never get a payout" |

## Pieces

| Piece | Where |
| --- | --- |
| Rules and storage | `coax/web/api/migrations/0072_realm_makers.sql` (`makers.claim`, `makers.edit`, `makers.of`) |
| Routes | `coax/web/api/makers.c` |
| Checks | `coax/web/api/test_makers.py`, the claim `#test` in `apps/openagents-web/private.coax` |
| Tips | Rules: `pay/makerpay-policy.coax` (`#authority`, no surviving mutants). The maker's invoice: `coax/web/api/lnaddr.c` (`lnaddr_invoice_of`, `lnaddr_settled`) on `coax/lib/lnaddr/lnurl.coax`. Rows: `coax/web/api/migrations/0073_maker_tips.sql`, `0154_maker_direct_pay.sql`. Routes: `makers.c` |
| Bonus and gift payouts | `makers.c` (`run_payouts`), `coax/web/api/spark/wallet.cjs` (`/quote`, `/sent`, `/send`) |
| Checks | `coax/web/api/test_makers_tips.py` (fake Lightning Address services with signed BOLT 11 invoices), the tip `#test`s in `private.coax`, the maker `#test` in each realm module |
| Pages | `apps/openagents-web/realms/<realm>/claim/index.html` (page 4), `.../tip/index.html` (page 5 of `private.coax`) |

## Fairy upgrades

Status: built October 31 (ev-asset-upgrades, Episode 290: "an upgrade button
next to my fairy ... the top three"). Someone makes a better fairy, lists it,
an admin approves it, and in Fairy Forest **UPGRADE** beside your fairy (U on
a computer) shows the top three. One tap buys one with sats, paid from the
buyer's wallet straight to its maker's Lightning Address (owner decision D6,
#13210). Fairies only for now.

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
  open. **BUY** (Bitcoin orange) asks for the maker's own invoice for the
  price (`POST .../<id>/buy {"pay":"sats"}`), checked as a tip's is, and shows
  it as a QR and **OPEN WALLET**; the pane asks `POST .../<id>/buy/<payment
  hash>` every 3 seconds. It is yours once the payment is confirmed: the
  maker's wallet service says settled with the preimage (LUD-21), or the
  paying wallet reports the preimage (`{"preimage"}`). An upgrade is delivered
  only when confirmed, so one is offered only where a confirmation can come:
  a listing whose maker's service has no LUD-21 answers 422 unless the wallet
  reports payments (`{"pay":"sats","reports":true}`). Service credit never
  buys an upgrade (`{"pay":"credit"}` answers 400): credit pays only for our
  own services. Bought, you wear it; **WEAR** and **THE FAIRY YOU MADE**
  switch (`POST .../<id>/wear`, id 0 your own).
- **What others see**: the buyer is the playtest player (X-Verse-Player). The
  room's presence (`verse.room_seen`) carries `look` for each player wearing an
  approved fairy, and Fairy Forest draws every other fairy in their own bought
  look (`fairyroom.coax`); someone who bought nothing is drawn as before.
- **Paying the maker**: a purchase is a row of `makers.tips` with its `listing`
  and `buyer`, `direct` and, once confirmed, `confirmed_by` and the preimage.
  The price goes to the listing's Lightning Address in the buyer's own
  payment; our wallet is not part of it and there is no payout. A confirmed
  purchase goes through the replay ledger once. No first-tip bonus.
  OpenAgents keeps nothing. Purchases made before November 11 (paid to our
  wallet, or with credit) keep their rows and what was bought stays bought.

| Piece | Where |
| --- | --- |
| Rules and storage | `pay/makerpay-policy.coax` (paying), `coax/web/api/migrations/0077_fairy_upgrades.sql` (`makers.listings`, `makers.wearing`), `0154_maker_direct_pay.sql` |
| Routes | `coax/web/api/makers.c` (`upgrades_route`) |
| Checks | `coax/web/api/test_upgrades.py` (the fake Lightning Address services), the UPGRADE `#test`s in `apps/openagents-web/fairyupgrade.coax` |
| Page | `apps/openagents-web/fairyupgrade.coax` (the button and pane), `fairy.coax` (`fairy_figure` draws a worn look), `fairy-face.c` (requests, the QR) |
