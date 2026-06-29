# Audit Export Contracts

Date: 2026-06-06

Status: implemented contract note for issue #333 / `OPENAGENTS-086`.

## Purpose

OpenAgents product surface now has a safe audit-export contract for Sites and Autopilot
fulfillment evidence.

The implementation lives in `workers/api/src/audit-export-contracts.ts`.

This is a contract and projection layer only. It does not add a live export
route, storage bucket, download endpoint, admin button, deploy action, email
send, payment action, or source mutation.

## Export Scopes

`OpenAgentsAuditExportScope` covers the fulfillment records operators need to
review:

- `order`;
- `site`;
- `site_revision`;
- `site_version`;
- `deployment`;
- `workroom`;
- `assignment`;
- `artifact`;
- `evidence_bundle`;
- `email`;
- `billing_payment`;
- `forum_activity`;
- `public_claim`; and
- `receipt`.

## Contracts

The export layer defines:

- `OpenAgentsAuditExportRequest`;
- `OpenAgentsAuditExportItem`;
- `OpenAgentsAuditExportDenial`;
- `OpenAgentsAuditExportBundle`; and
- `OpenAgentsAuditExportBundleProjection`.

Requests carry the audience, requester ref, approved-by ref, requested scope
refs, retention policy refs, export policy refs, caveat refs, created time, and
generated time.

Items carry the source refs, evidence refs, receipt refs, retention refs,
export refs, caveat refs, scope, and an `OmniDataPolicyEnvelope`.

Bundles separate included, omitted, and denied items. Projections expose
friendly generated/created labels instead of raw timestamps, plus included,
omitted, and denied counts.

## Classification And Retention Policy

The export contract consumes `OmniDataPolicyEnvelope` from
`workers/api/src/omni-data-classification.ts`.

An item is included only when:

- its data policy can project directly to the requested audience;
- `omniDataPolicyExportAllowed` returns true; and
- no export or retention ref denies export.

An item is omitted when it is safe to acknowledge but not export for that
audience.

An item is denied when blocked trust, secret-bearing classification,
deletion/retention-sensitive classification, explicit export denial, retention
deletion policy, or projection denial applies.

Sensitive payment, provider, private, and legal-sensitive records require an
explicit safe export policy such as `export.operator_safe` before they can be
included for operator export. Secret-bearing and deletion/retention-sensitive
records are not exportable through this contract.

## Audience Projection

The live audiences are:

- public;
- customer;
- agent;
- team;
- operator; and
- private.

Public and agent exports only include public-safe evidence. Customer exports
can include public and customer-safe evidence. Team exports can include
team-safe operational evidence. Operator/private exports can include broader
safe refs, but still cannot expose raw secrets, raw payment material, provider
grants, provider payloads, private repository material, raw source archives, or
raw timestamps.

Requester refs are visible only to operator/private projections. Approved-by
refs are visible to team/operator/private projections. Public, customer, and
agent projections get counts and generic denial refs without operator identity
details.

## Redaction Guard

The contract rejects refs containing:

- private customer data;
- raw emails;
- API keys, bearer tokens, callback tokens, cookies, OAuth material, and
  secrets;
- provider accounts when unsafe, provider grants, provider tokens, and raw
  provider payloads;
- wallet material, invoices, payment proofs, payment hashes, preimages, payout
  addresses, payout destinations, and payout targets;
- private repo material;
- raw runner logs;
- raw source archives;
- raw prompts, raw webhooks, and raw payloads; and
- raw timestamps.

The shared redaction regression suite now covers this export boundary.

## Tests

`workers/api/src/audit-export-contracts.test.ts` covers:

- schema/projection decoding;
- included, omitted, and denied item grouping;
- customer/public/operator projection behavior;
- explicit payment-private export requirements;
- deletion/retention-sensitive denial;
- generated/created friendly labels; and
- unsafe ref and raw timestamp rejection.

`workers/api/src/redaction-regression.test.ts` now includes audit export in the
shared unsafe fixture sweep.
