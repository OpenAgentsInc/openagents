# Character checkpoint and resume profile 2

This companion to Everglade's gameplay wire adds durable character recovery.
It preserves checkpoint-v1 and seat tickets v2/v3. Schema:
`proto/verse/character-checkpoint-v2.schema.json`.

`POST /api/playtest/room-checkpoint-v2` is a room-key HMAC-SHA256 signed JSON
capture, at most 2048 bytes. Its 20 base members have checkpoint-v1's meanings
and ranges, except `v` is 2. Required member `state` is an array of exactly 27
unsigned integers: class (0 druid, 1 warlock), demo (0/1), temporary HP,
Repair cooldown ticks, cast cooldown ticks, elapsed down seconds, 10 spell-slot
counts and 11 limited-use counts. Temporary HP is at most 99999, cooldowns at
most 18000 ticks, down seconds at most 9, resources at most 99. Captures include
the complete state; retries change only `sent`. Retries have no expiry.
`sent` must be within 120 seconds of the API clock; `at` must not follow it.

The receiver locks canonical character identity and the owner directory row,
checks the exact issued owner/process/revision and the XP floor across legacy
vitals and both checkpoint versions on owner transfer. Within one owner the
checkpoint XP chain is monotone; a newer asynchronous legacy XP report does not
invalidate an immutable older capture. Ticket issuance takes the maximum XP
across all three stores. The receiver commits with synchronous_commit and
fsync on before returning `{v:2,status,owner,process,rev,accepted_at}`.
Only `applied`/`duplicate` acknowledge the exact capture. A duplicate retains
its first acceptance time. Status names match checkpoint-v1. Legacy receipts,
missing fields, HTTP success alone and a different capture cannot acknowledge it.

Seat ticket v4 is `v4.CORE64.CAPTURE64.MAC`. CORE64 is unpadded base64url
of a complete signed legacy v2/v3 seat ticket, including its own MAC and
the new positive owner generation. The room validates both signatures.
CAPTURE64 is unpadded base64url of the last accepted complete v2 capture JSON
(without `sent`), or `-` for a character without one. MAC covers the exact
prefix before `.MAC`. It binds the newly issued owner, destination and entire
restore capture. The room rejects incompatible saved build IDs. Tickets are
at most 4096 bytes. A v4 ticket's saved-data schema is 2; old rooms refuse it.

Fresh-process recovery restores HP, maximum constrained to the actual build,
explicit down/dead state, temporary HP, cumulative XP, WANTED and resource
counts constrained to their build maxima. Place returns only in the captured
room, to millimetre precision; subsequent native movement resolves the ground.
Cooldowns and the down timer resume from their captured values, without offline
credit. World-bound conditions, concentration, zones, summons, in-flight casts,
death-save rolls and encounter contribution end with the lost world. A fresh
town/raid follows its ordinary start rules. This is not a town/raid checkpoint
or a transferable item/value ledger. Same-process grace keeps the live body.

Each admitted v4 character reserves one bounded outbox entry before it can
play. Unsaved entries for the current owner are never evicted or expired;
a newly issued owner supersedes the fenced owner's unsent captures. One immutable capture is
in flight; a separate latest capture coalesces changes until that receipt.
Initial gameplay waits for its first durable acceptance. Thereafter an
encounter pauses before advancing more than 30 seconds beyond its oldest
accepted capture, and takes no new mutating intents while paused. It keeps
retrying. Thus the loss bound is 30 seconds of authoritative gameplay, plus
at most one simulation tick at a cutover; wall-clock checkpoint age can grow
without bound while paused. A planned drain freezes and captures final state,
and succeeds only after the final checkpoints are accepted. Forced termination
reports remaining unsaved characters and follows the same loss bound.

Healthy-service objectives: capture every 5 seconds, and restore to the first
authoritative socket state within 60 seconds, with matching builds, at most
256 reserved characters, successful API responses within 100 ms and the host
being scheduled. These are separate from browser presentation and store/disk
or regional-disaster guarantees. Fault verification records measured loss,
acceptance and recovery time; an unavailable store has no finite recovery-time
objective. Deployment remains an owner action; tests use scratch stores only.

Activation is explicit: the page asks `checkpoint=2`, and the API issues v4
only with `OA_ROOM_CHECKPOINTS=1`. Its profile is sticky from the first issued
v4 ticket, before the first capture (`verse.character_profiles_v2`): an
older page or disabled flag fails closed with a recovery-unavailable answer.
The operator must first put schema-2 rooms on every eligible destination;
rolling back does not discard or translate a newer capture. Account linking
carries the latest accepted body to the account and advances both ownership
fences. No checkpoint row expires because the player was absent.
