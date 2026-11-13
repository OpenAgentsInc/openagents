# Coax relay after the cutover (#13382)

The owner approved file-backed relay custody and the Rust relay's private-kind
write checks on 2026-11-13, answering “Approve both”. In a separate reply,
the owner answered “Approved” to the named Rust service, database and
cutover-backup retirement request. That retirement completed the same day.

## Behavior

NIP-11 reports `max_content_length: 65536` alongside the 131072-byte frame
limit. The shared parser's bound is unchanged. Checkpoint-covered frames
skip signature verification only after the complete locked prefix hashes to
the trusted checkpoint; uncovered frames and offline check/export/compaction
still verify signatures. SIGHUP replaces a validated TLS certificate/key
context; existing TLS connections retain their context. A bad replacement
keeps the last good context.

Rate tracking has 4,096 key slots. Sixteen simultaneous connections per actual
IPv4/IPv6 address leave the 256-slot global pool available to other peers.
Weekly compaction fully checks the old generation, drops expired ordinary
events, retains replacement/deletion/control history, checks the candidate
and compares the served-set digest at one fixed clock. One atomic pointer
selects its store, journal and boot-disk checkpoint. Current, previous and
legacy initial generations are retained; unknown files are never pruned.
The systemd job restarts the relay on failure and has a twelve-minute limit.

The key file is `nostr.relay-key.v1`: 32 raw bytes, owned regular file,
0400/0600, no symlinks/FIFOs, no startup generation or replacement. The
shared signer admits the scalar. The host locks the owned key, disables core
dumps and wipes its key/signing workspace. `--identity` prints only the
x-only public key. Migration reuses the existing Rust relay identity rather
than changing the relay's identity.

With a key and `--groups`, NIP-29 publishes independently verifiable
39000–39003 state. Current membership gates history, search, live delivery,
COUNT and synchronization. Moderation deletes an event only within its
authorized group. Deleted group names remain reserved. A control reserves its original plus four complete 16 KiB signed output
frames before committing. Relay-only state cannot be submitted by a user. NIP-65 replacement, NIP-67 completion hints
and NIP-94 metadata validation are wired. NIP-98 protects the NIP-86 POST
endpoint when an explicit `--admin` key is configured; 86/98 are advertised
only with that role.

Private writes preserve the former Rust author/authentication, run/head,
artifact, private-policy and relay-only-kind rules. The artifact route uses
the shared curve reader and NIP-44 v2 envelope gate. Ciphertext validation
checks only its public envelope; it does not decrypt or authenticate it.

## Eleven configured extensions

| Extension | Relay role |
| --- | --- |
| NIP-MP | Project outer envelope, bounded unique repository coordinates; grouping grants no repository authority |
| NIP-OA | Original signed owner proofs; current agent profile can prove an owner relation |
| NIP-RS | Private addressable read states and capped newest-first enumeration |
| NIP-AA | Open-membership NIP-42 authentication with optional verified owner attestation |
| NIP-AE | Bounded encrypted engram envelope and author/recipient reads |
| NIP-AM | Authenticated agent, exact owner/agent route, proved relation, encrypted envelope and owner-only read |
| NIP-AO | Ephemeral encrypted telemetry/control, proved relation, fresh route and per-IP/per-agent 100/s limits |
| NIP-AP | Persona/team coordinates, shared flag and private/shared reads |
| NIP-ER | Private encrypted reminders, bounded future horizon, withheld until due, live delivery on the due tick |
| NIP-DV | Current DM membership, viewer-owned hide/open commands, relay-signed 30622 replacement state |
| NIP-IA | Consent/admin/owner decisions, relay-signed archival deltas and public 13535 snapshot; replay trusts the original relay decisions |

This is an open-membership relay. NIP-43 closed membership is not advertised.
An owner attestation records a connection relation; it does not add an owner
to reader keys or inherit group/admin rights. Unsigned ownership projections
from the former Rust database are not imported. Clients can supply a current
signed agent profile or reattest on authentication. NIP-IA retains the shared
32-identity state bound; NIP-DV retains the shared 16-hidden-channel bound.

## Monitoring and rollback

GCP uptime check `coax-relay-nip-11-and-tls-fYNYUFRBICs` checks HTTPS/443,
validates TLS and requires the NIP-11 relay name every 60 seconds from Iowa,
Europe and Asia. Availability policy `7984740570427026797` alerts after
120 seconds; certificate policy `9580676075493840699` alerts below fourteen
days. Both use the existing owner notification channel
`14067723742793378411` in project `openagentsgemini`. The checked-in JSON
files under `nostr/relay/monitoring/` describe these resources.

Cloud Run `openagents-nostr-relay`, database `nostr_relay_v2` and the three
cutover backups were retired with owner approval on 2026-11-13, as recorded
below. Recovery uses the retained Coax binary, store generations, data-disk
snapshots and freshness-checkpoint copies. Never pair a store with another
generation's checkpoint.

## Rust fallback quiet-period evidence

A read-only PostgreSQL metadata query at 2026-11-13 22:14:25 UTC found
602,125 events, maximum ingest sequence 780366, and last `received_at`
2026-11-12 21:47:46.065713 UTC. There were zero rows received after
2026-11-12 22:46:23 UTC, the final cutover backup. No event payloads or
credentials were printed. This metadata check preceded the separate owner
retirement approval and was repeated immediately before deletion.


## Rust fallback retirement

The owner answered “Approved” to the request explicitly naming Cloud Run
`openagents-nostr-relay`, database `nostr_relay_v2` in `khala-sync-pg`, and
cutover backups `1794510495518`, `1794519112832`, `1794523583705`.

A metadata query at 22:53:40 UTC on 2026-11-13 found the same 602,125 events
and sequence 780366. After deleting the Rust service, a second read-only
query at 22:54:52 UTC again found last receipt at
2026-11-12 21:47:46.065713 UTC and zero events received after the final
cutover backup. There was no new delta to import.

The Cloud SQL deletion API directed this application-owned database to its
SQL owner. A catalog query confirmed owner/current user `khala_app` and
zero remaining target-database sessions. `DROP DATABASE nostr_relay_v2`
then succeeded through a connection to `postgres`. The three named backup
runs were deleted individually through Cloud SQL.

API read-back at 22:57:57 UTC lists no matching Cloud Run service, database
or backup runs. The shared `khala-sync-pg` instance remains RUNNABLE. The
Coax relay's public NIP-11 response still has its existing public identity
and all eleven extensions; the service has zero automatic restarts, its
checkpoint upload succeeds, and weekly compaction is active. The next
scheduled run is 2026-11-15 09:10:19 UTC.

See [the final metadata and deletion read-back](retirement-readback.txt).
The earlier cutover record and chapter 103's Rust rollback steps describe
historical state; current recovery uses the retained Coax generations.

## Verification

The complete local TLS session against an ASan/UBSan relay passes, including
metadata/private writes, authenticated NIP-29 controls and snapshots, all
eleven extensions, restart replay, checkpoint-covered corruption refusal,
compaction and generation pruning, and certificate replacement while an
existing WebSocket remains connected. The final checked `-O2` deployment
binary independently passes the extension sessions with Noble-signed events.

New/changed scoped authorities kill every mutant: connection limits 34/34,
rate policy 215/215, checkpoint freshness 223/223, compaction 34/34, key
policy 50/50, metadata 10/10, group read policy 61/61, private reads 580/580,
deletions 478/478, store core 492/492, private writes 137/137, extension
policy 75/75, block ingest 218/218, agent metrics 44/44, NIP-44 envelope
19/19, shared Unicode space/control 46/46, shared tag reader 560/560, reminder
outer envelope 114/114. Existing route/server/message units kill
914/359/205 mutants respectively. These are scoped results, not an assertion
that every imported cryptographic unit has zero historical survivors.

The relay policy suite and all touched Nostr suites pass. The two books'
structure and links pass. Wire, layer, component and tools reports retain
the existing allowlist counts; no new allowance or equivalent mutant was
introduced.

Deployment binary SHA-256:
`1b42be622f67c6a3490fea52618349d82910db46446781989947b973a0101345`.
Maintenance binary SHA-256:
`961ed1e19f4239db4765babd7c496eeb6787593de9e73eb494d8eb836b6aa2b0`.
The relay links Debian's libssl.so.3/libcrypto.so.3 and glibc; maintenance
links glibc. Neither requires a Nix store path on the production VM.

The first production maintenance attempt exposed a real-clock integration
bug: `proc_run` strips the trailing newline, so removing another byte cut a
timestamp digit. The fail-safe retained the initial generation and restarted
the relay. A scheduler-form test without `--now` now checks the actual clock.
Maintenance verifies against a private checkpoint copy, so root-run checks
cannot replace the previous generation's checkpoint or change its ownership;
an inode/ownership integration check covers this. Failure output and the
fixed-clock served-set digest are now logged.

Production rollout at 2026-11-13 22:29 UTC reused the Rust relay's existing
public identity `e841147f262799821bbaa2930fcca982a575458f0e043e064a26ed8aba2046ed`.
The raw 32-byte key is owned by `nostr-relay`, mode 0400; no key was printed.
A 121,659,783-byte, 44,389-frame store replay took 18,350 ms; HTTPS became
available 25,735 ms after restart. The former binary's 36,380-frame replay
had taken 116,884 ms. This is an observed operational comparison, with a
larger and later store, not a controlled CPU benchmark. A public WSS upgrade,
AUTH challenge and read-only repository REQ/EOSE pass. NIP-11 advertises the
64 KiB content limit, unchanged identity, NIP-29/65/67/94 and all eleven
extensions. NIP-86/98 remain conditional on an explicit admin public key;
none was configured in the former deployment and none is invented here.

compaction 95931 ms; bytes 121714373 -> 112742351

old checkpoint before: 396278 nostr-relay 600; after: 396278 nostr-relay 600

/var/lib/nostr-relay-cp/generations/1794609475

The successful production run compared the served set at clock 1794609475:
`ea15bf7f8d186aa01cc777362f7502909e6e5bef33673e09a080ab7843243bc5`
was unchanged. The original checkpoint inode, owner and mode were retained.
The initial generation and prior binary remain available for rollback.

The weekly compaction timer is enabled; its next run after rollout is
2026-11-15 09:10:19 UTC (09:00 UTC plus up to fifteen minutes of jitter).
The checkpoint upload service passes after selecting the new generation.

The post-compaction service restart began at 22:39:31 UTC and reached
`listening` at 22:41:05 UTC (94 seconds), longer than the initial deployment
restart on the same shared-core VM. The manual maintenance interval plus
restart therefore caused about 190 seconds of unavailability. The weekly
schedule is a deliberate offline compaction; TLS renewal reloads no longer
need that outage. The two-minute availability alert can fire during this
planned maintenance. No machine-size or cost increase was made.

After the compacted generation reached `listening`, production SIGHUP
reload preserved an existing WSS connection and its REQ/EOSE response;
a new TLS/WSS connection also succeeded. The service had zero automatic
restarts. Checkpoint upload and the weekly timer are active.
