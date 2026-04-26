# Nexus Health Verification Pack

`nexus-control health verify` emits a machine-readable verification pack for
Nexus health, payout, training-dispatch, and Pylon compatibility work. It is
designed for Forge Evidence and Probe-generated fixes: the report separates
required checks from advisory evidence, redacts secret-shaped fields, and
returns JSON even when the snapshot capture fails.

## Command

Local deterministic check:

```shell
cargo run -p nexus-control --bin nexus-control -- health verify --fake --pretty
```

Live public Nexus check:

```shell
cargo run -p nexus-control --bin nexus-control -- health verify --pretty
```

The verification pack defaults to a `20s` timeout because the treasury status
payload can be large during payout reconciliation. Use `--timeout-ms <ms>` only
when you intentionally need a tighter probe bound.

Attach changed-path hints when using the report as evidence for a code change:

```shell
cargo run -p nexus-control --bin nexus-control -- health verify --pretty \
  --changed-path apps/nexus-control/src/health.rs \
  --changed-path scripts/deploy/nexus/18-deploy-health-runner-job.sh
```

Use `--deploy-dry-run` when the change touches deployment behavior and the
operator needs the report to surface deploy dry-run evidence commands.

## Report Contract

The report includes:

- `required_checks`: blocking pass/fail checks for public Nexus endpoints,
  treasury wallet connectivity, treasury degraded state, payout capability,
  training dispatch smoke, website stats freshness, infra availability, and
  Pylon fleet compatibility projection.
- `advisory_checks`: payout movement evidence, training dispatch smoke,
  launch-health backlog evidence, version-floor/readiness blockers,
  changed-crate test selection, and deploy dry-run guidance.
- `forge_evidence`: an inline-redacted
  `nexus.health.verification_pack` artifact descriptor with a SHA-256 digest.
- `redaction`: booleans proving sensitive-shaped keys and strings are absent
  from the emitted report.

The top-level `status` is:

- `passed` when required checks pass and no advisory check needs action.
- `advisory` when required checks pass but operator evidence remains to attach.
- `failed` when any required check fails.

## Boundary

This command does not mutate Nexus, GCP, treasury state, or Forge. It is a proof
and evidence packaging layer. If live public Nexus reachability fails, restore
public service first and then rerun the verification pack before closing the
incident.

Training launch health can be `bad` because of historical validation backlog,
skipped duplicate-host presence records, or signed-access latency while current
dispatch is still functioning. The required check therefore proves dispatch
smoke from visible online-node, run, window, or closeout activity; launch-health
alerts remain advisory evidence that operators should inspect separately.

## Focused Verification

```shell
cargo test -p nexus-control health_verification -- --nocapture
cargo test -p nexus-control health -- --nocapture
cargo run -p nexus-control --bin nexus-control -- health verify --fake --pretty
cargo check -p nexus-control --bins
git diff --check
```
