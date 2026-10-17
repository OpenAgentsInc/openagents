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

### Internal service API

All calls carry `Authorization: Bearer $OA_COMPUTERS_TOKEN` (the web
server's service token) and `X-OpenAgents-Account: <account>`; an account
sees only its own computers.

| Call | Does |
| --- | --- |
| `POST /v1/computers` `{chat?, image?, size?, idempotency_key?, environment?, version?}` | record intent; `202` new, `200` existing, `422` the driver cannot hold a chat computer, `400` bad size or field |
| `GET /v1/computers`, `GET /v1/computers/{id}` | the account's computers: `id`, `chat`, `driver`, `image`, `size`, `desired`, `state` (lifecycle phase), `resource`, `turns`, `attempts`, `last_error`, times |
| `DELETE /v1/computers/{id}`, `POST …/stop`, `POST …/wake` | change intent, `202` |
| `GET /v1/capabilities` | the default driver's declaration plus account-filtered placement offers and shapes |
| `GET /v1/usage?after=SEQUENCE&limit=100` | the account's usage evidence, up to 200 events per page, and allocation allowances |
| `POST /v1/computers/{id}/turns` `{command, cwd?, timeout_seconds?, env?}` | queue a turn (`202` with its id); `env`'s values stay in memory only |
| `GET /v1/computers/{id}/turns/{turn}`, `POST …/turns/{turn}/stop` | read a turn (`state` queued/running/done/failed, `exit_code`, `stdout`, `stderr`, `error`); stop it |

### Public account API

[#11801](https://github.com/OpenAgentsInc/openagents/issues/11801) adds public
account-authenticated create/list/detail/stop/wake/delete and capability reads
through the website/API front. See the
[implemented public lifecycle](../sdk/computers.md#implemented-public-lifecycle)
for routes, key scopes, paging, and errors. The server derives the opaque owner
from current account authority and constructs the internal service request;
callers never need `OA_COMPUTERS_TOKEN` or `X-OpenAgents-Account`.

A create key is retained in `computers.create_requests`, including additional
keys attached to the same chat and keys whose computer was deleted. Conflicting
immutable settings refuse. Keyed retries do not wake a subsequently stopped
computer; legacy unkeyed chat ensure still does. Transactional admission locks
serialize per account, limit unsettled computers to 20, and bound concurrent
create transactions. Paging is by `(created_at, id)` under the same owner.

The service assigns each new computer to a configured deployment before any
provider call. Its saved binding includes the resolved image, shape, requirements,
policy, and operation namespace. Restarts and changed defaults keep that binding;
a missing or changed connection leaves the computer unavailable until restored.
Legacy rows require an explicit `OA_COMPUTERS_LEGACY_DEPLOYMENT=oa-boat-primary`
mapping to their previous Boat connection. The deploy helper verifies that
connection before supplying the mapping; it never guesses from a new default.

`OA_COMPUTERS_REGISTRY_FILE` selects a JSON configuration with `default`, `policy`,
and `deployments`. Each deployment has a stable `id`, `kind` (`boat`, `space`, or
test-only `fake`), credential-free `api_url`, and a `credential_env` variable name.
Optional settings include `data_plane_url`, digest-pinned `template`, `scope`,
`placement`, `priority`, `capacity`, `rollout_accounts`, `enabled`, `admit_new`,
trusted `shapes`, and explicit `legacy`. Credentials stay in environment variables.
Boat remains the default. SPACE needs the server's `space` feature and an admitted
configuration; this change does not enable SPACE in production.
The [registry deployment evidence](../sdk/computers-registry-evidence.json) records
the scoped tests, staging and production checks, and compatibility with the
released clients.

Staging uses its own `oa-computers-staging` service, staging account database,
and `oa-computers-staging-token`. Deploy the appropriate lifecycle service from
current pushed main with `deploy/oa-computers/deploy.sh staging|production COMMIT`.
New builds require current main. Promotion reuses the exact image digest after
main advances. The helper verifies the running commit and image and refuses to
replace a newer build.
The web staging renderer points to that isolated service. Public gating and
account isolation stay the same in staging and production.

The [lifecycle verification record](../sdk/computers-lifecycle-evidence.json)
pins the 2026-10-16 deployment, account-isolation checks, dedicated-account Boat
workflows, and observed timings. Create acceptance and observed readiness are
measured separately.

### Usage and allocation limits

The first accounting increment for
[#11815](https://github.com/OpenAgentsInc/openagents/issues/11815) records
accepted requests, observed lifecycle revisions, meter counters, evidence hashes,
and operator decisions in append-only PostgreSQL events. Each event names the
account, computer, deployment, immutable shape, create operation, boot, turn
generation, and observation time. The core's logical timestamp is separate.
Reads omit command text, output, and credentials. Missing provider seconds,
including SPACE's absent counter, stay `null`; timestamps never fill them in.
Stopped disk storage and shared service costs also stay unknown until measured.

Account and service policies can limit computer count, running vCPUs and
memory, turn duration, stopped retention, and total boot allowance. A boot
reserves its declared CPU allowance before create or resume. Provider overruns
remain possible. Retries reuse the same reservation.
Unknown outcomes survive restart. Stop and delete requests release no capacity;
observed stop releases running capacity, and observed deletion releases the
computer slot. An unresolved boot allowance stays held until an explicit
operator decision. These allowances are distinct from provider usage and the
purchased compute balance. Customer prices and charges remain disabled. The
allowance is not a verified ceiling on runtime or provider cost.

Use `oa-computers-usage` with `OA_COMPUTERS_DATABASE_URL` for operator reads,
limit changes, and retry-safe allowance decisions:

```sh
oa-computers-usage read ACCOUNT [AFTER_SEQUENCE]
oa-computers-usage account-limits ACCOUNT limits.json
oa-computers-usage service-limits limits.json
oa-computers-usage resolve ACCOUNT COMPUTER BOOT MAXIMUM_CPU_MS USED_CPU_MS 'Decision reference'
```

Limit files accept `computer_count`, `running_vcpus`, `running_memory_mib`,
`boot_cpu_ms`, `turn_seconds`, and `stopped_retention_seconds`. The account and
service retention limits can shorten the configured stopped-storage period.
A zero retention limit starts cleanup after an observed stop. An uncertain stop
keeps its allocation until the provider confirms cleanup. Unknown shapes refuse
bounded shape or boot allowances. A policy change cannot lower a limit below committed allocations.
Migration preserves existing allocations as unknown. Older service writers
cannot change computer rows, job leases, or command queues after the upgrade;
use a compatible service build when rolling back. The public client contract
does not change. Account usage reads are available through
[`GET /v1/computer-usage`](../sdk/computers.md). Storage byte quotas,
account-deletion cleanup, service cost measurements, and priced settlement remain
follow-up work on #11815.

The [usage verification record](../sdk/computers-usage-evidence.json) records the
deployed builds, crash tests, dedicated-account lifecycle check, and remaining
implementation scope.

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

Resource reads include `capabilities` from the computer's saved deployment,
including `memory_suspend`, rather than the current default. A missing deployment
returns `null` for these capabilities. The additive `lifecycle_policy` reports
suspend-after-turn, admitted idle/observation bounds, and the absolute bound for
each boot. Observation cannot extend the absolute deadline. Stop and wake remain
explicit intents; idle and absolute stops update the saved intent and survive
API restart.

Memory-preserving drivers name suspend and resume with identities derived from
the saved boot and turn generation. The service records each SPACE request in
PostgreSQL before sending it, then saves its accepted provider operation ID
before waiting. Recovery only reads that saved operation; it does not depend on
how long SPACE keeps retry keys. A lost response before the ID is saved remains
unknown and is never sent again. The service waits for operation completion and
observed state before dispatch, including after an accepted operation's lookup
fails.
A restored guest must also answer a harmless command, report current wall time,
and pass the declared service checks before becoming ready. SPACE refreshes the
guest clock asynchronously; the service waits until its observed time is within
five seconds of the API process. Existing service processes are checked without running their startup commands.

An unknown command result on a memory-preserving computer keeps the active turn
fenced. Follow-ups and stop cannot snapshot that possibly live command or claim
it ended. A legacy command interrupted by API loss reports the same uncertainty
without starting it again. SPACE output observed before a disconnect is bounded
to 256 KiB per stream and saved during execution; recovery keeps those partial
bytes and does not invent an exit result. Explicit deletion removes the provider resource first,
without taking a memory checkpoint, then completes lifecycle cleanup. At the
saved absolute deadline, an uncertain command or lifecycle operation takes this
deletion path too; uncertainty cannot keep a guest running without a time limit.
Identified
commands on supported backends still observe process-group exit after cancellation.
SPACE's public identified-command capability remains unsupported.

API restart preserves the computer and lifecycle request identities. Worker loss
does not authorize creating a replacement computer or restoring a second copy.
Durable cross-worker checkpoint recovery, credential-reference renewal, and full
private-checkpoint cleanup still need their separate SPACE qualification; memory
preservation currently describes same-worker suspend and resume only.

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
own-machine and Pylon drivers, Kubernetes agent-sandbox and production SPACE
support, signed enrollment for cloud computers (see the design's build
order).

The [SPACE integration roadmap](space-integration-roadmap.md) defines the
remaining work and acceptance checks for a supported SPACE backend alongside
Boat.

The [public Computers delivery contract](../sdk/computers.md) and
[SDK issue directory](../sdk/delivery-plan.md#issue-directory) describe the
account-authenticated API/client work. The service-token/account-header protocol
above remains internal; a public caller does not supply its own owner identity.
