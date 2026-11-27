# 30. Accounts and persistence

> **Status: partly implemented.** Nostr identity provisioning lives in
> [`coax/web/api/auth-identity-policy.coax`](../../coax/web/api/auth-identity-policy.coax),
> hardware-bound multi-device refresh proofs live in
> [`coax/web/api/auth-device-policy.coax`](../../coax/web/api/auth-device-policy.coax),
> the durable `everglade.character.v2` aggregate and API route live in
> [`everglade/model/character.coax`](../model/character.coax) and
> [`coax/web/api/character-policy.coax`](../../coax/web/api/character-policy.coax),
> fenced checkpoint admission lives in
> [`coax/web/api/checkpoint-policy.coax`](../../coax/web/api/checkpoint-policy.coax),
> and asymmetric v5 seat tickets live in
> [`metaverse-engine/session/seat-ticket.coax`](../../metaverse-engine/session/seat-ticket.coax)
> and [`coax/web/room/room-ticket.coax`](../../coax/web/room/room-ticket.coax),
> exercised by [`everglade/examples/accounts-character-persistence.coax`](../examples/accounts-character-persistence.coax).
> Server-side Character Create appearance sync across devices and multiple
> character slots per account remain to be built.

This chapter answers the player's question: "If I play on my Mac tonight and
open Everglade on my phone tomorrow, why does my Ranger keep every level, item,
and quest—and why can't a stolen refresh token hijack my account?"

In [chapter 13](ch013-coming-back.md), a single room checkpoint kept you from
snapping back to spawn on restart. Across devices, restarts, and shards, the
backend API (`coax/web/api/`) owns three durable boundaries: who you are, what
devices hold your session, and which room generation may advance your character.

## What this chapter teaches

- How Nostr identity provisioning (`ai_plan`) gives every live account one
  durable identity while refusing frozen or deleted accounts.
- How native apps bind refresh tokens to non-exportable hardware P-256 keys
  (Secure Enclave / StrongBox / TEE) with 60-second anti-replay proofs.
- How `everglade.character.v2` (`character_ticket`, `character_checkpoint`)
  enforces class immutability, monotonic XP, and ledger-owned bags.
- How `room_checkpoint_decide` fences stale room writers and rejects payload
  conflicts or XP regressions before touching the database.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Identity provisioning | `ai_plan` decides `AI_CREATE` / `AI_LINKED` / `AI_MANAGED` / `AI_SKIP` for Nostr identities | [`coax/web/api/auth-identity-policy.coax`](../../coax/web/api/auth-identity-policy.coax) |
| Device-bound sessions | `dk_register` and `dk_refresh` enforce P-256 hardware proofs (`OA-Device-Proof`, ±60 s window, replay/revocation refusal) | [`coax/web/api/auth-device-policy.coax`](../../coax/web/api/auth-device-policy.coax) |
| Character aggregate & route | `everglade.character.v2` codec (`who`, `class`, `xp`, `floor`, `bag`, `inv`, `worn`, `coin`, `quests`, `range`) and API merge rules | [`everglade/model/character.coax`](../model/character.coax), [`coax/web/api/character-policy.coax`](../../coax/web/api/character-policy.coax) |
| Fenced checkpoints | `room_checkpoint_input` & `room_checkpoint_decide`: 9 explicit outcomes (`applied`..`regression`) | [`coax/web/api/checkpoint-policy.coax`](../../coax/web/api/checkpoint-policy.coax) |
| Asymmetric seat tickets | v5 tickets signed by API private key and verified by rooms using public keys only (`OA_ROOM_TICKET_PUBKEYS`) | [`metaverse-engine/session/seat-ticket.coax`](../../metaverse-engine/session/seat-ticket.coax), [`coax/web/room/room-ticket.coax`](../../coax/web/room/room-ticket.coax) |

**What remains to ship:** Class, XP, level floor, bag, worn gear, coin, quest
stages, Archery Range tallies, and fenced room checkpoints all persist on the
server in PostgreSQL (`verse.characters`, `verse.items`,
`verse.character_checkpoints`). However, Character Create cosmetic appearance
sliders and display name still originate from local browser storage on `hello`,
and multi-character selection slots per account remain to be built.

## The method

### Identity and device-bound refresh tokens

Every live account maps to one durable identity (`ai_plan`): an existing
managed or user-linked Nostr key wins; a keyless live account provisions one;
a frozen or deleted account always returns `AI_SKIP`. On native macOS, iOS, and
Android apps, login registers a hardware-backed P-256 public key (`dk_register`).
Every subsequent refresh requires a fresh `OA-Device-Proof` signature within
`DK_WINDOW_MS` (60,000 ms). A token copied to another device fails with
`DK_NEEDED` or `DK_BAD`; a replayed proof returns `DK_REPLAY` without revoking
the legitimate device; a removed key returns `DK_REVOKED`; and a bound session
can never silently downgrade to `DK_UNBOUND`.

### Server-owned character records and fenced checkpoints

When any device requests a seat ticket, `character_ticket` merges the stored
`everglade.character.v2` record with the highest room-reported XP, the realm's
level floor, and the item ledger's current bag and inventory version:

- **Class is set once.** Attempting to overwrite `"srd-ranger"` with
  `"srd-wizard"` on a second device leaves `"srd-ranger"` intact.
- **XP only rises.** A stale room or older device sending `xp = 100` against a
  stored `950` cannot lower it; `character_checkpoint` returns `""` (no write)
  for any XP that does not strictly increase.
- **Level is derived, never stored.** `srd_level_from(xp, floor, 6)` computes
  level directly from `rules/srd/advancement.coax` (`950 XP -> level 3`,
  `2700 XP -> level 4`).
- **Every checkpoint is fenced.** `room_checkpoint_decide` verifies owner
  generation, room process identity, revision order, and payload identity under
  an owner-row database lock before accepting a write (`applied`), coalescing an
  identical retry (`duplicate`), or rejecting `fenced`, `conflict`, or
  `regression`.

## Worked trace: second-device login and checkpoint fencing

```text
1. Provisioning & devices:
   live keyless -> AI_CREATE; linked Nostr -> AI_LINKED; frozen -> AI_SKIP
   iOS + Mac Enclave register -> DK_OK; fresh proof -> DK_OK
   stolen token (no proof) -> DK_NEEDED; replay -> DK_REPLAY
   clock off by 60,001 ms  -> DK_CLOCK; removed key -> DK_REVOKED

2. Character across devices (p-a):
   Device 1 ticket: new srd-ranger, xp=950, floor=2, bag=[4,9,12] -> level 3
   Device 2 ticket: sends srd-wizard, xp=100, bag=[4,9,12,15]
     -> keeps srd-ranger, keeps xp=950 (level 3), syncs ledger bag [4,9,12,15]
   Checkpoint xp=2700 -> accepted, level rises to 4
   Checkpoint xp=800  -> refused ("", stored record untouched)

3. Fenced checkpoint admission (directory owner=4, process=101):
   (owner=4, rev=2, xp=2700) over (rev=1, xp=950)           -> applied (0)
   (owner=4, rev=2, xp=2700, same=true) retry              -> duplicate (1)
   (owner=3, rev=3, xp=2800) stale owner                   -> fenced (3)
   (owner=4, rev=2, xp=2700, same=false) altered payload   -> conflict (4)
   (owner=4, rev=3, xp=2600) lower XP                      -> regression (8)
```

## Runnable example

[`everglade/examples/accounts-character-persistence.coax`](../examples/accounts-character-persistence.coax)
runs the production API policy modules for identity provisioning, device-key
binding, character merge/checkpoint rules, and fenced checkpoint admission
(committed transcript
[`everglade/examples/accounts-character-persistence.out`](../examples/accounts-character-persistence.out)):

<!-- source: everglade/examples/accounts-character-persistence.coax accounts-character-persistence -->
```jai
accounts_run :: () -> bool {
  // 1. Account identity provisioning and hardware-bound multi-device login/refresh.
  p_new := ai_plan(true, false, false, false);
  p_linked := ai_plan(true, false, false, true);
  p_frozen := ai_plan(true, true, false, true);
  reg_ios := dk_register(true, true, true, DK_IOS, DK_ENCLAVE);
  reg_mac := dk_register(true, true, true, DK_MAC, DK_ENCLAVE);
  now := 1800000000000;
  ref_ok := dk_refresh(true, false, true, true, now, now, true, false);
  ref_stolen := dk_refresh(true, false, false, false, 0, now, false, false);
  ref_replay := dk_refresh(true, false, true, true, now, now, true, true);
  ref_clock := dk_refresh(true, false, true, true, now - 60001, now, true, false);
  ref_revoked := dk_refresh(true, true, true, true, now, now, true, false);
  print("id_new=% id_linked=% id_frozen=% bind_ok=% refresh_ok=% refuse_stolen=% replay=% clock=% revoked=%\n",
    p_new == AI_CREATE, p_linked == AI_LINKED, p_frozen == AI_SKIP,
    reg_ios == DK_OK && reg_mac == DK_OK, ref_ok == DK_OK,
    ref_stolen == DK_NEEDED, ref_replay == DK_REPLAY, ref_clock == DK_CLOCK, ref_revoked == DK_REVOKED);

  // 2. Durable Character aggregate across devices: class is set once, XP never falls, bag follows ledger.
  b1: [CHAR_BODY] u8; b2: [CHAR_BODY] u8; b3: [CHAR_BODY] u8; b4: [CHAR_BODY] u8;
  kw: [16] u8;
  rec1 := character_ticket("", "p-a", "srd-ranger", 950, 2, "[4,9,12]", 5, b1[..]);
  // Second device signs in with stale lower XP (100) and attempts a class change -> both ignored/kept.
  rec2 := character_ticket(rec1, "p-a", "srd-wizard", 100, 1, "[4,9,12,15]", 6, b2[..]);
  cls := character_class_word(rec2, kw[..]);
  // Accepted checkpoint raises XP from 950 -> 2700 (level 4); regression to 800 returns empty (keep).
  rec3 := character_checkpoint(rec2, 2700, b3[..]);
  reg_ck := character_checkpoint(rec3, 800, b4[..]);
  print("cls=% xp=% lvl=% ck_xp=% ck_lvl=% refuse_xp_drop=%\n",
    cls, character_xp(rec2), character_level_of(rec2),
    character_xp(rec3), character_level_of(rec3), reg_ck.count == 0);

  // 3. Fenced checkpoint admission: owner generation, process identity, revision & XP monotonicity.
  d_ok := room_checkpoint_decide(true, true, OwnerGeneration.{value = 4}, OwnerGeneration.{value = 4}, 101, 2, 2700, OwnerGeneration.{value = 4}, 101, 1, 950, false);
  d_dup := room_checkpoint_decide(true, true, OwnerGeneration.{value = 4}, OwnerGeneration.{value = 4}, 101, 2, 2700, OwnerGeneration.{value = 4}, 101, 2, 2700, true);
  d_fence := room_checkpoint_decide(true, true, OwnerGeneration.{value = 3}, OwnerGeneration.{value = 4}, 101, 3, 2800, OwnerGeneration.{value = 4}, 101, 2, 2700, false);
  d_conf := room_checkpoint_decide(true, true, OwnerGeneration.{value = 4}, OwnerGeneration.{value = 4}, 101, 2, 2700, OwnerGeneration.{value = 4}, 101, 2, 2700, false);
  d_reg := room_checkpoint_decide(true, true, OwnerGeneration.{value = 4}, OwnerGeneration.{value = 4}, 101, 3, 2600, OwnerGeneration.{value = 4}, 101, 2, 2700, false);
  print("ck=% dup=% fence=% conflict=% regression=%\n",
    room_checkpoint_status(d_ok), room_checkpoint_status(d_dup),
    room_checkpoint_status(d_fence), room_checkpoint_status(d_conf), room_checkpoint_status(d_reg));

  return p_new == AI_CREATE && p_linked == AI_LINKED && p_frozen == AI_SKIP &&
         reg_ios == DK_OK && reg_mac == DK_OK && ref_ok == DK_OK &&
         ref_stolen == DK_NEEDED && ref_replay == DK_REPLAY && ref_clock == DK_CLOCK && ref_revoked == DK_REVOKED &&
         cls == "srd-ranger" && character_xp(rec2) == 950 && character_level_of(rec2) == 3 &&
         character_xp(rec3) == 2700 && character_level_of(rec3) == 4 && reg_ck.count == 0 &&
         d_ok == 0 && d_dup == 1 && d_fence == 3 && d_conf == 4 && d_reg == 8;
}
```

Output (`coax/bin/coax run everglade/examples/accounts-character-persistence.coax`, identical in `--release`):

```text
id_new=true id_linked=true id_frozen=true bind_ok=true refresh_ok=true refuse_stolen=true replay=true clock=true revoked=true
cls=srd-ranger xp=950 lvl=3 ck_xp=2700 ck_lvl=4 refuse_xp_drop=true
ck=applied dup=duplicate fence=fenced conflict=conflict regression=regression
ok=true
```

## The checks

```shell
coax/bin/coax test --library coax/web/api/auth-identity-policy.coax
coax/bin/coax test --library coax/web/api/auth-device-policy.coax
coax/bin/coax test --library coax/web/api/character-policy.coax
coax/bin/coax test --library coax/web/api/checkpoint-policy.coax
coax/bin/coax run everglade/examples/accounts-character-persistence.coax
```

## Mistakes

- **Shared symmetric secrets on rooms.** Earlier v2–v4 seat tickets used one
  HMAC key shared between the API and every room shard, meaning a compromised
  room could theoretically mint tickets for any account. `#13260` introduced
  asymmetric v5 seat tickets (`seat-ticket.coax`) so rooms hold verification
  public keys only.
- **Quietly downgrading malformed device keys.** Accepting a login with a
  malformed hardware key as an unbound session silently weakened device binding.
  `dk_register` fails closed with `DK_BAD_KEY` or `DK_BAD_PLATFORM` and never
  coerces a malformed key to `DK_UNBOUND`.

## What comes next

Once the game, shards, and persistence all work, you have to move new releases
to players without breaking live sessions. [Chapter 31](ch031-ship-it.md) covers
the single release-set command, canary checks, pin gates, and rollback.
