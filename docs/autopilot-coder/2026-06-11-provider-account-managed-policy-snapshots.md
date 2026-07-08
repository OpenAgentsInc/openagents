# Provider Account Managed Policy Snapshots

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

## Scope

This is the Pack B managed-policy snapshot record for the minimal
provider-account and team-budget decisions needed by current Autopilot
surfaces. It supports #4830 under the #4824 Pack B parent.

This is not a broad enterprise policy UI. The supported slice is typed policy
refs, effective policy snapshots, visible denial reasons, caveat refs, and
attachment refs for runs, leases, work orders, and receipts.

## Policy Inputs

Managed policy snapshots can cite:

- organization policy refs
- team policy refs
- repository policy refs
- user policy refs
- device/local policy refs
- provider policy refs
- budget policy refs
- retention policy refs
- telemetry policy refs

Snapshots must resolve to an `effectivePolicyRef` that is stable enough for a
later audit to explain which policy state governed a run, lease, work order,
receipt, or team-budget decision at evaluation time.

## Decisions

Policy projections may return:

- `allowed`: active policy state, provider allowlisted, user approved when the
  gate is enabled, budget within policy, and retention allowed.
- `denied`: active policy state with typed denial refs for provider,
  approved-user, budget, or retention failures.
- `stale`: policy state is explicitly stale or older than its freshness
  window.
- `unknown`: no reliable policy state exists for the decision.

Typed denial refs are public/agent-readable handles. They must not expose raw
policy internals, credentials, raw prompts, private repo data, raw provider
responses, wallet/payment material, customer-private data, or local paths.

## Attachments

Provider allowlists, provider disallow reason refs, approved-user gates,
budget caveats, retention caveats, and telemetry caveats may be attached to:

- runs
- leases
- work orders
- receipts

These attachments are evidence refs only. They do not override the underlying
credential, security-review, telemetry, retention, or ToS gates.
