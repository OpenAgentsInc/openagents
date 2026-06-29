# OTEC Public Proof Closeout

This note documents the OPENAGENTS-063 extension to the public OTEC proof endpoint.

The endpoint remains:

```text
GET /api/public/proof/otec
```

It returns a no-store public-safe projection for the canonical Ben OTEC Site
order. The route existed before this issue; the closeout now exposes more
explicit proof refs while keeping the same narrow public boundary.

## Public Fields

The response includes:

- the customer-safe order summary;
- public Site state and active Site URL only when the deployment is active;
- active/latest Site version IDs and dedicated revision URLs;
- deployment state and deployment URL;
- approved research/source counts without raw Exa payloads;
- compatibility and build-validation summaries;
- public usage receipt refs;
- payment caveats that separate buyer/Site checkout evidence from accepted-work
  settlement evidence;
- public claim-state projections for overall closeout, Site URL, research,
  latest saved version, active deployment, and public receipts; and
- agent referral/instruction/challenge cards.

## Claim Projections

`workers/api/src/public-otec-proof.ts` now uses
`projectPublicClaimRecord(record, 'public')` for OTEC closeout claims. That
means the proof page inherits the OPENAGENTS-059/060 claim rules: planned, modeled,
measured, verified, settled, blocked, and prohibited states must be backed by
the right evidence refs and caveats.

The current public claim projections include:

- `claim_otec_closeout_overall`;
- `claim_otec_site_url`;
- `claim_otec_research`;
- `claim_otec_latest_saved_version`;
- `claim_otec_active_deployment`; and
- `claim_otec_public_receipts`.

These are public projections only. They are not private workroom truth.

## Revision URLs

The proof payload now carries explicit revision URL refs such as:

```text
https://sites.openagents.com/ben-otec/versions/<site_version_id>
```

This lets agents and humans inspect the active or latest review-ready revision
without guessing how Site revisions are addressed.

## Redaction Boundary

The OTEC proof route fails closed with `public_otec_proof_unsafe` if projected
content contains private or secret-shaped material, including:

- private feedback or private workroom refs;
- raw prompts, raw runner payloads, or runner logs;
- provider grants, provider tokens, or provider payload refs;
- API tokens, bearer values, cookies, OAuth material, private keys, or secret
  refs;
- customer private data, including email-shaped refs;
- raw payment hashes, preimages, invoices, or wallet state refs; and
- unreviewed operator-only material.

The scanner allows safety instructions that mention secrets, tokens, or wallet
material generically. It rejects secret-shaped refs and values, not warnings
that tell agents what not to send.

## Verification

Coverage lives in `workers/api/src/public-otec-proof.test.ts` and checks:

- route response shape and Schema decode;
- Site URL refs, revision URL refs, evidence refs, and claim projections;
- public usage receipt refs and payment caveats;
- no-deployment saved-review state;
- planned state before proof records exist; and
- fail-closed behavior for bearer, wallet-state, and raw-payment-shaped data.
