# Making money from the Verse and Aethyr

Status: plan, 2026-11-06. Nothing here is built unless the "What we have" table
says so. Prices and the platform fee are proposals for the owner to decide
(listed at the end and in `NEEDS_OWNER.md`).

Source: Tim Fields, *Game Development 2042: The Future of Game Design,
Development, and Publishing* (CRC Press, 2022/23), read against this repository.
Chapter 10 (pp. 269–281: "The Act of Creation", "Crowdsourcing and NFT Sales",
the interview with Mauricio Longoni of Aquiris about Wonderbox) is the part that
fits us best. Chapters 3, 4, 7, 8 and 9 (game design, Web3, business models,
data, AI characters) fill in the rest.

## In one paragraph

We are already building what the book calls the future of games: a place where
"everyone is a game maker if they want to be" (p. 271), with an engine
(Aethyr) and a maker (Aethyrforge) in the same place people play. We already
move real sats: tips, fairy upgrades and paid change offers. What we don't do yet
is **keep any of it**: fairy upgrades go to the maker in full ("OpenAgents keeps
nothing", [makers.md](makers.md)). The plan is to keep play and making free,
take a small, stated share of what makers sell, sell a few things of our own
(scarce items, AI companions, hosting, agents), and replace NFTs with
**editions**: items with a supply that can be proven fixed, owned by a Nostr
key and anchored to Bitcoin.

## What the book says that matters to us

1. **Everyone becomes a maker, and most games make no money** (pp. 270–272).
   Millions of games came out between 2015 and 2020. Making games for profit is
   "a bloody Darwinian fight", and most games never earn back their costs. People
   make them anyway. *For us:* the platform that hosts the makers earns
   reliably even when single games don't. Our income should come from activity
   across all realms, not from picking hits.
2. **Make an adventure maker, not an "anything maker"** (Longoni, p. 275).
   Wonderbox kept the toolbox small so players understood what they could
   make. FPS and racing games were left out on purpose because they would have
   needed programming. *For us:* Aethyrforge's catalogue (20 features, 4 cameras,
   3 styles) is the right shape. Grow it by genre template (adventure, tower
   defence, arena) rather than toward a general engine UI. Agents cover the
   rest ("Asked for, not built yet" in [aethyrforge.md](aethyrforge.md)).
3. **Close the gap between players and makers, on one platform with familiar
   rules** (pp. 274–275). When a player finishes an adventure they don't wait
   3–4 years for a sequel: they play someone else's, or make their own.
   *For us:* PLAY → FORK → PUBLISH is already one loop ([realm-fork.md](realm-fork.md)).
   Every realm should show its maker and a FORK button. Every fork should credit
   the realm it came from, and share revenue with it (below).
4. **Make on the phone first** (pp. 275–276). "The phone is the device you use
   the most." *For us:* Aethyrforge's MVP is desktop-shaped. The touch layout
   and voice editing (HOLD TO TALK already edits realms) are how most of the
   world will make things. This matters more than any desktop feature.
5. **Content is king, and copies don't last** (p. 274). *For us:* this matches
   our rule against recreating existing games. Original realms are the product.
6. **Makers can come from anywhere** (pp. 276–277). Longoni: a Brazilian studio
   with quality work competes worldwide, and talent trained locally comes back
   to build its own companies. *For us:* Lightning pays a maker in Porto Alegre,
   Lagos or Jakarta the same day, with no bank and no app store. Card payments
   and app stores can't, and Chapter 7's sections on carrier billing and
   regional payment systems are workarounds for that same problem.
7. **Fans fund games before they exist** (pp. 272–273): crowdfunding since
   2009, then NFTs of items in games not yet built, which the book itself
   calls "an unregulated way of buying shares in a promising game concept".
   *For us:* keep the patronage and drop the shares (see "Editions" and
   "Patronage" below).
8. **Small purchases and regional prices** (Chapter 7, "Regional and Sachet
   Pricing"). *For us:* sats are naturally fine-grained. A 21-sat item is a
   real price.
9. **AI characters and companions** (Chapter 9). *For us:* speaking characters,
   `aethyr/talk/` and the voice engine already exist. A companion that remembers
   you is something people pay for again and again.
10. **Using games to train AI, data ownership, bots and ID** (Chapter 8, "On
    Bots and ID"). *For us:* deterministic replay (`aethyr/replay.coax`) makes
    realms good test grounds for agents, and Nostr keys tie each player to an
    identity. Both protect earning against bots.

## What we have

| Piece | State | Where |
| --- | --- | --- |
| Realm makers, claim codes, a maker's Lightning address | Built | [makers.md](makers.md), migrations 0072, 0084 |
| Tips (100 / 1,000 / 5,000 sats), first-tip bonus, payouts with idempotency | Built | migration 0073, `makers.c`, `coax/web/api/spark/wallet.cjs` |
| Fairy upgrades: list, approve, TRY, BUY with sats or inference credit; the maker gets everything less routing | Built, fairies only | migration 0077, `fairyupgrade.coax` |
| Fork with rules, publish, revisions, pinned engine build | Built | [realm-fork.md](realm-fork.md), [aethyrforge.md](aethyrforge.md), `aethyr/forge/publish.coax` |
| Paid change offers on a realm (`ChangeOffer.price_msat`, up to 1,000,000 sats) | In the protocol | [openagents-verse-protocol.md](openagents-verse-protocol.md) `verse.economy.v1` |
| Voice free tier and per-msat pricing | Built, `#authority` | `verse/voice-meter.coax` |
| Agents seated in realms | Slices 1–5 built, hosted seats not deployed | [realm-seat.md](realm-seat.md) |
| Agent labor market, paid after acceptance | Draft NIP | `nostr/nips/openagents/NIP-MKT.md`, `NIP-LAB.md` |
| Realm analytics (visitors, visits, minutes) | Built | [analytics-events.md](analytics-events.md), `GET /api/analytics/realms` |
| Phone wallet (Spark), Nostr key from the same 12 words | Built | [docs/lightning/](../lightning/README.md) |

So we have payments, makers, listings and payouts. What's missing is mostly policy
(a fee, generalized listings) and one new primitive (editions).

## Rules for all of this

- **Playing and making stay free.** The book's "the world wants to play for
  free" (Chapter 3) is right, and free making is what fills the platform.
- **Bitcoin only**: Lightning, on-chain, Spark, Taproot Assets. No tokens of our
  own, no stablecoins, no other chains.
- **Nothing sold changes the game's balance.** Purchases are looks, editions,
  services and capacity, never power.
- **No paid chance.** No loot boxes, gacha or paid random drops. These are
  being regulated in more and more countries, and they work against trust. Chapter 3's
  "Gacha, near miss, FOMO" is a list of things we won't do. Free random drops,
  earned by playing, are fine.
- **No wagering** (Chapter 7, "Real Money Gambling"). Paid-entry tournaments with
  prizes in sats are contest or gambling law in most places, so they need legal
  review before they happen at all.
- **No investment promises.** We never sell a share of a realm's future
  income, and we never market an item by its resale price.
- **Every rule about money goes in Coax, marked `#authority`, and gets
  `coax mutate` with zero survivors.** That includes fees, splits, caps,
  royalties and edition supply. Servers and hosts carry it out.
- **Protocol first.** New economy records go into `proto/` and the Verse
  protocol as new versions (`verse.economy.v2` or new messages), never as
  changes in place.

## Revenue lines

Ordered by how soon each could earn. Each says what to build.

### 1. A platform share on maker sales (now)

Today a fairy upgrade pays the maker everything except routing. Proposal:
**OpenAgents keeps 10%, the maker gets 90%**, shown on the BUY sheet
("90% TO <MAKER>"). For comparison, Steam keeps 30%, the app stores 15–30%,
and itch.io lets the maker choose (10% by default). Roblox's makers keep
much less once its currency exchange is counted. A low, stated share is
an advantage we can say out loud.

- Tips stay 100% to the maker. They're gifts, and taking a cut of gifts looks petty.
- Build: `verse/split.coax` (`#authority`): the price, the fee in basis points,
  the routing fee and the fork-ancestry share (line 6) in, each party's
  millisats out. Totals are checked, and rounding always goes to the maker.
  `makers.c` calls it instead of doing the arithmetic itself. The payout
  runner doesn't change.
- Decide: the fee, and whether credit purchases pay the same.

### 2. Listings for every look, not just fairies (now → next)

Generalize `makers.listings` from `fairy` to item kinds a realm declares:
outfits, emotes, a realm's decorations, title plates, music loops, and later
characters and voices. Each realm module says which kinds it can draw, the
way Fairy Forest checks a look code today (`ffl_code_read`). Keep admin
approval at first, then trusted makers can list without waiting.

- Prices from 10 sats (sachet pricing) up to 100,000.
- The UPGRADE pane becomes a SHOP pane per realm, with the same top three,
  TRY, BUY and WEAR.

### 3. Editions: provably scarce items without NFTs (next)

The book's case for NFTs is ownership a game can't quietly take back,
supply that's visible to everyone, and items that last longer than one
game. Those are good goals. The NFT version came with a token per item on
a chain that isn't Bitcoin, buyers betting on resale, and marketplaces with
no ties to the games. We can meet the goals with keys we already have and
with Bitcoin.

**An edition** is a run of an item with a fixed cap: "Moonlit Marsh Lantern,
1 of 100". It has four parts:

1. **The class**: a Nostr event signed by the maker's key (BIP-340 Schnorr,
   which `coax/lib/crypto/` has) naming the item, the realm, the asset's
   SHA-256 (as `assets.coax` names every asset), the cap and the royalty. Its
   id is the hash of the event, so the cap can't be changed without a new
   class.
2. **A mint**: serial `n` of the class, issued to the buyer's Nostr pubkey
   when payment settles, and recorded with the payment's settlement reference
   (never the invoice or a key, per the protocol).
3. **A transfer**: signed by the current owner's key, naming the class, the
   serial and the new owner, with a sequence number. Gifts are free. A resale
   pays the maker's royalty.
4. **An anchor**: once a day we commit the Merkle root of every class, mint
   and transfer to Bitcoin in one transaction: a Taproot key tweak
   (pay-to-contract) on one of our own outputs, or OP_RETURN as a fallback.
   A cheaper first step is an OpenTimestamps attestation of the root as a
   NIP-03 event (`coax/lib/nostr/nip03.coax` exists): free and still
   checkable against Bitcoin, though it relies on the OpenTimestamps
   calendars and confirms in hours rather than one block.
   After that, anyone holding the published log can check three things: no
   class ever had more serials than its cap, every transfer was signed by
   the owner at the time, and we haven't rewritten history since the anchor.

What this proves and what it doesn't: the anchor proves supply and history.
It doesn't stop us from refusing a transfer before the next anchor. Until
there's an on-chain or federated order, we're the one who orders transfers,
and the doc and the page say so plainly. That's the same trust a player
already places in us for the realm itself, and it is far more checkable than
a typical game's database.

**Taking an edition off our ledger (later).** For items an owner wants to hold
independently of us, re-issue the class as a **Taproot Asset** collectible
with the same cap, and burn the ledger serial when its asset is issued.
Taproot Assets are on the allowed list, and transfers can go over Lightning.
The cost is running `tapd` (an LND service), which is outside our Spark
stack, so it waits until editions have real demand.

**Not used:** Ordinals and inscriptions (they spend blockspace on every item
and depend on outside indexers), and Spark tokens (Spark's own token
system. Our Spark docs keep its token features off, and whether it fits the
money rule is an owner question).

**What editions are for:** cosmetics and proof of being there. A founder's lantern
for the first 100 patrons of a realm, a trophy for finishing an event, a
maker's signed first piece. **Never** power in the game, never randomly given
for money, never sold by telling buyers what it might be worth later.

Build:
- `verse/edition.coax` (`#authority`): checks for the class, mint, transfer
  and royalty, the cap, the sequence rules, the Merkle leaf encoding and the
  root. `#forall` that no sequence of operations exceeds the cap and that only
  the owner at the time can transfer. `coax mutate` with zero survivors.
- `proto/`: an `Edition`, `EditionMint`, `EditionTransfer` and `EditionAnchor`
  in a new `verse.economy` version. A NIP draft (`nostr/nips/openagents/NIP-ED.md`)
  so other clients can check editions without us.
- Server: the log table, a mint written when a payout's sale settles, and a
  daily anchor job. The anchor's txid is shown on every item page.
- The Bitcoin Lab can show an edition's anchor transaction, since it already
  shows blocks, transactions and scripts ([bitcoin-lab.md](bitcoin-lab.md)).

### 4. Patronage for realms (next)

The good half of "Crowdsourcing and NFT Sales" (p. 272): players fund a realm
they want to see grow, and get insight and influence back, not a share.

- A maker opens a **campaign** on a realm: a goal in sats, milestones ("the
  marsh gets a second island"), and an edition for backers ("Founder,
  1 of N", where N is the number of backers when the campaign closes).
- Backers pay into it, their editions mint when it closes, and the maker
  gets the money paid out (less the platform share) per milestone or at close.
- What backers get: their name in the realm's credits, early access to
  revisions, and a vote on priorities (shown as a poll, not binding). Never
  income.
- What happens if a campaign fails: state it at the start. Simplest is
  "funds go to the maker as they come in, no refunds", the way most
  patronage sites work. Refunds with hold invoices would need Lightning
  features Spark doesn't give us today.

### 5. AI companions and characters (next)

Chapter 9's "Building AI Companions". A companion follows you from realm to
realm, remembers you, and talks with voice.

- Free: authored characters in realms (what `aethyr/talk/` does now) and the
  daily voice free tier.
- Paid: a companion with memory and voice beyond the free tier, metered by
  `verse/voice-meter.coax` from the same balance. Later, makers sell
  companion personalities and voices as listings (line 2) and editions.
- Costs are real (recognition, synthesis, model calls), so prices must stay
  above cost. The meter's msat prices are the place to set that.

### 6. A share for the realms you forked from (next)

Line 1's split includes a **lineage share**: when a fork sells something, a
small part (proposal: 2.5% of the price, from the platform's 10%, not from the
maker) goes to the maker of the realm it was forked from, one level up
only. That makes forking generous instead of extractive, and gives makers a
reason to make realms worth forking. `forked_from` is already stored.

### 7. Hosting, capacity and Aethyrforge for teams (later)

- **Free:** making, publishing and playing a public realm, with today's limits.
- **Paid, in sats per month:** private realms with invited players, more
  daily builds (`OA_BUILDER_PER_DAY`), larger asset packs, more players in a
  room at once once fast multiplayer exists ([room-overload.md](room-overload.md)),
  hosted agent seats, and a custom address.
- **For studios:** Aethyrforge with team accounts, private assets (kept the
  way [private-assets.md](private-assets.md) says), and engine licensing.

### 8. Agents at work in realms (later; the OpenAgents link)

- **Hire an agent to build:** the "asked for, not built yet" list becomes
  priced tasks on NIP-MKT/NIP-LAB. A maker or player posts a price, an agent
  (or a person) builds the feature, and payment is made after acceptance.
  OpenAgents takes the same platform share.
- **Paid playtesting by agents:** a seated agent walks a realm before publish
  ("can a visitor reach the well?"). Charged per run.
- **Players earning sats, done honestly:** players are paid only for work
  someone outside the game buys: playtesting a realm for its maker, rating
  agents' builds, recording voice lines or animations under a clear licence,
  or teaching characters. The buyer pays, we take a share, and the player
  gets the rest. This is "paid to play" (Chapter 7) without Axie's flaw,
  where earnings came out of new players' entry money (Chapter 4).
- Identity before earning: every payout goes to a Nostr key with history,
  rate limits and replay-checked work, because bots arrive the day earning
  starts (Chapter 7's "On Bots and ID").

### 9. Sponsored realms (later)

Companies pay us (or a maker, through us) for a realm built around their
brand, with brand rules agreed up front. Never ads drawn inside other
makers' realms without the maker opting in. "In App Advertising" (Chapter 7)
isn't worth what it does to the realms.

## What to measure

From the existing analytics, per realm and in total, with our team left out:

- Visitors who become makers (fork or new realm) within 7 days.
- Makers who sell at least once; median and top sats earned per maker per month.
- What players pay: share of visitors who buy, sats per paying player, repeat buyers.
- Our share in sats per month, and its cost (voice, models, hosting) in sats.
- Payout health: time from sale to the maker being paid, failed payouts.
- Countries of makers and buyers, to check the claim that Lightning reaches
  makers that cards don't.

## Order of work

**Now (November):**
1. `verse/split.coax` with the fee, rounding to the maker, and `makers.c`
   using it. Show the split on the BUY sheet. (Owner: the fee.)
2. Generalize listings beyond fairies. Two kinds to start: an Everglade outfit
   and a made realm's decoration.
3. Add MADE BY and FORK to every realm, not only Everglade.

**Next (December–January):**
4. `verse/edition.coax`, the proto messages and NIP draft, the ledger and the
   daily anchor. First edition: founder lanterns for a realm's first backers.
5. Patronage campaigns, using editions.
6. Companions with memory, metered by the voice meter.
7. The lineage share.
8. Aethyrforge's touch layout: making on the phone first.

**Later:**
9. Paid hosting and capacity tiers.
10. Agent tasks from the waiting asks, and paid playtesting.
11. Paid player work, after identity and bot defences are in place.
12. Taproot Assets export for editions, if owners ask for it.
13. Sponsored realms and studio accounts.

## For the owner to decide

1. The platform share (proposed 10%) and whether credit purchases pay it too.
2. The lineage share (proposed 2.5%, one level, out of our share).
3. Editions: approve the design (our ledger, signed by Nostr keys, anchored
   daily to Bitcoin), and whether Spark tokens count as Bitcoin under the
   money rule (this plan assumes they don't).
4. Patronage: payments go to the maker as they arrive, with no refunds. Is that acceptable?
5. Legal review before money flows into resales, campaigns or player earnings:
   money transmission (we receive and pay out on others' behalf), consumer
   law, tax, and securities rules for anything that looks like a stake.
