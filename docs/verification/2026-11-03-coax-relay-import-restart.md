# Coax relay: durable import, restart, freshness checkpoint and the refreshed differential

2026-11-03, issue #12631 (the parts that need no production access). Run on
coderos-4080 under other agents' load (load average 65 to 100 on 28
cores), with TEST-ONLY keys and synthetic events. Nothing contacted
relay.openagents.com, read production data, used a secret, changed DNS or
ran on another machine. Parity: [2026-11-03-coax-relay-parity.md](2026-11-03-coax-relay-parity.md).

## What was built

- `coax/lib/nostr/store_fresh.coax` (`#authority`): the freshness
  checkpoint. One 187-byte line holds the `--store` file's and its
  `FILE.nip86` policy journal's length and SHA-256. Against it, a file is
  fresh (it starts with the checkpointed bytes), rolled back (it is
  shorter: an older copy, or deleted) or forked (as long, other bytes). A
  start without a checkpoint is a first start (empty files), adopted
  (`--adopt`) or refused; an unreadable checkpoint is always refused. A new
  checkpoint never moves back. 8 checks; `coax mutate`: 211 mutants, 211
  killed.
- `coax/lib/nostr/relay_import.coax` (`#authority`): two outcomes a
  deleting import adds: a line an earlier deletion or vanish covers
  (refused: the old relay kept it) and no room for a deletion's tombstones
  (incomplete). `coax mutate --root-only`: 90 mutants, 90 killed.
- `coax/relay/durable.coax` and `relay-host.c`: `relay --import EXPORT
  --now T --url U --store FILE --checkpoint CP`, `relay --check-store`, and
  `--checkpoint` on a serving relay (checked before replay; rewritten after
  every event commit and every NIP-86 journal commit: `CP.tmp` written,
  synced, renamed, the folder synced). The import admits through
  `rd_admit` (deletion and vanish applied), writes the store file without
  a sync per event (`cxstore_bulk`), syncs it once, and writes the
  checkpoint last, so a crashed import is refused on its next start (no
  checkpoint). Both modes print the served set's size and chapter 103's
  digest, and their time and peak memory.
- `coax/tests/nostr/relay_scale.coax` (a Coax tool) and
  `relay_scale_oracle.py` (Firm reason 2, an independent oracle): the
  oracle plans an export (regular notes, some with escapes and non-ASCII
  text; replaceable and addressable versions and stale ones; duplicates;
  expired and expiring events; NIP-09 by id, by address and for absent
  events later sent; one NIP-62 vanish; forged signatures), signs it with
  `relay_sign.ts` (@noble/curves, 16 test keys in parallel), and decides
  every line's outcome and the served set with its own rules. The tool
  imports, checks each expected count and the digest in the relay's
  output, restarts with `--check-store`, and checks three restores the
  checkpoint must refuse: the store's first half, the store with one byte
  changed, the store without its checkpoint.

## Scale runs

`coax/web/tool.sh coax/tests/nostr/relay_scale.coax --dir DIR --events N`.
Every run: every outcome count and the served-set digest equal the
oracle's, the restart's served set is identical, and all three restores
are refused (exit 3) with the files unchanged.

| Lines | Export bytes | Stored records (live) | Tombstones | Import (verification) | Import peak memory | Restart (replay, every signature again) | Restart peak memory |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 3,000 | 1,175,017 | 2,870 (2,419) | 34 | 2.4 s (2.0 s) | 31 MiB | 1.9 s | 30 MiB |
| 30,000 | 11,780,801 | 28,560 (22,954) | 246 | 74.9 s (27.5 s) | 69 MiB | 71.3 s | 57 MiB |
| 60,000 | 23,577,661 | 57,176 (44,488) | 478 | 312.9 s (63.5 s) | 113 MiB | 253.7 s | 89 MiB |

The 30,000-line outcomes: 28,560 stored, 365 duplicates, 303 superseded,
315 forged (bad signature), 434 expired by `now`, 23 covered by an earlier
deletion (sent after their deletion; refused for the owner); 245 deletion
requests and one vanish request retired 1,132 records.

### The finding: admission is linear in the store's size

Verification is about 0.5 to 1 ms an event under this load, and grows
linearly. The rest does not: `ns_store_plan` (`store_core.coax`) scans
every record for the event's id and its address's head, so each admission
costs O(records), and an import or a restart of N events costs O(N^2).
"Store and file" time went from 0.2 s at 3,000 to 47 s at 30,000 and 249 s at 60,000 (5.3 times for twice the events). At
250,000 events a restart would take most of an hour. Before a production
event count of that order can cut over, admission needs an id and address
lookup (a hash or the sorted index chapter 36 already keeps), with
`ns_store_plan`'s decisions unchanged and its mutation run kept at zero.
This is engineering work, not an owner decision; it is open on #12631.
The tombstone table is also fixed at `RD_TOMBS` = 8,192 (the import says
"incomplete" past it) and each admission scans it.

## The refreshed differential

`coax/tests/nostr/rust_relay_differential.sh` (SCRATCH holding the Rust
relay built from `0b401561a0^` in the owner-approved sandbox on
2026-10-30, sha256 `6704c1b3...`, no network, a throwaway Postgres 17.11).
Both relays had the test key as their NIP-86 admin. The driver gained an
R2 session for the operations wired since R1: COUNT, search, NIP-09 by id
and by address (and the deleted event sent again), deleting another key's
note, NIP-62, NIP-70 from the author, another key and without AUTH, gift
wrap and app-data reads by someone else, NIP-77 syncs through
`negentropy_ref.py` (equal, the client lacking one, the client with two
extra) and their refusals, and NIP-86 with the u tag as ws:// and as
http://.

Result: 163 observations; 117 match, 21 same verdict and prefix with other
text, 13 intentional, 12 for the owner, 0 unexplained; ASan/UBSan clean.

Every COUNT, NIP-09, NIP-62 refusal path, NIP-70 and NIP-77 sync agrees
in verdict. The owner's differences, beside R1's four:

| Observation | Rust | Coax |
| --- | --- | --- |
| REQ gift wrap, not its recipient | CLOSED "auth-required: gift-wrap reads require recipient authentication" | the gift wrap |
| REQ app data 30078, not its author | nothing | the event |
| search "n1" | substring: n1, n10 to n19 | whole words: n1 |
| NIP-86 with u = ws://127.0.0.1:PORT | 401 | 200 |
| NIP-86 with u = http://127.0.0.1:PORT/ | 200 | 401 "unauthorized: the u tag is not this URL" |
| supportedmethods | 9 methods, no event bans | 14 methods |

Intentional: NIP-11's lists; NEG-ERR (Coax) where Rust sends CLOSED for a
bad NEG-OPEN; Coax syncing up to 100,000 events where Rust refuses a sync
larger than one REQ; `restricted:` (Coax) against `auth-required:` (Rust)
for a protected event from another authenticated key; NIP-62.

## Also run

- `relay_session.py` (ASan/UBSan): ok, with a new `checkpoint` case: a
  first start, the checkpoint equal to both files' sizes and SHA-256 after
  an event and a NIP-86 change, a fresh restart, an emptied policy journal
  refused before listening (exit 3), and fresh again once restored.
- `nostr/examples/checks/relay-cutover.coax` (384 checks), the example,
  `relay_import_run.coax` (353 checks): ok.

## Not run

Anything against relay.openagents.com or its database, the restored-backup
export, the shadow period, the rollback rehearsal and the cutover: owner
actions in NEEDS_OWNER.md ("Coax relay cutover").
