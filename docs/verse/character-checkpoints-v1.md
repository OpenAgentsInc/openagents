# Character checkpoints, version 1 (#12838)

Status: receiver only, not activated by any room producer or ticket restore.
This is an additive contract; existing vitals reports retain their own meaning.
It does not yet change what a room crash loses.

`POST /api/playtest/room-checkpoint` takes at most 4096 bytes of JSON, signed
with the room ticket key (HMAC-SHA256 of the exact bytes, `X-Room-Mac`). All
20 members are required; other members, partial objects and other versions
are refused. Integer ranges are enforced by `checkpoint-policy.coax`.

| Member | Meaning |
| --- | --- |
| `v` | Exactly 1 |
| `pid`, `owner` | Player identity and issued character owner generation |
| `process`, `rev` | Positive process identity and revision, each at most 2^53−1 |
| `at`, `sent` | Capture and retry send wall milliseconds |
| `engine`, `rules`, `content` | Positive build identifiers, at most 2147483647 |
| `hp`, `max`, `dead` | Health 0..99999, maximum 1..99999, explicit boolean death |
| `xp`, `wanted` | XP 0..9999999, WANTED 0..5 |
| `room` | Room identifier, 1..24 lowercase ASCII letters/digits/hyphens |
| `x_mm`, `y_mm`, `z_mm` | Signed position millimetres, ±10000000 |
| `yaw_mrad` | Signed heading milliradians, ±1000000 |

`pid` allows 1..40 of the same identifier characters. HP cannot exceed the
maximum; dead requires zero HP. Send time must be within two minutes of the
API clock, and capture time within the preceding 15 minutes of send time.
That retry lease is not a crash-loss bound during an unbounded outage.

The receiver locks the player's identity and the same owner-directory row
that ticket issuance updates. The incoming owner must equal the issued
owner. One owner binds one process; revisions rise within it. A newly issued
owner may start a new process and revision. Accepted XP cannot decrease
within this checkpoint chain. Integration must also reconcile the legacy
XP source before activating the producer.

The complete captured JSON, excluding only `sent`, is compared for retries.
An equal revision with different captured data conflicts. A duplicate keeps
its first acceptance time. The transaction sets `synchronous_commit=on`,
requires PostgreSQL `fsync=on`, and commits before returning any receipt.
This relies on PostgreSQL and its storage honoring their durability settings;
it does not provide independent disk or replica failure guarantees.

The response is `{v:1,status,owner,process,rev,accepted_at}` (JSON with quoted
member names). Only `applied` and `duplicate` acknowledge this exact capture;
all other statuses return `accepted_at:0`: `stale`, `fenced`, `conflict`,
`process`, `unknown`, `unavailable`, `regression`. Transport/database errors
return no acceptance receipt. A response lost after commit is safe to retry.
An old API's missing endpoint cannot acknowledge this contract.

Run the scratch database integration through `coax/web/api/test.sh`; its
`room_checks.coax checkpoints` section covers signature/shape refusal,
lost-response replay, changed-payload conflict, process binding, revision and
XP ordering, takeover fencing, and a separate connection reading the full
committed dead/place/build state. Policy checks precede implementation and
root-only authority mutation must have zero survivors.

The implemented companion [character resume profile 2](../protocol/character-checkpoint-v2.md)
adds the live producer, restore and bounded-loss policy without changing this
inactive v1 contract. Its qualification record is
[character recovery](../verification/2026-11-05-character-recovery.md).

The integration requirements originally tracked for #12838 were:

- Versioned room producer, bounded outbox behavior under a full queue, and
  receipts that acknowledge every required field instead of legacy vitals.
- A signed versioned restore ticket and explicit resume profile covering
  cooldowns, effects and encounter participation, including dead/down state.
- Planned drain that waits for the final accepted capture and reports failure.
- Actual kill/restart before and after commit, lost response, stale-process
  resume, and measured rollback age and restore time. A process-local grace
  reconnect is not durable restore.
- Field-by-field loss and restore objectives, measured under healthy service,
  capped queues and outages. No finite outage loss bound exists until gameplay
  or durable acceptance has an explicit admission/backpressure policy.
