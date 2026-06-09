# OpenAgents product surface Data Classification Policy v2

Date: 2026-06-06

Status: implemented contract note for issue #329 / `OPENAGENTS-082`.

## Purpose

OpenAgents product surface now has a reusable data-classification policy envelope for orders,
Sites, revisions, workrooms, artifacts, evidence bundles, task packets, agent
API payloads, Forum records, receipt records, payment refs, provider-account
refs, and customer assets.

The implementation extends `workers/api/src/omni-data-classification.ts`.

This is a policy and projection contract. It does not grant runtime authority,
change dispatch, connect providers, export data, or deploy anything by itself.

## Classifications

The classification enum now covers:

- `public`;
- `customer`;
- `team`;
- `operator`;
- `private`;
- `legal_sensitive`;
- `provider_private`;
- `payment_private`;
- `secret_bearing`; and
- `deletion_retention_sensitive`.

The new `deletion_retention_sensitive` class is for records that are safe to
track internally but should not move through normal export paths after a
delete/retention-sensitive state is attached.

## Surfaces

`OmniDataPolicyEnvelope` can classify these surfaces:

- `order`;
- `site`;
- `site_revision`;
- `workroom`;
- `artifact`;
- `evidence_bundle`;
- `task_packet`;
- `agent_api_payload`;
- `forum_topic`;
- `forum_post`;
- `forum_receipt`;
- `forum_payment_ref`;
- `receipt`;
- `payment_ref`;
- `provider_account`; and
- `customer_asset`.

Safe `provider_account` refs are allowed because provider-account objects are
part of the required policy surface. Provider grants, provider tokens, raw
provider payloads, and raw auth state remain unsafe.

## Projection Decisions

`omniDataPolicyProjectionDecision(record, audience)` maps classification and
audience to one of:

- `allow`;
- `redact`;
- `omit`; or
- `deny`.

Public and agent audiences can only directly allow public data. Customer
audiences can directly allow public and customer data. Team and operator
audiences can see progressively broader safe classes. Private/internal
audiences can see every classification.

If a record is not directly allowed but has a redaction policy ref, the
projection can return `redact`. If no redaction path exists, the projection
returns `omit`. Blocked trust or non-private secret-bearing projections return
`deny`.

## Retention And Export

Every policy envelope carries:

- `retentionPolicyRefs`;
- `exportPolicyRefs`;
- `redactionPolicyRefs`;
- `providerEligibilityRefs`; and
- `evidenceRefs`.

`omniDataPolicyExportAllowed(record, audience)` only allows export when the
audience can directly allow the record and the export/retention refs do not
deny export.

Sensitive classes such as `private`, `legal_sensitive`, `provider_private`,
and `payment_private` require explicit safe export refs such as
`export.operator_safe`, `export.team_safe`, `export.customer_safe`, or
`export.redacted`.

`secret_bearing` and `deletion_retention_sensitive` records are never export
allowed through this helper.

## Provider Eligibility

`omniRequiredProviderEligibilityRefs(record)` returns future placement-policy
requirements based on classification:

- public data can run on `provider.eligibility.public`;
- customer-visible data requires `provider.eligibility.customer_visible`;
- team/operator/private data requires `provider.eligibility.reviewed_private`;
- legal-sensitive data requires `provider.eligibility.legal_sensitive`;
- provider-private data requires `provider.eligibility.provider_private`;
- payment-private data requires `provider.eligibility.payment_private`; and
- secret-bearing or deletion/retention-sensitive data requires
  `provider.eligibility.no_external_provider`.

Issue #330 now consumes these refs in
`workers/api/src/provider-placement-policy.ts` for provider allowlist and
placement restrictions.

## Redaction Guard

Policy envelopes reject refs containing:

- private customer emails or customer names/values;
- provider grants, tokens, or raw payloads;
- callback tokens, bearer tokens, OAuth material, API keys, and secrets;
- wallet material, invoices, payment hashes, payment proofs, preimages, and
  payout targets;
- raw runner logs, raw provider payloads, raw prompts, raw emails, raw source
  archives, and raw webhooks;
- private repo refs; and
- raw timestamps.

## Tests

`workers/api/src/omni-data-classification.test.ts` now covers:

- schema/projection decoding;
- allow/redact/omit/deny audience decisions;
- customer, public, team, operator, agent, and private audience behavior;
- export/retention denial behavior;
- explicit sensitive export policy requirements;
- provider-eligibility refs by classification; and
- unsafe ref rejection.
