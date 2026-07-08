# Provider Account Telemetry Privacy Fixtures

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

## Scope

This is the Pack B telemetry/privacy fixture record for provider-account
health, rate-limit, low-credit, cooldown, reset-hint, reconnect, lease
utilization, and provider-routing state.

It supports #4828 under the #4824 Pack B parent and is an input to the #4771
provider-peer closeout path.

## Telemetry Modes

- `aggregate`: public or agent-readable projections may expose metric refs,
  counters, duration values, statuses, provider ids, provider-account classes,
  caveat refs, redaction fixture refs, source refs, and freshness metadata.
- `local_only`: projections may expose metric refs and freshness metadata, but
  must not expose metric details outside the local/debug boundary.
- `off`: projections carry opt-out status and metric refs only, with no metric
  details.

`approved_users_only` is the default sharing policy for aggregate telemetry
until a surface has a narrower policy. `local_only` and `opt_out` must be
preserved in downstream projections rather than silently upgraded.

## Required Redaction Fixtures

Telemetry that mentions account health, rate-limit state, low-credit state,
cooldown state, reset hints, or reconnect state must cite redaction fixture
refs. Missing fixture refs produce typed blocker refs instead of a ready
projection.

Debug and support bundles may reference Pack B state only by redacted bundle
refs. Raw prompts, transcripts, shell output, private repo data, raw provider
responses, provider credentials, OAuth material, and payment/wallet material
must not appear in public or agent-readable telemetry.

## Freshness

Every provider-account telemetry projection must carry:

- `generatedAt`
- `observedAt`
- `staleAt`
- `ageMs`
- `freshness`

Stale telemetry can remain visible as evidence, but it must not be treated as a
fresh readiness or routing signal without a newer projection.
