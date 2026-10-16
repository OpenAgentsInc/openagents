# Coder sync server

`coder-sync` serves OpenAgents Coder on Cloud Run in `openagentsgemini`,
`us-central1`. The production address is `https://sync.openagents.com`.
The service starts with one instance and a 3,600-second request timeout.
Clients reconnect when Cloud Run closes a WebSocket.

## Build and test

The existing `crates/coder-sync` library uploads the frozen Coder's chats to
OpenAgents. Keep that library and its default dependencies intact. The phone
server lives in `src/server`, behind the `server` feature, with a separate
`coder-sync` executable. Nothing here requires rebuilding a Coder 1.0 program.

Use Rust 1.98.1 and a Cargo target directory outside the worktree:

```sh
export RUSTUP_TOOLCHAIN=1.98.1
export CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1"
cargo fmt -p coder-sync --check
scripts/coder-sync/test.sh
cargo build -p coder-sync --features server
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
for the image. When changing server dependencies, update both manifests and
regenerate the package lockfile. No application code is copied into that
package.

## Configuration

| Variable | Meaning |
| --- | --- |
| `CODER_SYNC_DATABASE_URL` | Postgres connection string; Secret Manager in Cloud Run. Unix socket connections use Cloud Run's Cloud SQL attachment. |
| `CODER_SYNC_BUCKET` | The private Cloud Storage bucket. Cloud Run uses its runtime service account through the metadata token endpoint. |
| `CODER_SYNC_ENV` | `local`, `staging`, or `production`. Missing and unknown values refuse dev auth. |
| `CODER_SYNC_AUTH` | `disabled` by default; all room requests are refused until account sign-in lands in #11848. `dev` also requires `--allow-dev-auth` and a local or staging environment. |
| `CODER_SYNC_LISTEN` | Listen address; defaults to `0.0.0.0:$PORT`, or port 8080. |
| `CODER_SYNC_REVISION` | The full deployed Git commit, returned by `/health`. |

Dev bearers use `user` or `user@workspace`, matching the reference clients.
Do not use personal accounts. Production refuses dev auth even when the
command-line flag is set. Until #11848, production health works but no account
can open a room. There is no WorkOS server or Cloudflare deployment.

Startup applies migrations under a Postgres advisory lock and checks bucket
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
It reuses the latest published builder by digest and publishes new caches with
commit-specific tags, preserving Artifact Registry tag immutability. It refuses
a dirty checkout or one behind `origin/main`. Each Cloud Run
service has `max-instances=1`, a 3,600-second timeout, and its own database
secret and bucket. Staging enables explicit dev auth; production disables it.

Read `/health` at each returned `run.app` URL. Confirm `revision` equals the
pushed commit and production's `auth` is `disabled`. A room request with a dev
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

The initial migration creates chat ownership and rows, checkpoints, registry
heads and rows, devices and connections, and blob metadata. Protocol handlers
and client conformance land in #11843–#11847. Runbook statements about their
ordering, account checks, and compaction must be verified as those handlers
land; the skeleton does not expose room reads or writes.
