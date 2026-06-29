# Signature Package Validation API

Date: 2026-06-06

Status: implemented contract note for issue #345 / `OPENAGENTS-DEV-001`.

## Purpose

OpenAgents product surface now has a read-only developer API for validating submitted signature
package manifests before review.

The implementation lives in:

- `workers/api/src/signature-package-validation.ts`; and
- `workers/api/src/signature-package-validation-routes.ts`.

The endpoint is:

```text
POST /api/developer/signature-packages/validate
```

This route validates package shape and returns deterministic validation output.
It does not install a package, promote runtime behavior, create a public
marketplace listing, deploy anything, or mutate payment state.

## Manifest Model

`SignaturePackageManifest` records:

- package ref;
- version ref;
- display name;
- risk class ref;
- schema refs;
- fixture refs;
- evidence requirement refs;
- receipt requirement refs;
- selector metadata refs;
- json-render binding refs;
- source refs; and
- caveat refs.

The default fixture is `SIGNATURE_PACKAGE_VALIDATION_MANIFEST_FIXTURE`.

## Validation Request

`SignaturePackageValidationRequest` records:

- manifest;
- validation request ref; and
- validation clock ref for friendly time projection.

The API accepts the manifest in a `manifest` property. If
`validationRequestRef` is omitted, the route uses the `Idempotency-Key` header
when present, otherwise derives a deterministic request ref from the package
and version refs.

## Validation Result

`SignaturePackageValidationResult` records:

- validation request ref;
- deterministic validation result ref;
- status: `valid`, `invalid`, or `blocked`;
- blocker refs;
- caveat refs;
- redacted manifest projection;
- schema/fixture/risk/evidence/receipt/selector/json-render presence flags;
- operator diagnostic refs for operator/private audiences; and
- hard false authority flags for install, runtime promotion, public
  marketplace listing, deployment, and payment mutation.

Missing schema, fixture, evidence, receipt, selector metadata, or json-render
binding refs produce validation blockers while still returning `200` with an
`invalid` validation result.

Unsafe request material returns `400` and does not project the package.

## Projection And Redaction

Public and agent projections hide source refs and operator diagnostic refs.

The contract rejects refs containing:

- private package source;
- raw prompts;
- raw package payloads;
- provider payloads, grants, accounts, or tokens;
- private repo refs;
- raw schemas, fixtures, source archives, runner logs, or documents;
- secrets, bearer tokens, OAuth material, cookies, and API keys;
- wallet/payment material; and
- raw timestamps.

Projection times use friendly labels instead of raw timestamps.

## Tests

`workers/api/src/signature-package-validation.test.ts` covers:

- schema/projection decoding;
- valid manifest results;
- invalid manifest blockers;
- deterministic result refs;
- hard false install/promotion/marketplace/deploy/payment authority flags;
- public/agent source redaction;
- unsafe package/source/provider/private repo/material rejection;
- no-store route responses;
- route method denial; and
- invalid route payload handling.

The OpenAgents manifest and OpenAPI route tests assert that
`validate_signature_package` and `validateSignaturePackage` remain
discoverable.
