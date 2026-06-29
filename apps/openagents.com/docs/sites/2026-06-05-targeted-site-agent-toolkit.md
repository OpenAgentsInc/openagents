# Targeted Site Agent Toolkit

Date: 2026-06-05

Status: implemented for issue #207.

## Purpose

The targeted Site remake/outreach lane needs a private, inspectable contract for
agents before those agents can run user-owned prospecting campaigns. This slice
adds that internal toolkit ledger without exposing a public agent API yet.

The toolkit is deliberately dry-run first. A user or authorized operator can
grant scoped tools to an agent, and every attempted action is recorded with the
grant, scope, cap, suppression, approval, receipt, and result state needed for
later UI, API-key, manifest, and billing integration.

## D1 Ledgers

`targeted_site_agent_toolkit_grants` records:

- unique `id` and `idempotency_key`;
- `campaign_id` and canonical campaign `owner_user_id`;
- public-safe `agent_ref`;
- `scopes_json`;
- `dry_run_default`;
- `spend_cap_cents`;
- `daily_send_cap`;
- optional `suppression_policy_ref`;
- `approval_policy`;
- `status`;
- bounded `metadata_json`;
- lifecycle timestamps.

`targeted_site_agent_toolkit_actions` records:

- unique `id` and `idempotency_key`;
- `grant_id`, `campaign_id`, and public-safe `agent_ref`;
- `action_kind`;
- effective `dry_run`;
- requested cost and send count;
- `suppression_state`;
- `approval_state`;
- `result_state`;
- `receipt_ref`;
- public-safe `reason`;
- bounded `metadata_json`;
- lifecycle timestamps.

## Scopes

Supported grant scopes are:

- `campaign:discover`
- `campaign:capture`
- `campaign:audit`
- `campaign:preview`
- `campaign:outreach:request`
- `campaign:metric:record`
- `campaign:reward:propose`

Action kinds map one-to-one onto these scopes:

- `discover_prospects`
- `capture_site`
- `audit_site`
- `generate_preview`
- `send_outreach_request`
- `record_metric`
- `propose_reward`

## Service Contract

`createTargetedSiteAgentToolkitGrant`:

- requires an active, unarchived campaign;
- enforces owner authority unless `isAdmin` is explicitly true;
- records the canonical campaign owner, not the caller-supplied user when an
  admin creates a grant;
- requires at least one scope;
- defaults to `dryRunDefault: true`;
- defaults approval policy to `auto_dry_run_only`;
- records spend and daily-send caps;
- records idempotently by grant `idempotency_key`;
- rejects raw provider, email, private customer, payment, wallet, token,
  invoice, preimage, and secret-like material in refs or metadata.

`recordTargetedSiteAgentToolkitAction`:

- requires an active grant;
- derives the effective dry-run value from the action input or grant default;
- rejects missing required scopes;
- blocks non-dry-run actions for `auto_dry_run_only` grants;
- blocks requested costs above the grant spend cap;
- blocks requested sends above the same-day send cap;
- blocks suppressed or manual-review suppression states;
- blocks non-dry-run owner/operator approval grants unless the action records
  `approvalState: approved`;
- records idempotently by action `idempotency_key`;
- generates a deterministic receipt ref from action kind and idempotency key
  when the caller does not provide one.

## Projections

`agentToolkitActionContract` returns the machine-readable grant contract:

- grant id;
- campaign id;
- agent ref;
- scopes;
- dry-run default;
- spend cap;
- daily send cap;
- approval policy;
- status.

`publicTargetedSiteAgentToolkitActionProjection` exposes only:

- campaign id;
- action kind;
- dry-run flag;
- approval state;
- result state;
- receipt ref;
- created timestamp.

It does not expose raw metadata, agent private material, internal reasons, or
prospect/customer contact material.

## Boundaries

This slice does not create scoped API keys, public manifests, browser UI,
payment checkout, real outreach sending, or reward payout. It creates the
private D1 and service contract that those surfaces can consume once auth,
payment, abuse, and operator-review controls are complete.
