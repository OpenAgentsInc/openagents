# Chapter 103: export, rollback rehearsal and shadow period against relay.openagents.com

Run on 2026-11-05. Every read of relay.openagents.com went through the public
Nostr protocol (REQ and COUNT over wss://), as any client can. Nothing read the
relay's database, its backups or its host, and nothing wrote to it.

## Export

`python3 coax/tests/nostr/relay_live_cutover.py export OUT` ([log](export.log)):

| | |
| --- | --- |
| Lines | 12,278 |
| Bytes | 64,881,789 |
| SHA-256 | `d4696a95dbf30302223e2f6a60a361479a2657191bbaa7060c08cba64020cd2c` |
| Newest `created_at` | 1793849370 |
| COUNT `{}` | 12,279 |
| Left out | 1 event of kind 31990 (`created_at` 1790162811), below |

The first export attempts read pages of 127. Four of them reached one stored
kind 31990 (a NIP-89 handler) at `created_at` 1790162811. The Rust relay's own
read check now refuses it ("a platform URL contains the bech32 placeholder"),
and its database worker treats that as fatal: the process exits and every
connection drops, at 13:26, 13:27, 13:30 and 13:34 UTC. That is filed as
[#12909](https://github.com/OpenAgentsInc/openagents/issues/12909). COUNT reads
ids only and is safe, so the export finds every 31990 by COUNT bisection (one
event, in one second) and reads around that second with pages of 50. The
export holds every other event an anonymous reader gets: 12,278 of COUNT's
12,279.

The export doesn't hold events that only an authenticated reader sees (gift
wraps, OpenAgents' private kinds). Those need the backup restore in the
runbook's steps 4 to 6.

## Import

`relay --import` on the Mac: 12,278 lines read, verdict clean, every signature
verified (3.5 s of 4.5 s), 11,493 regular, 80 replaceable and 705 addressable
events, 6 deletion requests. Served set at 1793849370: 12,278 events, digest
`e25ff6aa27792e3e22063c6c97cf6dda9c50eb71cf10524b2250e7b7d7624a89`.
`--check-store` printed the same served-set line.

## Rollback rehearsal

`coax/tests/nostr/relay_rollback_rehearsal.py` inside
`coax/tests/nostr/rust_relay_sandbox.sh run` on coderos-4080 (no network but
loopback). The Rust relay is the one relay.openagents.com runs
(`crates/nostr-relay` at `0b401561a0^`). [result](rehearsal-result.json),
[log](rehearsal.log):

1. Both relays import the export: the Rust relay with `import-jsonl`
   (12,278 stored), the Coax relay with `--import`. Both serve the same 12,278
   ids (SHA-256 of the sorted ids `f33ac519...` on both).
2. Cutover on a staging name (a proxy on its own port): pointed at the Coax
   relay, a client authenticates (NIP-42), writes three events and reads them
   back through it.
3. Rollback: `relay --export --from BYTE` (BYTE the store's size at the
   cutover) exports exactly those three; the Rust relay imports them with
   `import-jsonl`; the proxy points back at the Rust relay; the client reads
   the three and writes and reads one more.
4. Final equality: the Rust relay serves the Coax store's 12,281 events plus
   exactly the write after the rollback.

Passed.

## Shadow period

`relay_live_cutover.py shadow` on the Mac, from 07:48 to 08:36 CST
(1793886508 onward), [shadow.jsonl](shadow.jsonl): a Coax relay started from the import on 127.0.0.1:18931; every new
live event (from the export's newest `created_at` on) is mirrored into it;
every 10 minutes the same twelve REQs go to both relays and each answer pair
is classified match, order, limit or unexplained.

Five rounds (60 answer pairs): 60 match, 0 order, 0 limit, 0 unexplained;
6 new live events mirrored, none refused. The runbook asks for a week of
normal traffic before the cutover (step 12); this run shows the comparer
working against the live relay, and the agent doing the cutover runs it again
for that week.

Writes don't go to both relays through a proxy in front of production: that
changes production routing, which is the cutover's deploy. The mirror stands
in for it.

## Left for the production deploy

Step 14 (the Rust relay read-only), step 15 (relay.openagents.com pointed at
the Coax relay) and step 16 are production changes, in `NEEDS_OWNER.md`
"Coax relay cutover".
