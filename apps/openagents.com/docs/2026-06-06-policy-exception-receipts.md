# Policy Exception Receipts

Date: 2026-06-06

Status: implemented contract note for issue #331 / `OPENAGENTS-084`.

## Purpose

OpenAgents product surface now has an evidence-only policy exception receipt contract for reviewed
bypasses across research, provider placement, access, environment/secret
policy, public proof, payment/L402, email delivery, Forum moderation, Site
deployment, and legal-sensitive rules.

The implementation lives in `workers/api/src/policy-exception-receipts.ts`.

This is not an execution primitive. A policy exception receipt records that an
exception was requested, reviewed, and scoped. It does not deploy, spend,
grant access, send email, mutate source, dispatch runtime, or bypass policy by
itself.

## Families

The current exception families are:

- `research_policy`;
- `provider_placement`;
- `access_control`;
- `environment_secret_policy`;
- `public_proof`;
- `payment_l402`;
- `email_delivery`;
- `forum_moderation`;
- `site_deployment`; and
- `legal_sensitive_rule`.

## Receipt Shape

`OpenAgentsPolicyExceptionReceipt` records:

- exception family;
- review state;
- requested-by ref;
- approved-by ref when approved;
- subject refs;
- scope refs;
- expiration;
- risk refs;
- blocker refs;
- evidence refs;
- created/updated timestamps for internal records; and
- an authority block.

Projections convert timestamps into friendly display labels and never expose
raw ISO timestamps.

## Authority Boundary

The default authority block is `OPENAGENTS_POLICY_EXCEPTION_NO_AUTHORITY`.

It denies:

- access grant;
- deployment;
- email send;
- runtime dispatch;
- source mutation; and
- spend.

`openAgentsPolicyExceptionHasRuntimeAuthority` detects any receipt that tries
to carry authority. Such a receipt cannot apply through
`openAgentsPolicyExceptionAppliesNow`.

## Applicability

`openAgentsPolicyExceptionAppliesNow(receipt, nowIso)` returns true only when:

- review state is `approved`;
- an approved-by ref exists;
- the receipt is not expired;
- the receipt is not overbroad;
- the receipt is evidence-only; and
- no runtime authority is present.

Helpers also detect:

- expired receipts;
- rejected receipts;
- revoked receipts;
- unreviewed receipts; and
- overbroad receipts.

A receipt is overbroad when it has no subject refs, no scope refs, or scope refs
such as `scope.all`, `scope.*`, `scope.wildcard`, or `wildcard`.

## Projection And Redaction

`projectOpenAgentsPolicyException` emits public, customer, team, and operator
projections.

Public/customer/team projections hide requested-by refs, approved-by refs,
operator-only evidence, private scope refs, and private subject refs as
appropriate. Operator projections can show safe internal refs, but never raw
secrets or raw payloads.

The contract rejects:

- raw secrets and token-shaped refs;
- provider grants, provider tokens, and provider payloads;
- wallet material;
- payment proofs, preimages, raw invoices, and payout targets;
- raw emails;
- private repo refs;
- raw runner logs;
- raw source archives; and
- raw timestamps.

## Tests

`workers/api/src/policy-exception-receipts.test.ts` covers:

- schema/projection decoding;
- approved evidence-only applicability;
- expired, rejected, revoked, unreviewed, and overbroad state helpers;
- no-runtime-authority behavior;
- required exception families; and
- public/customer redaction plus unsafe ref rejection.
