# 28. Interactions

> **Status: partly implemented.** Data-driven quests, dialogues, loot tables,
> and rewards live in [`everglade/model/content.coax`](../model/content.coax),
> [`everglade/content/everglade.content`](../content/everglade.content), and
> [`everglade/client/bag.coax`](../client/bag.coax); compare-and-set quest
> stages and equipment rules live in
> [`everglade/model/character.coax`](../model/character.coax); the idempotent
> item and escrow ledger lives in
> [`metaverse-engine/session/ledger.coax`](../../metaverse-engine/session/ledger.coax)
> and [`everglade/model/item-op.coax`](../model/item-op.coax); and per-NPC
> conversation memory lives in [`everglade/world/people.coax`](../world/people.coax)
> (`0031_npc_memory.sql`), exercised by
> [`everglade/examples/interactions-quests-ledger.coax`](../examples/interactions-quests-ledger.coax).
> Live player-to-player trade wire commands, merchant shops, and city-wide
> faction memory remain to be wired into production rooms.

This chapter answers the player's question: "If I finish the Warden's quest and
trade his cloak to a friend, why can't a network retry duplicate the cloak or
pay the reward twice?"

Talking, questing, looting, and trading all turn player actions into lasting
state changes. If the browser decides outcomes or the server applies retries
without idempotency, players duplicate items, skip quest chains, or lose gear
across reconnects.

## What this chapter teaches

- How Everglade declares quests, dialogue chains, loot weights, and rewards as
  validated data tables (`aethyr.content.v1`) rather than zone code.
- How `char_claim` advances quest stages by compare-and-set (`from -> to`) so
  duplicate or stale completions never double-pay coin or XP.
- How `ItemLedger` guarantees every grant, escrow, settle, and consume is
  idempotent (`OperationId`), fenced by owner generation (`LEDGER_STALE`), and
  protected against double-spend (`LEDGER_HELD`, `LEDGER_VERSION`).
- How per-NPC conversation memory quotes quest context into the player's turn
  without handing persona authority to the client.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Content tables & quest engine | 14-table schema (`ability`..`troop_phase`), `ec_ok`, `ec_drop` (weights sum to 100), `eq_next`, `eq_offer_open` | [`everglade/model/content.coax`](../model/content.coax), [`everglade/content/everglade.content`](../content/everglade.content) |
| Client bag, quest log & talk | Displays server-owned record and sends `E SEQ 6/7/8` asks; stores nothing in the browser | [`everglade/client/bag.coax`](../client/bag.coax) |
| Character claims | Compare-and-set quest stage transitions, game coin (copper only), XP, and 10 equipment slots | [`everglade/model/character.coax`](../model/character.coax) |
| Item & escrow ledger | `LOP_GRANT`..`LOP_CLAIM` with `OperationId` dedup, owner fencing, version CAS, escrow/settle, and outbox | [`metaverse-engine/session/ledger.coax`](../../metaverse-engine/session/ledger.coax), [`everglade/model/item-op.coax`](../model/item-op.coax) |
| Per-NPC memory | Visit-stamped conversation history and quest `scene` notes persisted per player per NPC | [`everglade/world/people.coax`](../world/people.coax) |

**What remains to ship:** The `#authority` item ledger already enforces full
`LOP_ESCROW`, `LOP_SETTLE`, `LOP_CANCEL`, and `LOP_TRANSFER` semantics, and
production rooms already grant loot, equip items, and pay quest rewards through
`everglade.item-op.v1`. However, as [`everglade/model/item-op.coax`](../model/item-op.coax)
notes, rooms do not yet emit player-to-player `transfer` or `escrow` wire
commands (`"to"` is still empty), merchant buy/sell UIs are not yet built, and
city-wide faction reputation beyond per-NPC memory remains to be designed.
Game coin in the character record is in-game copper only, never money (real
value settles on Bitcoin rails per `docs/decisions/0006-item-ledger-rules-in-coax.md`).

## The method

### Quests as monotonic stage counters over data tables

In `everglade/content/everglade.content` (revision 3), a quest's entire progress
fits in one ascending integer stage on the character record:

- `0`: not taken (`EQ_TAKE` — offered by a `dialogue` row).
- `1 .. 1 + steps`: each objective unit (`EQ_KILL`, `EQ_REACH`, `EQ_BRING`, or
  `EQ_TALK`) advances the stage by 1.
- `1 + steps .. done`: each `reward` row (`EQ_REWARD`) advances the stage by 1
  until `EQ_DONE`.

Chain quests reuse one dialogue to hand in Quest A and offer Quest B.
`eq_offer_open` refuses to offer Quest B until Quest A has reached that exact
hand-in stage, preventing out-of-order quest starts on both client and server.

### Compare-and-set claims and escrowed trades

Every reward, equip, or item mutation crosses `everglade.item-op.v1` with a
caller-chosen `OperationId`, the room's `OwnerGeneration`, and the inventory
version `ver`:

1. **Retries replay safely.** Re-sending the same `OperationId` and payload
   returns `replay=true` and the original result without creating a second item.
2. **Payload mismatches fail closed.** Reusing an `OperationId` with a different
   item kind returns `LEDGER_CONFLICT`.
3. **Escrow blocks double-spending.** Escrowing an item from Alice to Bob marks
   `escrow_to = Bob`; any competing `LOP_TRANSFER` returns `LEDGER_HELD` until
   `LOP_SETTLE` moves ownership to Bob (or `LOP_CANCEL` returns it).
4. **Fencing stops stale rooms.** Once Bob's owner generation advances from 1 to
   2, any delayed write from generation 1 returns `LEDGER_STALE`.

## Worked trace: Quest 1 (*Thin the pack*) and escrowed cloak trade

```text
Content rev 3, Quest 1 ("Thin the pack"):
  stage 0 -> EQ_TAKE (1)    Warden dialogue 1 at Glade's edge
  stage 1 -> EQ_KILL (2)    Slay 1 Wolf (creature 1; loot roll 0..59 -> Wolf pelt, 60..99 -> none)
  stage 2 -> EQ_TALK (5)    Warden dialogue 3 (hands in Q1 AND unlocks Q3 via eq_offer_open)
  stage 3 -> EQ_REWARD (6)  Warden's cloak (item 3, body slot 1), +25 coin, +250 XP
  stage 4 -> EQ_DONE (7)

Character CAS & Ledger trade:
  char_claim(q1: 0->1 -> 2 -> 3, +25 coin, +250 XP, wear item 3 in slot 1) -> ok
  stale char_claim(q1: 1->2) -> refused (!ok)
  wear unowned item 99       -> refused (!ok)
  op 10 LOP_GRANT(alice, gen=1, ver=0, kind=3) -> ok, item=0; retry -> replay=true
  op 10 with kind=4                            -> conflict
  op 11 LOP_ESCROW(alice -> bob, ver=1)        -> ok
  op 12 LOP_TRANSFER(alice -> bob, ver=2)      -> held (cannot double-spend escrowed item)
  op 13 LOP_SETTLE(alice -> bob, ver=2)        -> ok, owner=bob (2), outbox=3
  fence(bob, gen=2); op 14 LOP_CONSUME(gen=1)  -> stale
```

## Runnable example

[`everglade/examples/interactions-quests-ledger.coax`](../examples/interactions-quests-ledger.coax)
loads the real shipped content tables (`EC_SHIPPED`), walks Quest 1 and its
quest-chain lock, verifies `char_claim` CAS & equipment refusals, and runs a
grant → replay → conflict → escrow → held-refusal → settle → stale-fence cycle
on `ItemLedger` (committed transcript
[`everglade/examples/interactions-quests-ledger.out`](../examples/interactions-quests-ledger.out)):

<!-- source: everglade/examples/interactions-quests-ledger.coax interactions-quests-ledger -->
```jai
interactions_run :: () -> bool {
  // 1. Load and validate Everglade's shipped content tables (14 tables, rev 3) and walk Quest 1.
  doc: [1] ContentDoc;
  loaded := ct_read(doc[..], EC_SHIPPED) && ec_ok(doc[..]);
  q0 := eq_next(doc[..], 1, 0);   // EQ_TAKE dialogue 1 (Warden)
  q1 := eq_next(doc[..], 1, 1);   // EQ_KILL creature 1 (Wolf)
  q2 := eq_next(doc[..], 1, 2);   // EQ_TALK dialogue 3 (hands Quest 1 in, unlocks Quest 3)
  q3 := eq_next(doc[..], 1, 3);   // EQ_REWARD (Warden's cloak, 25 coin, 250 XP)
  q4 := eq_next(doc[..], 1, 4);   // EQ_DONE
  // Quest chain gate: dialogue 3 offers Quest 3 only once Quest 1 reaches stage 2.
  early_stages := [5] int.[1, 0, 0, 0, 0];
  ready_stages := [5] int.[2, 0, 0, 0, 0];
  chain_locked := !eq_offer_open(doc[..], 3, 3, early_stages[..]);
  chain_open := eq_offer_open(doc[..], 3, 3, ready_stages[..]);
  drop_hit := ec_drop(doc[..], 1, 59);
  drop_miss := ec_drop(doc[..], 1, 60);
  print("tables_ok=% rev=% q1_kinds=%,%,%,%,% chain_locked=% chain_open=% wolf_drop=%,%\n",
    loaded, doc[0].rev, q0.kind, q1.kind, q2.kind, q3.kind, q4.kind,
    chain_locked, chain_open, drop_hit, drop_miss);

  // 2. Compare-and-set quest claims, coin, XP, and wearing a reward item in Character.
  c := char_new("p-a", 1);
  c.bag[0] = 3; c.bag_n = 1; c.inv = 1;   // item 3 (Warden's cloak) granted into bag
  c = char_claim(c, CharClaim.{quest = 1, from = 0, to = 1});
  c = char_claim(c, CharClaim.{quest = 1, from = 1, to = 2});
  c = char_claim(c, CharClaim.{quest = 1, from = 2, to = 3, coin = 25, xp = 250, slot = 1, item = 3});
  stale_claim := char_claim(c, CharClaim.{quest = 1, from = 1, to = 2});
  missing_wear := char_claim(c, CharClaim.{slot = 6, item = 99});
  print("claim_ok=% stage=% coin=% xp=% worn_body=% refuse_stale=% refuse_unowned=%\n",
    c.ok, char_stage(c, 1), c.coin, c.xp, c.worn[1], !stale_claim.ok, !missing_wear.ok);

  // 3. Idempotent item & trade ledger: grant -> replay -> escrow -> double-spend refusal -> settle.
  l: [1] ItemLedger;
  alice := CharacterId.{value = 1};
  bob := CharacterId.{value = 2};
  g1 := OwnerGeneration.{value = 1};
  g2 := OwnerGeneration.{value = 2};
  _ := ledger_fence(l[..], alice, g1);
  _ = ledger_fence(l[..], bob, g1);
  grant_ask := LedgerAsk.{id = OperationId.{value = 10}, op = LOP_GRANT, c = alice, gen = g1, ver = 0, kind = ItemKindId.{value = 3}, sent = 1000};
  r_grant := ledger_apply(l[..], grant_ask, 1000);
  r_replay := ledger_apply(l[..], grant_ask, 1005);
  // Same OperationId with a different kind is refused as LEDGER_CONFLICT.
  bad_reuse := grant_ask; bad_reuse.kind = ItemKindId.{value = 4};
  r_conflict := ledger_apply(l[..], bad_reuse, 1010);
  // Escrow item to Bob; trying to transfer the escrowed item fails with LEDGER_HELD.
  esc := ledger_apply(l[..], LedgerAsk.{id = OperationId.{value = 11}, op = LOP_ESCROW, c = alice, gen = g1, ver = 1, item = r_grant.item, to = bob, sent = 1020}, 1020);
  dbl := ledger_apply(l[..], LedgerAsk.{id = OperationId.{value = 12}, op = LOP_TRANSFER, c = alice, gen = g1, ver = 2, item = r_grant.item, to = bob, sent = 1030}, 1030);
  set := ledger_apply(l[..], LedgerAsk.{id = OperationId.{value = 13}, op = LOP_SETTLE, c = alice, gen = g1, ver = 2, item = r_grant.item, sent = 1040}, 1040);
  // Advance Bob's owner generation to 2; a stale write under gen=1 is refused with LEDGER_STALE.
  _ = ledger_fence(l[..], bob, g2);
  stale_op := ledger_apply(l[..], LedgerAsk.{id = OperationId.{value = 14}, op = LOP_CONSUME, c = bob, gen = g1, ver = 1, item = r_grant.item, sent = 1050}, 1050);
  print("grant=% replay=% conflict=% escrow=% held=% settle=% owner=% stale=% outbox=%\n",
    io_word(r_grant.status), r_replay.replay, io_word(r_conflict.status),
    io_word(esc.status), io_word(dbl.status), io_word(set.status),
    l[0].items[r_grant.item.value].owner.value, io_word(stale_op.status), l[0].out_n);

  return loaded && q0.kind == EQ_TAKE && q1.kind == EQ_KILL && q2.kind == EQ_TALK &&
         q3.kind == EQ_REWARD && q4.kind == EQ_DONE && chain_locked && chain_open &&
         drop_hit == 1 && drop_miss == 0 && c.ok && char_stage(c, 1) == 3 &&
         c.coin == 25 && c.xp == 250 && c.worn[1] == 3 && !stale_claim.ok && !missing_wear.ok &&
         r_grant.status == LEDGER_OK && r_replay.replay && r_conflict.status == LEDGER_CONFLICT &&
         esc.status == LEDGER_OK && dbl.status == LEDGER_HELD && set.status == LEDGER_OK &&
         l[0].items[r_grant.item.value].owner == bob && stale_op.status == LEDGER_STALE && l[0].out_n == 3;
}
```

Output (`coax/bin/coax run everglade/examples/interactions-quests-ledger.coax`, identical in `--release`):

```text
tables_ok=true rev=3 q1_kinds=1,2,5,6,7 chain_locked=true chain_open=true wolf_drop=1,0
claim_ok=true stage=3 coin=25 xp=250 worn_body=3 refuse_stale=true refuse_unowned=true
grant=ok replay=true conflict=conflict escrow=ok held=held settle=ok owner=2 stale=stale outbox=3
ok=true
```

## The checks

```shell
coax/bin/coax test --library everglade/model/content.coax
coax/bin/coax test --library everglade/model/character.coax
coax/bin/coax test --library everglade/model/item-op.coax
coax/bin/coax test --library metaverse-engine/session/ledger.coax
coax/bin/coax run everglade/examples/interactions-quests-ledger.coax
```

## Mistakes

- **Splitting item mutations from outbox writes.** If a room or store updated
  item ownership and then failed when appending its outbox entry, downstream
  views drifted permanently from the inventory. `ledger_commit` wraps both in
  one `#transaction` block so an outbox refusal rolls back items and versions
  whole.
- **Letting SQL own item rules separately from Coax.** Duplicating escrow and
  version rules in SQL stored procedures invited silent divergence between the
  in-memory test ledger and Postgres (`#13318`, decision
  [`0006-item-ledger-rules-in-coax.md`](../../docs/decisions/0006-item-ledger-rules-in-coax.md)).
  Every store now asks `metaverse-engine/session/ledger.coax` before mutating.

## What comes next

Part IX turns from building the world to operating it as an MMORPG.
[Chapter 29](ch029-many-rooms.md) scales beyond a single 16-player room into
multi-shard directories and crash-safe room travel.
