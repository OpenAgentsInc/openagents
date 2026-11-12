# relay.openagents.com is the Coax relay (2026-11-12)

The cutover of [#13361](https://github.com/OpenAgentsInc/openagents/issues/13361),
by chapter 103's runbook. Times are Chicago (CST, UTC−6) with Unix seconds
where a command takes them. Production's database was never read: every
export came from a backup restored into a throwaway Cloud SQL instance.

| | |
| --- | --- |
| Cutover | 15:43:15 (1794519795): Cloudflare record `560991ecfda6f8080d48a00198c64964`, `relay.openagents.com`, from CNAME `ghs.googlehosted.com` to A `34.45.121.213`, TTL 60, not proxied |
| Serves it | the Coax relay from `nostr/relay/`, binary `relay-2e8b947ecece`, on the VM `oa-relay-1` (e2-small, us-central1-a) |
| Fallback | the Rust relay, Cloud Run service `openagents-nostr-relay`, unchanged and running at `wss://openagents-nostr-relay-ezxz4mgdsq-uc.a.run.app`; its domain mapping for relay.openagents.com is still in place |
| Rollback | put the record back to CNAME `ghs.googlehosted.com`, then carry the Coax relay's writes since the cutover to the Rust relay ([below](#rollback)) |

## Before the cutover

The earlier steps (the move to `nostr/relay/`, NIP-11, the first export and
import, the VM, 256 connections, the 7-day retention of kind 3188 and the
rollback rehearsal on relay-staging.openagents.com) are in the issue's
comments. The VM had been stopped since 12:48, so its store was behind.

### Catch-up and equality

`nostr/relay/export.sql` gives every row; `nostr/relay/export-delta.sql` gives
the rows after an `ingest_seq`; `relay_live_cutover.py publish` sends those
to the Coax relay over one NIP-42 connection. The oracle is
`relay_export_served.py` (Python, none of the Coax store's code): the export
minus what NIP-40 and the 7-day rule for kind 3188 have expired at `--now`.
The Coax side is `relay --check-store` at the same `--now` on a copy of the
live store and its checkpoint (the running relay holds the lock on the
original). `--now` was set 30 minutes ahead each time, so the Pylon's beacon
(kind 30200, which expires 4 minutes after it is made) is out of both sets.

| | First catch-up | Second, right before the cutover |
| --- | --- | --- |
| Backup of `khala-sync-pg` (on demand) | `1794510495518`, 13:08 | `1794519112832`, 15:31 |
| Export lines | 601,657 | 602,080 |
| Export SHA-256 | `b4d3b6b12452b2a71cb8a99828c3617b65179b10026f4bc3f27223c25c3d1372` | `51c131f929a8c0693f419afc656863b400f90c145b572648cd62b85fdb9af8d3` |
| Newest `ingest_seq` | 779737 | 780303 |
| Private or hidden NIP-29 groups, ban and allow lists | 0, empty | 0, empty |
| Delta | 1,595 rows after 777601: 1,593 kind 3188, one 10002, one 30200 | 424 rows after 779737: 423 kind 3188, one 30200 |
| Published | 1,593 stored, 1 duplicate, 1 refused | 423 stored, 1 refused |
| The refused line | beacon `44aadb53…`, already expired | beacon `3860b67f…`, already expired |
| `--now` | 1794513104 | 1794521386 |
| Oracle | 37,615 events, `a30ecf8df840deb5d3e700d6eb8a73b1cf69e59f528fceb18782227a973e7bc7` | 37,633 events, `c4d3d6b081840c5178410e06af7641d8e9561987135c0cda1f4fef26ff6095cc` |
| Coax relay | 37,615 events, the same digest | 37,633 events, the same digest |
| Result | **equal** | **equal** |

The second check ran after the shadow period ended, so nothing but the
catch-up wrote to the Coax relay between the backup and the check. The store
file was 111,803,475 bytes at the cutover; that is `--from` for a rollback's
export.

### Shadow period

`relay_live_cutover.py shadow` on the VM, from 13:10:23 to 15:30:23
(1794510623 to 1794519023, 8,400 s): every event the Rust relay delivered to
an anonymous subscription was sent to the Coax relay, and every 5 minutes the
same 14 REQs went to both. The log is [shadow.jsonl](shadow.jsonl); the
round totals are [shadow.out](shadow.out).

| | |
| --- | --- |
| Rounds | 28 |
| Answer pairs | 392: 387 match, 5 differ |
| Events mirrored | 144 (143 Pylon beacons, one kind 10002), none refused |
| Reconnects to the Rust relay | 3 |

The five differing pairs are all in the first 10 minutes, and each is
explained:

| Round | Filter | Difference | Why |
| --- | --- | --- | --- |
| 1 (13:10:26) | `{"limit":50}` | `dbe5c8de…` only on the Rust relay, `80ee5d4c…` only on the Coax relay | Both are the Pylon's beacon (kind 30200, one address). The round ran before the mirror delivered anything: the Coax relay still had the beacon from before the VM was stopped. The mirror stored `dbe5c8de…` 27 s later |
| 2 and 3 (13:15, 13:20) | `{"limit":50}` | `c8a53856…` only on the Coax relay, `604644a7…` only on the Rust relay | `c8a53856…` is this run's own check note (`rw`, kind 1, expiring after 10 minutes), written to the Coax relay at 13:14. It pushed `604644a7…` out of a 50-event answer |
| 2 and 3 | `{"kinds":[1],"limit":50}` | `c8a53856…` only on the Coax relay | The same note |

Rounds 4 to 28 (13:25 to 15:30, 2 hours 5 minutes, 350 pairs) match
completely. No check note was written to either relay in that time.

### Relay tests on this tree

On the build pool (`coax/remote/remote.sh run`), all under ASan and UBSan:
`relay_session.py` ok, `relay_differential.py` ok, `store_crash.py` ok (95
acknowledged of 120 sent), `coax/tests/net/tls.sh` ok, and
`relay_flood.py --sanitize` ok (532 s, peak 439.5 MiB, no sanitizer report).
The session tests sign with `relay_sign.ts`, which needs `@noble/curves`
beside the checkout on the host (`bun add @noble/curves@1.8.1
@noble/hashes@1.7.1` in the slot's folder).

## After the cutover

Public resolvers (1.1.1.1, 8.8.8.8) served the new address 5 s after the
change; this Mac's resolver took about 5 minutes (the old record's TTL).

| Check | Result |
| --- | --- |
| NIP-11 at `https://relay.openagents.com/` | "OpenAgents Relay", `supported_nips` 1, 9, 11, 17, 40, 42, 45, 50, 59, 62, 70, 77; `max_message_length` 131072 |
| `relay_live_cutover.py rw --url wss://relay.openagents.com` | AUTH true, a 10-minute note stored (`46bc77e3…`), read back: ok |
| `relay_live_cutover.py pages wss://openagents-nostr-relay-ezxz4mgdsq-uc.a.run.app --url wss://relay.openagents.com` (the website's subscriptions, compared with the fallback) | AUTH challenge on connect; 16 projects, 80 REQs, each ended with EOSE and equal to the fallback's answer: ok |
| `openagents nostr publish --content … --relay wss://relay.openagents.com` (a throwaway profile, a 10-minute note) | `auth 1`, `ok 1` |
| The Pylon (chapter 24) | Its beacon (kind 30200, key `1803fb32…`) on the Coax relay was 21 s old at 15:50, after the mirror had stopped: the Pylon writes to the Coax relay itself |
| Nostr login on openagents.com | `POST /api/auth/nostr/challenge` 200, then `POST /api/auth/nostr` with a NIP-98 event from a throwaway key: 200 with a session cookie; logout 200. (The login doesn't use the relay) |

One `rw` at 15:44 reached the Rust relay through this Mac's stale resolver
(its NIP-11 listed 16 NIPs); its note `3c8370f6…` expired 10 minutes later.

### Writes the Rust relay took after the cutover

Cloud Run ends a WebSocket after 3,600 s, so a client connected before the
cutover could keep writing to the Rust relay for up to an hour. At 16:46, an
hour and three minutes after the cutover, a third backup (`1794523583705`)
was restored and `export-delta.sql` run after `ingest_seq` 780303:

| | |
| --- | --- |
| Rows | 46, all kind 3188 (SHA-256 `10347202689e65ac79c2474294d8cd9e06df27c3e1d581e9faee34d75f6f093c`) |
| Written after the cutover | 13, the last at 15:47:46 (1794520066), 4.5 minutes after it |
| Published to wss://relay.openagents.com | 45 stored, 1 duplicate, none refused |

The Rust relay took no write in the 58 minutes before that backup. The Coax
relay's store file grew from 111,803,475 to 112,325,844 bytes in the same
hour: the host envelopes now arrive there.

### Certificate

`nostr/relay/vm.sh cert-renew-by-http` issued the certificate again by
HTTP-01 for relay.openagents.com and relay-coax.openagents.com (expires
2027-02-10), without restarting the relay. `certbot.timer` renews it alone
from now on; each renewal restarts the relay.

## The three known issues

**Event size: not raised, and here is why.** The Coax relay refuses an event
with a string over 64 KiB ("invalid: malformed event"); the Rust relay took
events up to 131,072 bytes. The bound isn't the relay's. It is the shared
event profile's: `JSON_MAX_STRING` in `coax/lib/nostr/json.coax`, the `raw`,
`decoded` and `content` limits in `adapters/json-policy.coax` (`#authority`)
and the decode buffers in `event.coax` and `event_sign.coax` (`#authority`).
Every Coax Nostr client, the engine and the game load those files, and
65536 appears in 93 files under the Nostr tree. Raising it grows the decode
buffers on every reader's stack, the browser's included, and belongs to the Nostr
library's own change with its own mutation runs, not to a relay cutover. No
stored event needs it: the restored copy holds 505 events with content over
60,000 bytes, all kind 3188, the newest from 2026-10-09 (so all past 7 days
and not served); the largest content of any other kind is 54,704 bytes, and
the largest 3188 of the last 7 days is 1,456 bytes. Neither catch-up nor the
shadow period had a refusal for size. Follow-up: [#13382](https://github.com/OpenAgentsInc/openagents/issues/13382).

**Restart downtime: about 2 minutes, up to 4.5.** A start replays the store
and verifies every signature again; nothing is served until it ends. Today:
116.9 s (36,380 frames, after the VM's boot), 114.8 s and 177.2 s (the two
`--check-store` runs beside the live relay), and 269.7 s once, at 12:22. The relay restarts only for a new
binary, a renewed certificate (every 60 days) or a crash; a VM host
maintenance event migrates it live. Nothing cheap shortens the replay: it is
signature checks on a shared-core e2-small. `cert-renew-by-http` no longer
restarts it. Follow-up: [#13382](https://github.com/OpenAgentsInc/openagents/issues/13382).

**Fallback: running, writable, not read-only.** The Rust relay has no
read-only switch, so it is left as it was. It is no longer reachable by
relay.openagents.com; a client that names the Cloud Run address can still
write to it, and those writes would be found by the same delta export.

## Rollback

1. Put Cloudflare record `560991ecfda6f8080d48a00198c64964` back:
   `relay.openagents.com` CNAME `ghs.googlehosted.com`, not proxied. The
   Cloud Run domain mapping was never removed. The rehearsal took 71 s to
   propagate.
2. Carry the Coax relay's writes to the Rust relay: on the VM, stop the
   relay and run `relay --export OUT --from 111803475 --store
   /var/lib/nostr-relay/store/relay.store --checkpoint
   /var/lib/nostr-relay-cp/relay.cp` with the service's other flags
   (chapter 103, step 19), then
   `AUTH_URL=wss://relay.openagents.com python3 coax/tests/nostr/relay_live_cutover.py publish OUT --url wss://relay.openagents.com`
   once the name is back on the Rust relay. Kind 3188 host envelopes are
   about 4,200 a day; the rest is the Pylon's beacon each minute and a few
   other events.

## Resources (project openagentsgemini)

| Resource | Change | Cost a month |
| --- | --- | --- |
| Cloudflare record `560991ecfda6f8080d48a00198c64964` (`relay.openagents.com`) | CNAME `ghs.googlehosted.com` to A `34.45.121.213`, TTL 60 | $0 |
| VM `oa-relay-1` (e2-small) | started; deletion protection on; `nostr-relay.service` enabled at boot | $12.23 |
| Disks `oa-relay-1` and `oa-relay-1-data` (10 GB each) | unchanged | $2.00 |
| Address `oa-relay-1-ip` (34.45.121.213) | in use | $3.65 |
| Snapshot schedule `oa-relay-1-hourly`, bucket `gs://openagentsgemini-relay-checkpoint` | unchanged | a few cents |
| Let's Encrypt certificate on the VM | issued again for relay and relay-coax.openagents.com | $0 |
| Cloud SQL backups of `khala-sync-pg` `1794510495518`, `1794519112832`, `1794523583705` (on demand) | created; left in place | cents (they share blocks with the daily backups) |
| Cloud SQL `relay-export-restore-2` (throwaway, the restored copies) | created, then **deleted** | about $1 in all (3.8 hours) |
| Cloud Run `openagents-nostr-relay`, its database `nostr_relay_v2` and its backups | **unchanged** | as before |

The Coax relay costs about $18 a month. The full exports on the VM's
data disk and the throwaway instance's credentials were removed after the
last catch-up; the deltas, counts, digests and logs stay in
`/var/lib/nostr-relay/export/`.
