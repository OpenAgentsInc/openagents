# Deployment and host operations

For cross-project priorities and dependencies, see the [master roadmap](../roadmap.md).

This section covers the Nostr relay: one Rust binary and one Postgres database.
It does not deploy the Coder task owner, a model provider, the decision gateway,
or a labor marketplace as part of starting the relay.

## Relay operations

| Document | Use it for |
| --- | --- |
| [Local development](runbook-local-dev.md) | Disposable Postgres, loopback relay, authentication, and local checks. |
| [Configuration](configuration.md) | Environment variables, defaults, supported optional roles, and startup refusals. |
| [Database and roles](database.md) | Embedded migrations, admission transactions, visibility, and database ownership. |
| [Debian VPS](runbook-debian-vps.md) | Single-host installation, TLS proxy, service, backup, upgrade, and rollback procedure. |
| [Cloud Run](runbook-cloud-run.md) | Recorded deployment revisions and the operator deployment procedure. |
| [Signed-event import](import-jsonl.md) | Ordered event import, prefix failures, and idempotent replay. |
| [Deployment assets](../../deploy/README.md) | Committed service, proxy, environment, and backup templates. |

Use the pinned compiler from [rust-toolchain.toml](../../rust-toolchain.toml).
The relay's database client currently uses `NoTls`; use the documented local
or protected socket topology. Direct managed-Postgres TLS needs a separate
adapter. Media bytes require persistent storage, so the Cloud Run procedure
leaves media disabled.

## Other services

| Service | Owning guide |
| --- | --- |
| Coder conversation worker or approved executor worker | [Worker deployment](../coder/guides/worker-executor.md) and [service assets](../../deploy/README.md) |
| Durable local Coder task owner | [Portable host packaging](../coder/runtime/portable-host.md) |
| Decision API gateway | [Gateway deployment](../decision-models/service/deployment.md) |
| Free-only labor reference host | [Labor runtime](../coder/runtime/free-labor.md) |
| Local model doors | [Kev](../kev/README.md), [Lev](../lev/README.md), and [Laya](../laya/README.md) |

These services have distinct grants, identities, credentials, persistence, and
acceptance evidence. The portable-host launchd fixture and Linux systemd fixture
do not establish a clean Debian relay installation or a full CoderOS release.

## Evidence and changes

Use [protocol coverage](../protocol/2026-09-26-nip-implementation-coverage.md)
for supported roles and [verification](../verification.md) for checks appropriate
to a change. Live service health, configured features, and migration versions
must be read from the target deployment; a runbook is not current telemetry.
Retain installation and rollback receipts before claiming a new host supported.
