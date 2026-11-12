# Room writes v1

A room shard writes to the API: vitals, checkpoints, items, its directory
report, its stats, its control ask and its moderators' actions. A deploy asks a shard to drain. Before
version 1 (#13260) every shard signed these with the one shared
`OA_ROOM_TICKET_KEY`, so any shard's key could write as every shard. In
version 1 each shard holds its own key, the shard is part of the signed bytes,
and the API refuses a write that names another shard.

The codec is [`coax/web/room/room-write.coax`](../../coax/web/room/room-write.coax).
The room signs with it (`rm_write_mac` in `room.coax`), the API checks with it
(`room_write_signed` in `coax/web/api/rooms.c`, through
`coax/web/api/ticket-sign-unit.c`), and the deploy scripts sign drains with
[`room-write-tool.coax`](../../coax/web/room/room-write-tool.coax). The
fixtures are [`room-write-v1.json`](../../proto/verse/room-write-v1.json).

## Keys

The API holds a write master: 32 random bytes as 64 lowercase hex digits
(`OA_ROOM_WRITE_MASTER`). While a master rotates it holds two, joined by `,`.

| Value | Definition |
| --- | --- |
| Master ID, `KID` | The first 8 hex digits of `HMAC-SHA256(MASTER, "oa-room-write-kid-v1")` |
| Shard S's key | `HMAC-SHA256(MASTER, "oa-room-shard-key-v1\n" S)`, S in decimal, 1 to 99 |
| A shard's `OA_ROOM_WRITE_KEY` | `KID:KEYHEX`: the master's ID, `:`, and the shard's key in 64 lowercase hex digits |

A shard never holds the master or another shard's key.

## Signing

A shard signs

```
"oa-room-write-v1\n" PURPOSE "\n" SHARD "\n" KID "\n" BODY
```

with its key (HMAC-SHA256) and sends:

- `X-Room-Mac`: the MAC in 64 lowercase hex digits.
- `X-Room-Signer`: `SHARD KID`, the shard in decimal without a leading zero
  and the master's ID.

| Purpose | Route | Body |
| --- | --- | --- |
| `vitals` | `POST /api/playtest/room-vitals` | the post's body |
| `checkpoint` | `POST /api/playtest/room-checkpoint-v2` | the post's body |
| `items` | `POST /api/playtest/room-items` | the post's body |
| `directory` | `POST /api/playtest/room-directory` | the post's body |
| `stats` | `POST /api/playtest/room-stats` | the post's body |
| `crash` | `POST /api/playtest/room-crash` | the crash report ([crash reports](../deployment/crash-reports.md), added in #13298) |
| `control` | `POST /api/playtest/room-control` | `t AT`, where AT is the `X-Room-At` header |
| `moderation` | `POST /api/playtest/room-mod-log` | the post's body ([moderation v1](moderation-v1.md)) |
| `drain` | `POST /api/room/sK/drain` (deploy to room) | `drain K AT WAIT` |

A drain goes from a deploy to the shard, so the shard checks it with its own
key and the deploy sends no `X-Room-Signer`.

## Checking

The API derives the key from the master that `KID` names and the shard that
`X-Room-Signer` names, and compares MACs in constant time. A MAC from shard 2
does not check as shard 3's, and a MAC for one purpose does not check for
another. The API then binds the body to the signer:

- Directory: every `s SHARD` line names the signer.
- Stats: every `shard=N` line names the signer.
- Vitals: the `o SHARD GEN BATCH` line is present and names the signer.
- Items, checkpoints: these are per player. Each seat ticket takes the
  player's next owner generation and notes the shard it seats with it
  (`verse.room_owners.shard`, migration 0156). A write whose generation (`gen`,
  `owner`) is the newest or above must come from that shard, or the API answers
  403. A write for an older generation goes on to the fence, which refuses it
  as before (`stale`, `fenced`). Travel, a drain's re-seat and a failover each
  take a new ticket, so the new shard writes and the old one's late writes stay
  fenced. A generation from before 0156 names no shard and is not bound.
- Control: the answer is the realm's, so the signature is enough.

A write without `X-Room-Signer` is checked the old way: the shared key's MAC
over the body alone. The API takes it while `OA_ROOM_TICKET_KEY` is set, and
refuses it once that key is gone. That keeps rooms and the API deployable in
either order. See [room keys](../deployment/room-keys.md) for the rollout.

## Check

```sh
coax/bin/coax test coax/web/room/room-write.coax
coax/web/tool.sh coax/web/room/room-write-tool.coax profile proto/verse/room-write-v1.json
```

The fixtures' keys and MACs were computed with Python's `hmac` module, apart
from the Coax codec, and the codec's checks pin them. The API's end-to-end
checks are in `coax/web/api/tests/room_checks.coax` (`rc_write_keys`).
