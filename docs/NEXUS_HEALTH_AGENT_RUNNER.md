# Nexus Health Agent Runner

`nexus-health-agent` is the cloud-worker entrypoint for the Autopilot/Forge
health loop. It is intentionally monitor-only in this phase: it observes Nexus,
classifies the result, emits redacted evidence, and writes Forge health
work-orders/events. It does not restart services, mutate GCP state, move funds,
or run recovery actions without a future Forge controller lease path.

## Entry Point

Build or run the worker from the `openagents` repo:

```shell
cargo run -p nexus-control --bin nexus-health-agent -- --dry-run --fake-nexus --pretty
```

The command emits one JSON report. The report includes:

- `snapshot`: deterministic Nexus health snapshot from the existing classifier.
- `evidence_artifacts`: redacted inline `nexus.health.snapshot` evidence plus a
  stable SHA-256 digest.
- `forge_work_order_request`: the planned or submitted Forge
  `POST /v1/health/work-orders` body.
- `forge_event_request`: the planned or submitted Forge
  `POST /v1/health/events` body.
- `forge_writes`: dry-run, fake, or live write results.
- `redaction`: booleans proving secret-shaped keys and strings are absent from
  the emitted report.

## Modes

Use fake Nexus plus dry-run for local parser and report validation:

```shell
cargo run -p nexus-control --bin nexus-health-agent -- --dry-run --fake-nexus --pretty
```

Use live Nexus but no Forge writes when checking public service health from a
local or cloud shell:

```shell
cargo run -p nexus-control --bin nexus-health-agent -- --dry-run --pretty
```

Use fake Forge integration in tests or local harnesses when validating the
write path without a real Forge service:

```shell
cargo run -p nexus-control --bin nexus-health-agent -- \
  --fake-nexus \
  --fake-forge \
  --pretty
```

Use live Forge only from a protected cloud-worker environment with the service
bearer and actor context injected as secrets:

```shell
NEXUS_HEALTH_AGENT_FORGE_BASE_URL="https://forge.internal.example" \
NEXUS_HEALTH_AGENT_FORGE_BEARER_TOKEN="<redacted>" \
NEXUS_HEALTH_AGENT_FORGE_ACTOR_JWT="<redacted>" \
cargo run -p nexus-control --bin nexus-health-agent -- --pretty
```

Do not put those secret values in tracked files, issue comments, or logs.

## Environment

- `NEXUS_HEALTH_AGENT_NEXUS_BASE_URL`: defaults to
  `https://nexus.openagents.com`.
- `NEXUS_HEALTH_AGENT_TIMEOUT_MS`: defaults to `8000`.
- `NEXUS_HEALTH_AGENT_FORGE_BASE_URL`: required outside dry-run/fake-Forge
  mode.
- `NEXUS_HEALTH_AGENT_FORGE_BEARER_TOKEN`: required outside dry-run/fake-Forge
  mode.
- `NEXUS_HEALTH_AGENT_FORGE_ACTOR_JWT`: required outside dry-run/fake-Forge
  mode.
- `NEXUS_HEALTH_AGENT_PROJECT_ID`: defaults to `openagents`.
- `NEXUS_HEALTH_AGENT_ACTOR_ID`: defaults to `nexus-health-agent`.

CLI flags with the same meaning override the non-secret defaults:

```text
--nexus-base-url <url>
--forge-base-url <url>
--timeout-ms <ms>
--project-id <id>
--actor-id <id>
--fake-nexus
--fake-forge
--dry-run
--pretty
--json
```

## Forge Contract

The worker writes the APIs introduced for the health-agent program:

- `POST /v1/health/work-orders`
- `POST /v1/health/events`

The current work-order kind is always `nexus.health.monitor`. The event payload
contains redacted evidence references and classifier facts. If Nexus public
endpoints fail, the worker still produces a Forge event plan or write result
instead of crashing; the event class/resource is derived from the failed
predicate, such as `nexus-cloudflared`, `nexus-treasury-wallet`, or
`nexus-training-dispatcher`.

## Recovery Boundary

Mutating actions are deliberately gated by `HealthAgentActionPlan` validation.
Any action other than `monitor` must provide a non-empty Forge lease id. This
issue does not grant recovery authority; later issues should wire the actual
lease acquisition and approved recovery actions through Forge before enabling
service restarts, GCP mutations, or payout-affecting changes.

## Verification

Focused verification for this runner:

```shell
cargo test -p nexus-control health_agent -- --nocapture
cargo run -p nexus-control --bin nexus-health-agent -- --dry-run --fake-nexus --pretty
cargo check -p nexus-control --bins
git diff --check
```

Expected tests cover:

- fake Nexus plus dry-run report generation.
- fake Forge integration for work-order and event append calls.
- public endpoint failure handling.
- no sensitive-shaped keys or strings in emitted reports.
- lease requirement for future mutating action plans.
