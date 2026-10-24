# Operations, durability, and portability

Status: proposed deployment and operational requirements. Availability and
recovery figures are acceptance targets until measured in failure drills.

Own the service contracts and data formats; use mature replaceable infrastructure
for transactions, object storage, transport, cryptography, and observability.
Unlimited engineering justifies strong fault isolation, not unnecessary services
in the creator's local editing loop. See [architecture](architecture.md),
[integration](integration.md), and [performance](performance.md).

## Service and storage boundaries

**OPS-001.** The following logical services have independently testable APIs and
authority. An initial deployment may package compatible services together.

| Boundary | Responsibility | Durable state |
| --- | --- | --- |
| Identity and policy | Membership, sessions, capabilities, revocation | PostgreSQL |
| Project authority | Branches, accepted revisions, receipts, merge decisions | PostgreSQL plus immutable document blobs |
| Collaboration transport | Presence, authenticated CRDT updates, subscriptions | Accepted update records; presence expires |
| Compiler scheduler | Jobs, deduplication, resource reservations, worker leases | PostgreSQL job records and output references |
| Artifact service | Digest-verified uploads, access checks, retention | S3-compatible object storage |
| Publication and trust control | Manifest admission, channel comparisons, publisher grants, key roles/revocation | Three-region consensus ledger, quorum two; protected signing keys |
| Session authority | Multiplayer ticks, checkpoints, input admission | Session-specific checkpoint and event stores |
| Integration broker | Optional models and external tools | Scoped grants, attempt receipts, usage ledger |

Do not make Redis, a message bus, or a CDN the sole record of an accepted edit.
Queues carry retryable notifications from a transactional outbox. Consumers
deduplicate effects and tolerate repeated or reordered delivery. Workers receive
short-lived scoped access to exact inputs and a destination prefix; they cannot
read all tenant storage.

**OPS-002.** A branch commit acknowledges remote durability only after its
metadata transaction and referenced input blobs satisfy the configured durable
storage policy. Stage and verify blobs before committing their references.
Incomplete uploads remain unreferenced and expire. Every database head points
to retrievable immutable content; periodic integrity scans verify that rule.
Derived artifacts can be recomputed from retained permitted inputs and pinned
tools. Accepted source revisions cannot. Retain full command receipts for at
least 30 days and compact scoped command-ID/request-digest tombstones for the
project's lifetime, including across branch deletion and restoration. Compaction
cannot make an accepted command executable again. Receipt/status reads require
current authenticated read permission; a new mutation additionally requires its
current write grants. See [collaboration](collaboration.md).

## Local mode and browser persistence

**OPS-003.** Local mode supports creating, editing, importing supported assets,
compiling supported workloads, previewing, saving, and exporting without a cloud
account or model. The browser uses OPFS for blobs and a transactional IndexedDB
index/outbox. Write, flush, and verify blobs first; atomically commit references
and command receipts second. The application never commits a reference while its
blob write is pending. Recovery checks referenced digests before exposing a
revision, recovers from the retained command log where possible, and reports
missing source rather than silently opening an incomplete project. One local
authority coordinates open tabs; competing tabs cannot bypass branch-head
comparisons.

The optional Rust companion provides large imports, native compilation, durable
filesystem projects, and local serving. It binds loopback, authenticates the
browser handshake, validates `Origin` and `Host`, and exposes project-scoped
operations rather than a general shell. Browser-only limitations remain visible
in [platform support](platform-support.md). A downloaded editor supports offline
use only when its required code and content are already available locally.

Request persistent browser storage where available, display storage status, and
offer portable backups. The browser's
[Storage Standard](https://storage.spec.whatwg.org/) defines quota estimates and
persistence; users can still clear their data. Browser storage is not a backup.
Use the OPFS sync handle's `flush()` before committing a blob reference;
the [File System Standard](https://fs.spec.whatwg.org/#api-filesystemsyncaccesshandle-flush)
defines its storage transfer behavior. Use strict IndexedDB durability hints for irreplaceable local commands and
relaxed durability for replaceable caches, then test supported browsers rather
than claiming identical power-loss behavior.
[IndexedDB durability](https://www.w3.org/TR/IndexedDB/#transaction-durability).

## Recovery and disaster handling

**OPS-004.** Hosted production targets no loss of acknowledged project commits
within the declared single-region fault model through synchronous replicas
across availability zones. A region-wide disaster has an initial recovery-point
target of at most five minutes and a recovery-time target of one hour. These are
distinct targets; a deployment must publish its measured scope and cannot claim
zero global loss from asynchronous replication.

Publication and trust control use a stricter, separate failure model. Their
consensus ledger spans three independent regions and acknowledges only after
two regions commit. Loss of any one region must preserve every acknowledged
channel sequence, publisher grant change, and key revocation. Loss of quorum
pauses publication and trust changes rather than accepting writes through a
regional authoring database. This control ledger is not restored from the
five-minute-RPO authoring backup. PostgreSQL publication indexes and CDN metadata
are rebuildable views of it.

Before admitting a channel update, verify the complete release artifacts and
its private pinned source snapshot in at least two independent regions. Admission
records their digests and replication evidence in the control ledger. Rights
checks govern source replication and export; a public game does not make its
source public. Losing a region therefore cannot strand an acknowledged channel
on a release whose only artifact or source copy vanished. Replication and quorum
behavior require failure-drill evidence before production qualification.

Maintain encrypted database backups, write-ahead-log archives, object-version
retention, independent credentials, and at least one failure-independent backup
location. PostgreSQL supports point-in-time recovery from base backups and WAL;
Aethyrforge must also restore the matching artifact and key history.
[PostgreSQL recovery](https://www.postgresql.org/docs/current/continuous-archiving.html).

Run scheduled restore drills to an isolated environment. Reconstruct branch
heads, verify every referenced digest, rebuild channel views from the control
ledger, and confirm trust/revocation history before opening writes. A restored
older authoring database must not publish a lower channel sequence or restore a
revoked publisher grant. Restore an admitted release's replicated private source
snapshot as an immutable checkpoint with its original source/release references;
do not invent missing command history. If a restored branch's latest accepted
head cannot be proved current, close that branch to writes and create a new
`branchId` for recovery. Never reuse a lost revision number under the old branch
identity. Reconnecting clients resynchronize and merge their retained work into
the recovery branch explicitly. Session recovery resumes a
declared checkpoint or explicitly ends the session; it must not claim every
uncheckpointed simulation tick survived.

During authority failure, clients retain local work on offline branches. Public
players may continue already loaded releases when their required services allow
it. Publishing, payments, and privileged integration calls fail closed. Failure
of a model provider does not disable manual creation.

## Observability and cost control

**OPS-005.** Use OpenTelemetry-compatible traces, metrics, and logs behind a
replaceable exporter. Trace the accepted command through validation, dependency
invalidation, compilation, activation, persistence, and publication. Include
project-safe identifiers, revision/generation IDs, cache decisions, timings,
byte counts, and classified failures. Keep source, prompts, access tokens, player
chat, and raw private asset data out of default telemetry.
[OpenTelemetry trace conventions](https://opentelemetry.io/docs/specs/semconv/general/trace/).

Track tail edit latency, frame time, memory, device loss, queue age, cache hit
rate, unsynced work, rejected transactions, missing blobs, checkpoint lag, and
cost per compiler/model/session workload. Correlate browser and service spans
without collecting a permanent cross-project player identity. Sampling may drop
debug traces; security events and financial records have separate retention and
delivery requirements.

Admission reserves bounded CPU/GPU time, bytes, concurrent jobs, and optional
model spend before starting costly work. Reconcile reservations with recorded
attempt results, including unknown outcomes after worker loss. Weighted fair
queues prevent one project from starving interactive edits. Preempt background
bakes before live iteration. Billing and unlimited product budgets never remove
per-project abuse limits or operator kill switches.

## Export, deletion, and self-hosting

**OPS-006.** Export a documented directory/archive containing project snapshots,
branch and command history, source frontiers plus portable source text, asset
metadata and permitted source files, immutable artifacts, release manifests,
signatures, and public trust history. Include schema versions and a digest index.
Do not require an Aethyrforge account to read the export. Restricted assets may
need reauthorization on another host; identify omitted files and reasons without
silently producing an unusable export.

Self-hosting supports the same Rust services, transactional store, object-store
interface, browser headers, verification tools, and documented key bootstrap.
Single-machine local use does not inherit hosted availability claims. Imports
verify digests and validate all structures; an archive is never executable
installation authority.

Deletion removes live access promptly, then expires content according to a
published backup and legal-retention policy. Tenant-specific authorization and
encryption prevent shared digest caches from becoming an asset-discovery API.
Garbage collection uses reachability from revisions, pending jobs, published
releases, their control-ledger admission/source pins, active session pins, and
retention holds. A stale restored authoring index cannot collect a ledger-pinned
source snapshot or release artifact. Storage deduplication never
overrides an asset license or a deletion obligation. The [verification
plan](verification.md) tests export/reimport, offline recovery, storage exhaustion,
regional restore, worker loss, and tenant isolation before their associated
product capabilities become generally available.
