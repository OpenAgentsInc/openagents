# 0006. The item ledger's rules live in Coax; SQL stores

Date: 2026-11-12
Status: Accepted
Source: [Domain-driven design and how to incorporate it](../audits/2026-11-11-domain-driven-design-audit.md), sections 3.2 and 4.2; issue #13318 (DDD.7, tracker #13311); [coax/web/api/COAX.md](../../coax/web/api/COAX.md)

## Context

The item ledger existed twice. `metaverse-engine/session/ledger.coax` (`#authority`, mutation-tested) held the rules for an in-memory store. The plpgsql function `verse.item_apply` (migration 0130) called itself "the Postgres form" of that model, and it was what the API's route would call. No check ran one against the other, and they had already drifted apart:

- Keep time: the Coax model remembered an operation ID for `LEDGER_KEEP` = 1000 ticks; SQL remembered it for a day in milliseconds (`verse.item_keep_ms`).
- Character identity: Coax compared `CharacterId` ints; SQL compared `coalesce(account, id)` text.
- Exhaustion: Coax refused a write that would advance a version past the largest int (`LEDGER_EXHAUSTED`). SQL had no such rule, and its `int` version column would raise an error instead.
- An unknown operation: SQL refused it before reading the ID. Coax read the ID first and recorded the refusal.

#13291 is about to send loot rewards through the ledger. If the two models still disagreed then, the disagreement would reach players.

## Decision

- **Coax decides and SQL stores.** `metaverse-engine/session/ledger.coax` is the one owner of the ledger's rules. These are `ledger_gate` (the ask before any inventory is read: a known operation and character, the ID's record, the keep time), `ledger_rule` (the generation fence, the version, item ownership, escrow and exhaustion) and the effects (`ledger_creates`, `ledger_moves`, `ledger_escrows`, `ledger_settles` and `ledger_clears`). The in-memory store calls them. The API's route calls them too: `player.c` `room_items` goes through `coax/web/api/item-policy.coax`, a thin `#authority` adapter that holds only the route's words.
- **SQL keeps storage.** That means the rows (`verse.items`, `verse.inventories`, `verse.item_ops`, `verse.item_outbox`), the version guard (one advisory lock for each operation ID, then the item's row, then the inventories' rows in `who` order, all inside one transaction), operation-ID uniqueness (the primary key; a forgotten record is replaced) and the outbox insert. Migration 0165 drops `verse.item_apply` and `verse.item_keep_ms`. `verse.item_reconcile` stays: it is the restore audit, which replays the outbox independently and compares the result with the rows the rules wrote.
- **Keep time is wall-clock milliseconds, a day.** `LEDGER_KEEP :: 86400000`. Every store measures `at`, `sent` and `now` in milliseconds. There are no ticks in the ledger.
- **Character identity is the store's own, and the rules only see equality.** The durable identity is the account's: `coalesce(account, id)` text, as `verse.item_who` names it. `CharacterId` is the in-memory store's index into its arrays and nothing more. The rules never compare identities themselves. Each store reports the facts as booleans: the item is the writer's (`mine`), the receiver is a character (`to_known`), the receiver is the writer (`to_self`), the item has an escrow party (`escrowed`). A room that asks the API names players by player ID, and the API maps them to account identities.
- **Versions are 64-bit everywhere.** `verse.inventories.version` is a `bigint` (0165), so the rules' `LEDGER_EXHAUSTED` applies at the same value in both stores.
- **The operation ID is typed.** In Coax it is `OperationId` (`metaverse-engine/core/ids.coax`). On the wire and in SQL it is text of 1 to 80 bytes, and `item_gate` checks that length. A room builds it from its shard, its process generation and a counter. An unknown operation, an unknown character or a bad ID is refused (`held`) before the ID's record is read, and that refusal is not recorded.

## Consequences

- One set of checks covers both stores. `ledger.coax` and `item-policy.coax` are tested and mutated with zero survivors. `room_checks.coax` "items" runs the route against Postgres: duplicates, conflicts, versions, escrow, settles, fences, expiry, exhaustion, the outbox and the restore audit.
- A new ledger rule goes into `ledger.coax`, and the route picks it up from there. The SQL side changes only when storage changes.
- The route takes a few more round trips inside its transaction than the old single plpgsql call did. The items route is cold: nothing calls it in play until #13291. (Since #13291 a room calls it for loot, quests and what is worn, with two more operations, `consume` and `claim`: [item operation v1](../protocol/everglade-item-op-v1.md).)
- A settle's outbox entry names the escrow party as its receiver, in both stores.
- The item's content kind is still a raw `int`. Its type comes from #13286.
