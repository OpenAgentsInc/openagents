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

### Running it

```sh
OA_COMPUTERS_DATABASE_URL='host=127.0.0.1 port=55432 user=postgres dbname=computers' \
OA_COMPUTERS_TOKEN=dev-token OA_COMPUTERS_DRIVER=fake \
cargo run -p oa-computers
```

Settings: `OA_COMPUTERS_DRIVER` (`boat` with `BOAT_API_BASE`/`BOAT_API_KEY`,
or `fake`), `OA_COMPUTERS_LISTEN` (else `0.0.0.0:$PORT`),
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

## Not yet

Warm pool, activity-based suspend beyond the idle bound, billing, the
own-machine and Pylon drivers, Kubernetes agent-sandbox and Firecracker
drivers, signed enrollment for cloud computers (see the design's build
order).
