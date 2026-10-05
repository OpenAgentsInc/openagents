# Dedicated Verse host

`verse-host` serves authenticated TLS worlds through `verse-world`. Its normal
build depends on portable content admission and simulation, Tokio, and Rustls.
It does not link a renderer, window library, font library, retained reader, or
agent. REACH hosting remains available through `openagents chamber host`.

Compile an original world with the Rust content tool:

```sh
cargo run -p verse-content --features compiler -- ritual /tmp/verse-ritual
cargo run -p verse-content --features compiler -- observatory /tmp/verse-observatory
```

Each command requires a new directory and writes `pack.json`, verified runtime
textures, `scene.json`, and `profile.json`. The ritual uses the combat profile;
the observatory uses its authored floor and social seat/switch profile. Both use
the same asset, scene, collision, identity, authority, and checkpoint contracts.
The compiler does not create enrollment keys or TLS credentials.

Configure a host using the existing [TLS host configuration](../../docs/verse/networking.md).
Set `scene` and `pack` to the generated files. For the observatory, copy the
object in `profile.json` into `social_profile`. Supply the instance, listen
address, enrollment public keys, DER certificate, and owner-only private key.

```sh
cargo run -p verse-host -- /tmp/host.json --check 300
cargo run -p verse-host -- /tmp/host.json
```

`--check` admits content, collision, ownership, and saved state, advances the
shared fixed schedule, and validates a checkpoint. It opens no listener and
writes no new checkpoint. Serving uses the same admission path and schedule;
`state_dir` enables durable storage. SIGINT or SIGTERM requests clean shutdown.
Check mode validates the configured TLS paths but does not open credentials.

## Local operations

On a Unix host, add `--operations DIR` to enable an owner-only local socket:

```sh
cargo run -p verse-host -- /tmp/host.json --operations /tmp/verse-operations
cargo run -p verse-host -- --status /tmp/verse-operations
cargo run -p verse-host -- --drain /tmp/verse-operations
```

The parent directory must exist. The host creates `DIR` with mode `0700`, holds
an exclusive operator lock, and creates its socket with mode `0600`. The socket
accepts only status and drain requests. It has eight workers, two-second worker
timeouts, and a 256 KiB response limit. The command has a three-second total
connection and response timeout. It adds no network listener or player right.

`verse.host.operator.response.v1` wraps a `verse.host.operations.v1` snapshot.
`availability` is `fresh`, `stale`, or `unavailable`. A live sample older than
three seconds, a sample with an invalid clock, or a retained offline sample cannot
report readiness. A failed read, unsupported wire version, or malformed input
returns an error. After exit, `--status` reads the retained final snapshot with
`availability: unavailable`; `--drain` fails if it cannot reach the host.

The authority samples once per second into a latest-value channel. It retains
128 connection observations and 128 phase/reason transition records, with an
omitted-record count. Output includes content, instance, package, source, and
wire identities; simulation and storage timing distributions; current queues and
peaks; declared budgets; commit revision and oldest pending age; admitted actor
and living hostile counts; and motor/navigation refusal counts. The build records
its Git revision and adds `-modified` for modified source. Without Git, supply
`VERSE_SOURCE_REVISION` as a 40-character revision when building; otherwise the
source identity is `unrecorded`. These labels describe a local build.

Readiness means the running service has connection, writer, and receipt admission
room. A writer pending for at least one second reports `writer_stalled` and
refuses readiness. Lifetime simulation p99 above the 30 Hz interval and newly
observed work-budget refusals have separate reasons. Timing percentiles are
histogram upper bounds. They do not establish a production latency guarantee.
Connection IDs last only for this process. Traffic counts complete application
payloads consumed or delivered by the service, including authentication and local
refusals; they exclude TLS overhead and partially received frames. Tick lag and
last-delivery age describe delivered responses, not a continuously polling client.
Snapshots contain no keys, addresses, player names, chat, command bodies, or raw
worker errors. Admission failure stages use aggregate counters.

`--drain` confirms the request, not completion. The host stops admission, closes
connections, settles admitted durable mutations, writes final state, and releases
the writer lock. Wait for a successful process exit and the retained `stopped`
phase. A storage failure produces `failed`, stops the service, and prevents an
acknowledgment for the failed write. There is no drain timeout that releases a
still-running writer. A stale sample or timed-out command exposes a blocked loop
or drain without claiming readiness.

## Verified recovery and retention

Stop and drain the durable host before exporting a backup:

```sh
cargo run -p verse-host -- /tmp/host.json --backup /tmp/verse-backup
cargo run -p verse-host -- /tmp/host.json --verify-backup /tmp/verse-backup
cargo run -p verse-host -- /tmp/host.json --restore-backup /tmp/verse-backup /tmp/verse-restored
cargo run -p verse-host -- /tmp/host.json --prune-history
```

Export acquires the source writer lock and copies the last durable revision.
Export and restore require new directories and existing parent directories; they
refuse existing destinations and symlink paths. Set `state_dir` in a separate host
configuration to the restored directory before checking or starting that host.
Verify and restore admit the configuration's content without opening its original
storage. Every manifest entry, checkpoint, reward-history node, receipt binding,
character state, and retained migration archive is verified. Output is a bounded
`verse.backup.v1` report of scope, revisions, file totals, and digests. Backup
files hold private enrollment, ownership, and character state; retain their owner-only
permissions. Digests detect damage and mismatches; they do not authenticate a
backup obtained from another party.

The default operation permits 65,536 files and 1 GiB of data, a 16 MiB manifest,
256 KiB history nodes, and the existing bounded checkpoint format. Budgets are
operation limits, not reward-lifetime limits. Export copies only history reachable
from current state and every retained migration checkpoint. Pruning requires an
offline recovered store, plans the entire bounded scan before deletion, and
retains those same roots. It can remove unreferenced nodes and interrupted
history publications; it keeps all migration records and rollback checkpoints.
Keep a verified backup before pruning. Budget, unknown-file, or symlink failures
leave the removal plan unapplied; an I/O failure during deletion reports failure,
and any completed deletions remain safe because they were unreferenced.

An incomplete export retains `backup.pending`. An incomplete restore retains
`restore.pending`, and host startup refuses that directory. Restore holds its own
writer lock through verification and final directory synchronization. It preserves
migration rollback records, including the refusal to roll back after later
progress. Inspect a failed new destination before removing it and retry into
another new directory. The source is never overwritten.

The recovery objective is all effects in the verified backup revision, including
exact retry receipts. For a running store, acknowledged effects precede durable
publication; recovery from an older backup loses progress since that backup.
Schedule exports at the required recovery-point interval and time restore drills
on the intended storage. Scratch Linux tests establish correctness, not a
production recovery-time objective. This tool covers one chamber `Store`; a
coordinated realm transfer/registry recovery requires its own realm workflow.
