# Coder sync server

`coder-sync` serves OpenAgents Coder on Cloud Run in `openagentsgemini`,
`us-central1`. The production address is `https://sync.openagents.com`.
The service starts with one instance and a 3,600-second request timeout.
Clients reconnect when Cloud Run closes a WebSocket.

## Build and test

The existing `crates/coder-sync` library uploads the frozen Coder's chats to
OpenAgents. Keep that library and its default dependencies intact. The phone
server lives in `crates/coder-sync/src/server`, built by the standalone
`deploy/coder-sync` workspace as the `coder-sync` executable. Nothing here requires rebuilding a Coder 1.0 program.

Use Rust 1.98.1 and a Cargo target directory outside the worktree:

```sh
export RUSTUP_TOOLCHAIN=1.98.1
export CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1"
cargo fmt --manifest-path deploy/coder-sync/Cargo.toml --all --check
scripts/coder-sync/test.sh
cargo build --locked --manifest-path deploy/coder-sync/Cargo.toml --features server
```

The test script starts its own Postgres cluster under `openagents scratch`,
listens on a free loopback port, and removes it on exit. It never opens the
owner's database or account. Set `CODER_SYNC_POSTGRES_BIN` to a Postgres `bin`
directory if `pg_config` selects the wrong installation. On this Mac, use
`/opt/homebrew/opt/postgresql@16/bin`. `CODER_SYNC_POSTGRES_SHARE` optionally
selects the matching `initdb` data files.

Integration tests use that disposable database and an in-memory bucket. They
check repeatable migrations, schema constraints, auth configuration, health
and request IDs, serialized room writes, and writes whose caller disconnects.
The image package can also be checked without compiling the frozen library:

```sh
cargo build --locked --manifest-path deploy/coder-sync/Cargo.toml --features server
```

`deploy/coder-sync/Cargo.toml` and its lockfile package the same server source
for the image. The pinned fork client uses SQLite 0.32, while the frozen monorepo uses
0.31. The separate workspace avoids a native-library conflict without changing
those clients. Update the standalone manifest and lockfile for server
dependencies. No application code is copied into that package.

## Configuration

| Variable | Meaning |
| --- | --- |
| `CODER_SYNC_DATABASE_URL` | Postgres connection string; Secret Manager in Cloud Run. Unix socket connections use Cloud Run's Cloud SQL attachment. |
| `CODER_SYNC_BUCKET` | The private Cloud Storage bucket. Cloud Run uses its runtime service account through the metadata token endpoint. |
| `CODER_SYNC_ENV` | `local`, `staging`, or `production`. Missing and unknown values refuse dev auth. |
| `CODER_SYNC_AUTH` | `openagents` by default. Each request checks an OpenAgents app session with the public account API. `disabled` refuses every room request. Local `dev` requires `--allow-dev-auth` and a local or staging environment. |
| `CODER_SYNC_LISTEN` | Listen address; defaults to `0.0.0.0:$PORT`, or port 8080. |
| `CODER_SYNC_REVISION` | The full deployed Git commit, returned by `/health`. |

Dev bearers use `user` or `user@workspace`, matching the reference clients.
Do not use personal accounts. Production refuses dev auth even when the
command-line flag is set. Room access requires a current OpenAgents account
session. There is no WorkOS server or Cloudflare deployment.

Startup applies each migration once under a Postgres advisory lock and checks bucket
read/write access with a short-lived test object. `/health` checks Postgres
and returns the revision and auth mode. It returns 503 when Postgres is
unavailable. Every response includes a generated `x-request-id`; structured
request logs include that ID, status, method, and duration. They omit paths,
queries, credentials, and room contents.

## Provision and deploy

Use the automation account configuration, without browser login:

```sh
export CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config"
export BROWSER=/usr/bin/true NO_OPEN_BROWSER=1
python3 scripts/coder-sync/deploy.py provision
```

The first call starts `coder-sync-pg` (Postgres 17, Enterprise shared-core,
10 GB SSD). Run `provision` again when the instance is `RUNNABLE`. It creates:

- databases and users `coder_sync` and `coder_sync_staging`;
- private buckets `openagentsgemini-coder-sync` and
  `openagentsgemini-coder-sync-staging`;
- Secret Manager entries `coder-sync-database-url` and
  `coder-sync-staging-database-url`;
- runtime account `coder-sync@openagentsgemini.iam.gserviceaccount.com`.

Passwords are generated in memory, sent in HTTPS bodies, and stored in Secret
Manager. They are never printed. The runtime account gets object access to
these buckets, access to these secrets, and `roles/cloudsql.client` on the
project. If automation cannot grant a permission, put the exact owner command
in the workspace repository's `NEEDS_OWNER.md`; do not switch to the owner's
account or give the runtime a broader identity.

Commit, rebase on current `origin/main`, and push before deploying:

```sh
python3 scripts/coder-sync/deploy.py build
# Check the returned build ID until its status is SUCCESS:
gcloud builds describe BUILD_ID --project=openagentsgemini --format='value(status)'
python3 scripts/coder-sync/deploy.py deploy --staging
python3 scripts/coder-sync/deploy.py deploy
```

The script exports only committed build inputs, builds on Cloud Build using
the automation service account, and tags the image with the full Git commit.
It copies the latest published builder's Cargo registry, Git sources, and
compiled dependencies by digest and publishes new caches with
commit-specific tags, preserving Artifact Registry tag immutability. It refuses
a dirty checkout or one behind `origin/main`. Each Cloud Run
service has `max-instances=1`, a 3,600-second timeout, and its own database
secret and bucket. Staging and production use OpenAgents accounts. Neither deployment enables dev auth.

Read `/health` at each returned `run.app` URL. Confirm `revision` equals the
pushed commit and production's `auth` is `openagents`. A room request with a dev
bearer must return 401 in production. Do not send credentials to `/health`.

## Domain and owner steps

```sh
python3 scripts/coder-sync/deploy.py domain
gcloud beta run domain-mappings describe --domain=sync.openagents.com \
  --region=us-central1 --project=openagentsgemini --format=yaml
```

Create exactly the DNS records returned by the domain mapping. If domain
verification, IAM, or DNS needs the owner, record the exact command or record
in `~/work/NEEDS_OWNER.md`, from a clean workspace-repository worktree. Continue
with the service's `run.app` URL. Verify
`https://sync.openagents.com/health` after the mapping and certificate are ready.

## Room and data boundaries

Each chat, account registry, and device has one bounded mailbox and one writer
in the process. New rooms are refused above the room limit; full mailboxes
return a retryable refusal. Accepted work continues if its caller disconnects.
This is a single-instance starting point. Future room handlers must keep
sequence allocation, deduplication, and checkpoints in Postgres transactions:
Cloud Run can overlap revisions during a deployment even with a one-instance
setting. Additional instances require the planned room leases and fan-out.

Chat routes now implement `/chat2/{id}/ws` (and the base-path alias),
`/rows` GET/POST, and `/checkpoint` GET/POST. The first authenticated account
claims the chat. PostgreSQL locks that ownership row for every operation;
row insertion, batch deduplication, and head advancement commit together.
Batch records survive compaction, so a delayed retry cannot append twice.

The wire codec and client come from `zeron-sync` at the fork revision recorded
in `deploy/coder-sync/Cargo.toml`. The server treats payloads as
opaque. As in the edge, clients decide when to post checkpoints (more than
200 rows or 512 KiB); the server validates the covered sequence and frontier,
stores an immutable bucket object, then atomically publishes its metadata
and deletes covered rows. A failed transaction never publishes a sequence
or checkpoint. Old checkpoint objects remain readable for in-flight downloads;
object retention and garbage collection need a later storage policy.

HTTP replay pages stay below 4 MiB and omit `rowsDone` when more rows remain.
WebSocket replay streams those pages, then sends `rowsDone`. Broadcast queues
are bounded; lagging sockets disconnect and recover using their cursor.
Pushes retain the edge's per-device 300 pushes/8 MiB per minute limits,
1 MiB row cap, and 32 MiB checkpoint cap. Presence stays in memory.

The integration suite runs the pinned client through checkpoint loading,
push, duplicate replay after reconnect, live delivery, and HTTP fallback. It
also checks account isolation, range downloads, multi-page replay, and a SQL
failure after row insertion but before head advancement. A bounded Stateright
model checks two writers, two readers, two attempts per batch, two crashes,
and checkpoint compaction (8,796 states). These bounds do not prove behavior
for arbitrary fleets or failed storage systems.


## Account registry

`/registry/{org}` exposes WebSocket sync, HTTP rows/push, stats, reset, and
push-target registration. The path must match the authenticated account or
workspace; storage and room keys always use the account. Workspace aliases
never share private rows across accounts. The pinned `zeron-doc` types and
merge core preserve per-field clocks, tombstones, original clocks on re-seed,
and atomic batches such as deleting a space with its chats and sessions.
Rows and sender acknowledgments use one ordered queue, with merged rows first.

Presence expires after 30 seconds of inactivity and is never stored in
Postgres. Tombstones remain stored; `gcFloor` stays zero until a retention
policy is added. Push targets stay private to the account, stats omit tokens,
and registration sends no notifications in this release. Reset closes sockets;
the pinned client reconnects and re-seeds its rows with their original clocks.
The tests cover that recovery, account isolation, sender ordering, failed-batch
rollback, and migration startup while a registry writer holds a row lock.

## Device relay

`/device/{id}/ws?role=host|client` uses the pinned `zeron-rpc` binary codec.
The relay stamps viewer connection IDs, forwards opaque payloads, and returns
transport errors when a peer is unavailable. A new host replaces its previous
connection. Host selection uses the newest activity within 75 seconds; text
`ping` receives `pong`. Slow outbound queues disconnect instead of dropping
RPC frames. Device rooms and connection queues have fixed limits.

The first host claims a device for its account. Other accounts cannot connect,
read status or sidecars, or send wakeups. Owners can grant viewer access with
`PUT /device/{id}/grants/{account}` and remove it with `DELETE`; removal closes
that account's existing viewers. A viewer grant never permits hosting or
publishing sidecars. `/status`, `/nudge`, and `/sidecar/{name}` retain the
reference protocol. Sidecars persist in Postgres.

Chat pushes with `hostDevice` store a wakeup in the same transaction as the row,
including retries. Receipts can precede the first host connection, but only
that account's host can consume them. Hosts advertising `nudgeAck=1` receive
receipts again after disconnects or missing acknowledgments. Receipt tokens
prevent an old acknowledgment from deleting newer work. At 4,096 pending
chats, explicit nudges return a retryable 503 and retain a full-scan request;
accepted chat rows still commit with that request. Legacy hosts receive the
reference delivery behavior without acknowledgments.

The default suite ports both edge device codec cases and all nine host-liveness
cases. It also covers opaque routing, host replacement, disconnects, grants,
sidecars, account isolation, wakeup retries, overflow, and transaction rollback.
Build `zeron` from that same pinned fork revision to run the engine integration:

```sh
ZERON_TEST_HEADLESS_BIN=/path/to/pinned/zeron \
  scripts/coder-sync/test.sh --test device pinned_headless -- --ignored
```

This test launches only its own headless engine with a scratch home, data
folder, mock harness, and free IPC port. The real pinned viewer lists a folder,
creates a scratch chat, queues and observes a message, removes it, and stops
that engine. It also verifies refusal of another account's viewer.

## Blobs and image attachments

`/blob/{chatId}/{partId}` supports PUT, GET, HEAD, ETags, conditional reads, and
single byte ranges (closed, open, or suffix). Tool blobs have the reference
1 MiB limit and default to `text/plain; charset=utf-8`. An account must own
the chat; stored keys include an encoded account prefix and an immutable version.
Replacements keep older objects for seven days so in-flight reads can finish.

Uploads first enter `orphan/`, then copy to an immutable published key before
the database references it. A committed upload intent covers a crash between
copying and publication. An hourly worker deletes expired intents and retired
versions, after checking that no live blob references them. Database deletion
also schedules the old object for cleanup. Apply the bucket lifecycle rules:

```sh
python3 scripts/coder-sync/deploy.py lifecycle
```

The rules delete abandoned `orphan/` stages after seven days and `_health/`
probes after one day; they never expire published blobs or checkpoints.
See Google's [object lifecycle documentation](https://docs.cloud.google.com/storage/docs/lifecycle).
Provisioning applies these same rules to both dedicated buckets.

The pinned phone client uploads images to its host through chunked device RPC,
with a 24 MiB client limit; other viewers read them through the same relay.
The headless fixture uploads a PNG across multiple chunks, queues its path,
and verifies that a second pinned client downloads and renders identical pixels.
It writes `11846-rendered.png` under `openagents scratch`. The legacy
`/attachments/{…}` PUT acknowledges and discards retired mirror uploads,
matching the pinned edge; it does not create a second attachment store.

## Protocol conformance checkpoint

The `coder-sync-conformance` crate depends on the real `zeron-sync`,
`zeron-client`, `zeron-proto`, `zeron-doc`, and `zeron-rpc` crates. One workspace
revision pins all five. Run the complete suite, including the isolated engine
and image fixture:

```sh
scripts/coder-sync/conformance.sh
```

For an upstream merge candidate, use a clean OpenAgents worktree:

```sh
scripts/coder-sync/conformance.sh --fork-repo /path/to/fork --pin FULL_COMMIT
```

The script resolves the full local commit, updates the shared pin and lockfile,
exports its source without editing the fork checkout, and builds the headless
engine in the fork's persistent external target directory. It then runs every
case against temporary Postgres and a fake bucket, checks formatting, and
builds the server executable. Local Git configuration lets Cargo resolve an
unpublished candidate without changing global settings. A failed run restores
the pin files if nothing else changed them. Phase logs stay under
`openagents scratch` in `coder-sync-conformance`.

Commit the pin and lockfile after the candidate passes and is published to our
fork. `FORK.md` requires this checkpoint whenever an upstream merge changes
`edge/`, `crates/sync`, `crates/proto`, or `crates/rpc`. Counterexamples become
plain regression tests before retrying. Disk checks stop the script below
8 GiB free. `CODER_SYNC_FORK_TARGET_DIR` selects the fork build cache; the
server keeps `CARGO_TARGET_DIR`.

The remaining edge update-log cases check small updates, large updates,
interleaving, slice offsets, old-row migration, repeated migration, and real
Loro fold/replay. Postgres needs no SQLite chunk layout; large logical updates
use modern checkpoints, and normal row uploads retain their 1 MiB limit.
Pure registry notification rules cover completed turns, questions, errors,
stale activity, first sync, titles, and preferences. They send no notifications
and create no APNs transport; #11853 must use the OpenAgents push gateway.

### OpenAgents account sessions

Both clients reuse `coder-account` and the public OpenAgents SDK device-code
flow from #11824. The account service verifies each HTTP request and WebSocket
handshake. Live sockets recheck the session every 15 seconds and close after
revocation, expiry, or an account-service outage. No successful authentication
result is cached. The account ID returned by the service owns rooms and blobs;
client-supplied account headers are discarded.

`CODER_SYNC_ACCOUNT_API` and `CODER_SYNC_ACCOUNT_WEBSITE` default to
`https://api.openagents.com/v1` and `https://openagents.com`. Remote deployments
require HTTPS. Loopback HTTP is available for dedicated local test accounts.
Run `scripts/coder-sync/test.sh --ios-auth-fixture CONFIG_PATH` to start the
isolated account service, sync service, fake bucket, and temporary Postgres
for the native sign-in test. The configuration file contains addresses and
fixture IDs only. Stop that fixture with `POST /__fixture/stop` on its sync URL.
The fixture does not contact GitHub, save a production credential, or open a
browser.
