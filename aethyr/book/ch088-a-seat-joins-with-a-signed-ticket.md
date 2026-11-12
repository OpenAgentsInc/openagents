# 88. A seat joins with a signed ticket

> **Status: partly implemented.** The API signs a seat ticket and oa-room
> checks it at join, so a page cannot claim another player's id
> ([room-ticket.coax](../../coax/web/room/room-ticket.coax), #12712). The
> ticket names the room and the shard, and a shard seats only its own
> tickets (#12717). The hello carries the wire version, and a room refuses
> any other with `r 0 version` (chapter 98, #12788, commit 67b4f62448). A
> ticket authenticates a player. Owner generations and durable character
> capture add fencing and resume state; their rollout and persistence
> remain distinct from seat authentication.

A server that owns the scene must know who sits in each seat. The page can't
say so itself: anything a page sends, a player can change. This chapter adds
a signed seat ticket and the join that checks it.

## What this chapter teaches

- Identity decided by the API, carried by a ticket, checked by the server.
- What a join checks, in order.
- Which hello fields the server takes from the page and which it overrides.

## The design

The login sequence in the research doc (section 4) has eight steps. A room
join does the ones that belong to the room:

1. **Authenticate.** The page asks the API for a ticket,
   `POST /api/playtest/room-ticket/ROOM`. The API knows the player by their
   key and refuses a kicked player.
2. **Place.** With `?realm=`, the API's instance directory picks a room and a
   shard with a free seat (chapter 101), or answers 503 when every room is
   full.
3. **Sign.** The ticket is
   `v3.PID.ACCT.NAME64.ROOM.SHARD.EXPIRES.HP.MAC`, the MAC an HMAC-SHA256
   under the room's key. It also carries the HP and XP the player left with,
   so a reload doesn't heal them.
4. **Join.** The page opens `/api/room/relay/everglade/CODE?t=TICKET`. The
   server checks the room code, the MAC in constant time, the expiry with a
   minute of clock skew, the shard and the kick list (`tk_check`,
   `rl_join`).
5. **Seat.** The seat takes the ticket's id, name and account bit.

<!-- source: coax/web/room/room-ticket.coax seat-ticket -->
```jai
Ticket :: struct {
  ok: bool;
  pid: [TICKET_PID] u8; pid_n: int;
  account: bool;
  admin: bool;            // ACCT 2: an account who is one of the realm's admins (its demo character, everglade/server/progress.coax)
  name: [TICKET_NAME] u8; name_n: int;
  expires: int;
  hp: int = -1;           // the HP it brings back (-1 none)
  shard: int = 1;         // the oa-room shard it was signed for (v2: 1)
  xp: int;                // the XP it brings back (everglade/server/progress.coax)
  wanted: int;            // the WANTED level it brings back (1 to 5; 0 none; #12794)
  owner: int;             // the character's owner generation the API issued with it (#12834; 0 none): its vitals lines carry it
  schema: int = TICKET_SCHEMA;   // the format of what it brings back (HP, XP, WANTED): the room admits it only up to RV_SCHEMA (#12839)
  capture: CharacterCapture;     // ticket v4's signed durable resume profile (schema 2)
  rest: CharacterRest;           // and a profile 3 capture's rest (#13046)
  ext: [TK5_EXT] u8; ext_n: int; // v5's signed EXT items ("" none): tk5_ext(cast(string) t.ext[0..<t.ext_n], NAME)
  key: int = -1;                 // v5: which of the room's public keys checked it (-1: v2, v3 or v4, the HMAC key)
}
```

The page's hello (`H 1 LOOK CLASS FLAGS NAME`) supplies only the look and the
class. When a ticket exists, its name and account bit replace the hello's
(`eg_heard_hello`).

## Run it

The ticket tests sign, check, expire and forge tickets:

```shell
coax/bin/coax test --library coax/web/room/room.coax --only "ticket:" --steps 4000000000
```

```text
ok   #test ticket: t= comes out of the request line's query (1 case, 5285 steps, 0 ms)
ok   #test ticket: tk_mac signs a report longer than a ticket, as the vitals post for many seats is (#12717) (1 case, 589540 steps, 6 ms)
6 of 357 checks (--only 'ticket:'), 6 passed, 0 failed
```

The output is cut to its last lines. The two `room.coax` checks join a seat
through `rm_join` with a ticket and with a forged one.

## What is left

- Authentication alone does not prove character liveness or ownership.
  Keep owner fencing, durable capture and deployment policy qualified
  separately from the signed ticket codec. Chapter 98 covers wire versions
  and seat generations, both of which are implemented.

## Acceptance checks

- A ticket for another room, an expired ticket and a ticket with a changed
  byte are refused.
- A page that sends another player's name in its hello is seated under its
  ticket's name.

## Dependencies

Chapter 86 (the server).

## In the other books

- The player key behind a ticket is a Nostr key. Signatures over events are
  built in [Nostr chapter 21](../../nostr/book/ch021-events-ids-and-signatures.md),
  on the Schnorr signatures of
  [Bitcoin chapters 23 and 24](../../bitcoin/book/ch023-verify-a-schnorr-signature.md).
- [Everglade chapter 12](../../everglade/book/ch012-the-room.md) joins a
  room with this ticket, and
  [chapter 13](../../everglade/book/ch013-coming-back.md) uses it to return
  a player to where they stood.
