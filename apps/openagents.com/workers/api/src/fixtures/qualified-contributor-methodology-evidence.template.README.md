# Qualified-contributor methodology — evidence document template

Promise: `pylon.consumer_compute_earns_bitcoin_self_serve.v1`
Blocker:  `blocker.product_promises.consumer_compute_self_serve_scale_methodology_missing`

`qualified-contributor-methodology-evidence.template.json` is the **canonical,
public-safe SHAPE TEMPLATE** for the per-run qualified-contributor evidence
document that the documented remaining step for this blocker consumes:

> run `verifyQualifiedContributorMethodologyDocument` against the live run's REAL
> evidence file and cite the `ok:true` / `verdict.conforms === true` result.

It exists because the verifier + parse boundary
(`src/qualified-contributor-methodology.ts`) already define the document SHAPE in
code, but there was no checked-in template an auditor could copy to assemble the
real evidence file, and no test exercising the real **file → parse → verify**
path (every other test builds objects in-memory). This template + its disk-load
test (`qualified-contributor-methodology.test.ts`, the "evidence document
template" suite) close that gap.

## What this is NOT (honesty)

- It is **synthetic**. Every ref is a self-evident placeholder
  (`pylon.example.contributor_a`, `lease.example.…`, `receipt.example.…`). It is
  **not** the live run's evidence and does **not** assert that the real run
  conforms or that any real Bitcoin moved.
- It does **not** clear the blocker or flip any promise state. Clearing the
  blocker still requires dropping in the run's REAL per-contributor evidence,
  citing the `ok:true` / `conforms:true` verdict, plus owner sign-off
  (receipt-first per `proof.claim_upgrade_receipts.v1`).

## Constraints the template honors (so it parses)

The parse boundary enforces a **closed key allowlist** at every level (document,
contributor, settlement receipt) and rejects any extra field — that is how a
leaked raw address / balance / credential is kept out of a published evidence
artifact. So this template carries ONLY the allowed keys; you cannot annotate the
JSON itself with a "this is a template" field (it would be rejected). That framing
lives here instead.
