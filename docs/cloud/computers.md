# Computers: one lifecycle layer (oa-computers)

Issue [#11624](https://github.com/OpenAgentsInc/openagents/issues/11624).
Design: [`docs/research/factory/README.md`](../research/factory/README.md),
"Our version, now". Current state before this work:
[`docs/audits/2026-10-14-computers-for-chats-audit.md`](../audits/2026-10-14-computers-for-chats-audit.md).

## Pieces

| Crate | What it is |
| --- | --- |
| `computer-driver` | The one driver contract. Profiles: **Core** (create under an idempotent operation, observe, exec, files, meter, delete), **Suspend** (stop/resume on the same disk), **Snapshot** (save a disk under a name, create from it), **Fork** (copy a computer). Drivers declare them in `Capabilities`; `admit` refuses a request needing an undeclared profile before anything is written, and the driver itself answers an undeclared operation with a definite "unsupported" failure. Drivers: `FakeDriver` (tests: a directory per computer, real `sh`), `BoatDriver` (our `oa-boat`). |
| `computer-conformance` | Black-box cases per profile, plus refusal cases for undeclared ones. `cargo test -p computer-conformance` (fake driver) and `cargo test -p oa-boat --test conformance` (oa-boat on loopback). Against a deployed driver: `BOAT_API_KEY=… cargo run -p computer-conformance -- --boat https://HOST/api/v1`. |
| `coder-working-computer` | The pure lifecycle core (decide/apply, unchanged rules), now generic over where its record is held (`driver::Held`), with `on_driver::OnDriver` running a chat computer on any driver that has Core and Suspend. A turn's checkpoint is the stopped disk. |
| `oa-computers` | The binary: HTTP API + reconcile loop + Postgres. |

## oa-computers

- **Intent, at once.** `POST /v1/computers` validates the request against
  the driver's capabilities, writes one row (`desired = running`), and
  answers `202` with the id. The same live chat (or idempotency key)
  answers `200` with the existing computer.
- **Observed state, through a lease.** The reconciler claims a due row
  (`FOR UPDATE SKIP LOCKED`, `lease_holder`/`lease_until`), runs one settle
  of the lifecycle core toward `desired`, and retains every driver outcome
  before the next decision, fenced on the record's revision and on still
  holding the lease. A long effect renews the lease. Failures back off
  (5 s doubling, 5 min cap) with `last_error`; settled live computers are
  revisited every 30 s for idle, absolute and silent-turn bounds. A
  lifecycle bound that stops a computer also moves its intent to
  `stopped`, so it is not woken again until asked.
- **Restart-safe.** A process killed mid-provision leaves the create's
  operation identity retained; the next process (after the lease expires)
  repeats that identity and finds the same computer. Tested by killing the
  real binary with `SIGKILL` while a computer is creating or booting on
  oa-boat (`a_process_killed_mid_provision_finishes_the_same_computer_after_restart`).
- **Storage.** Its own schema, `computers`, in the account database (the
  same Cloud SQL instance and connection the web's tenancy and actors
  tables use, each in its own schema). Migrations under
  `crates/oa-computers/migrations`, applied at start under an advisory
  lock.

### API

All calls carry `Authorization: Bearer $OA_COMPUTERS_TOKEN` (the web
server's service token) and `X-OpenAgents-Account: <account>`; an account
sees only its own computers.

| Call | Does |
| --- | --- |
| `POST /v1/computers` `{chat?, image?, size?, idempotency_key?, environment?, version?}` | record intent; `202` new, `200` existing, `422` the driver cannot hold a chat computer, `400` bad size or field |
| `GET /v1/computers`, `GET /v1/computers/{id}` | the account's computers: `id`, `chat`, `driver`, `image`, `size`, `desired`, `state` (lifecycle phase), `resource`, `turns`, `attempts`, `last_error`, times |
| `DELETE /v1/computers/{id}`, `POST …/stop`, `POST …/wake` | change intent, `202` |
| `GET /v1/capabilities` | the driver's declaration |
| `POST /v1/computers/{id}/turns` `{command, cwd?, timeout_seconds?, env?}` | queue a turn (`202` with its id); `env`'s values stay in memory only |
| `GET /v1/computers/{id}/turns/{turn}`, `POST …/turns/{turn}/stop` | read a turn (`state` queued/running/done/failed, `exit_code`, `stdout`, `stderr`, `error`); stop it |

### Turns

A turn is one command on a computer for a chat. The API records it
(command and env *names* only) and keeps the env *values* in the process's
memory. Once the computer is awake the reconciler dispatches a turn
generation through the lifecycle core, takes the values out of memory, and
runs the command with them as that one process's environment (on oa-boat:
the command request's `env`, carried over the SSH channel's standard input
and exported for that process only). The turn's owner heartbeats while it
runs; when it ends, its output is kept (256 KB a stream), the turn's
checkpoint stops the computer with its disk, and its intent moves to
`stopped` until the next turn. A restart of the service before a queued
turn starts forgets its values: that turn fails asking to be sent again,
and nothing runs without the credentials it was sent with. Stop ends a
queued turn at once and abandons a running one's command.

A computer stopped for `OA_COMPUTERS_KEEP_STOPPED_DAYS` (7) with nothing
asked of it is deleted (its disk bills while stopped: about $0.027 an hour
for a 200 GB `oa-coder-host` disk).

## Chats on their own computers (web)

With `OPENAGENTS_WEB_COMPUTERS_URL` (or `--computers-url`) and the service
token (`OPENAGENTS_WEB_COMPUTERS_TOKEN`, or `--computers-token FILE`), a
"Claude Code in {repo}" message in a web chat runs on the chat's own
computer (`chat_work::begin` → `crate::computers`):

1. `Studio::chat_turn` builds the turn: the saved version's image, the
   Coder runtime command (`coder --json … delegate claude --session
   chat-{chat}`), and the per-turn env: the task, the person's own Claude
   credential (`cloud::byo::release_turn`, else this server's key) and the
   GitHub broker ticket for pushing.
2. `POST /v1/computers` with the chat and image makes the chat's computer
   the first time and wants it running every later time.
3. `POST …/turns` queues the turn; the chat records a task of kind
   `computer` whose id is `{computer}.{turn}` and follows it like any run;
   the reply joins the chat when it is done.

Messages with files still run on a fresh computer through the studio.
Cloud computers stay behind the agent-work gate (site admins and named
agent accounts); `GET /v1/computers` adds the account's cloud computers
(`kind: "cloud"`) to its own for those accounts. oa-computers knows an
account only by the chat store's opaque owner key, never its id or email.

### Running it

```sh
OA_COMPUTERS_DATABASE_URL='host=127.0.0.1 port=55432 user=postgres dbname=computers' \
OA_COMPUTERS_TOKEN=dev-token OA_COMPUTERS_DRIVER=fake \
cargo run -p oa-computers
```

Settings: `OA_COMPUTERS_DRIVER` (`boat` with `BOAT_API_BASE`/`BOAT_API_KEY`,
`fake`, or experimental [`space`](space.md) when built with `--features space`),
`OA_COMPUTERS_LISTEN` (else `0.0.0.0:$PORT`),
`OA_COMPUTERS_RECONCILE=0` (API only), `OA_COMPUTERS_LEASE_SECONDS` (60),
`OA_COMPUTERS_REVISIT_SECONDS` (30), `OA_COMPUTERS_BACKOFF_MS` (5000),
`OA_COMPUTERS_POLL_MS` (1000), `OA_COMPUTERS_DRIVER_POLL_MS` (3000).

Tests that need Postgres run with a throwaway server:

```sh
initdb -D /tmp/pg -U postgres --auth=trust
pg_ctl -D /tmp/pg -o "-p 55432 -k '' -c listen_addresses=127.0.0.1" -l /tmp/pg.log start
OA_COMPUTERS_TEST_DATABASE_URL='host=127.0.0.1 port=55432 user=postgres' cargo test -p oa-computers
```

Without the variable those tests print a skip line and pass.

## Deployment

Cloud Run service `oa-computers` (project `openagentsgemini`, us-central1),
one instance always on (`--min-instances 1 --max-instances 1
--no-cpu-throttling --cpu 1 --memory 512Mi`), built by
`deploy/oa-computers/cloudbuild.yaml`. Its tables are in the `computers`
schema of the production account database (`openagents-production-pg`,
DSN secret `openagents-production-pg-dsn`); `OA_COMPUTERS_TOKEN` is the
secret `oa-computers-token`, `BOAT_API_KEY` the secret `oa-boat-api-key`,
`BOAT_API_BASE` oa-boat's `/api/v1`.

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
C=$(git rev-parse origin/main)
gcloud builds submit --no-source --config deploy/oa-computers/cloudbuild.yaml --substitutions _COMMIT=$C \
  --project openagentsgemini --region us-central1 \
  --service-account projects/openagentsgemini/serviceAccounts/oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com \
  --gcs-log-dir gs://openagentsgemini-oa-mvp-cloud-build-logs/oa-computers
gcloud run deploy oa-computers --project openagentsgemini --region us-central1 \
  --image us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/oa-computers:$C
```

Deploying as the automation account is refused `actAs` on the runtime
account (the compute default, which already reaches the account database
and the secrets); deploy with the owner's gcloud account, as
`scripts/deploy/web.sh` does.

Live check, 2026-10-14 (as the synthetic account `test-smoke-11624`, no
person's data): `POST /v1/computers` answered in 0.21 s; the reconciler
booted a `small` oa-boat VM from `oa-coder-host` (77 s to the first turn),
ran a turn with a per-turn secret (the command saw its 22 characters) and
found `coder-cloud-runtime`; the checkpoint stopped it; a second turn
resumed the same VM in 52 s, read the first turn's file, and saw no
secret; delete removed the VM (no `oa-boat` instance left).

## Not yet

Warm pool, activity-based suspend beyond the idle bound, billing, the
own-machine and Pylon drivers, Kubernetes agent-sandbox and Firecracker
drivers, signed enrollment for cloud computers (see the design's build
order).
