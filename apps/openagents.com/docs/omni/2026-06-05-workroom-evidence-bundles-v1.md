# Omni Workroom Evidence Bundles v1

Date: 2026-06-05

Status: implemented for issue #211.

## Purpose

Omni evidence bundles collect the proof refs for a workroom without embedding
private run logs, raw provider payloads, raw emails, private payment material,
or customer-private data.

Bundles are reusable across Sites, PR-style coding work, adjustments,
existing-project imports, business work, and legal-sensitive work. They are the
typed bridge between workroom state and later acceptance, Mission Briefing,
proof export, and economics records.

## D1 Record

`omni_evidence_bundles` records:

- `id` and unique `idempotency_key`;
- required `workroom_id`;
- `work_kind`;
- `status`;
- legal-sensitive flag;
- `summary_ref`;
- optional source-authority caveat ref;
- typed evidence `entries_json`;
- `public_receipt_ref`;
- bounded metadata;
- lifecycle timestamps.

Supported bundle statuses:

- `draft`
- `ready`
- `redaction_required`
- `superseded`
- `archived`

## Entry Kinds

Evidence entries support:

- `exa_source_card`
- `research_brief`
- `source_commit`
- `generated_source`
- `build_log`
- `screenshot`
- `deployment_url`
- `diff`
- `test_report`
- `email_receipt`
- `receipt`
- `redaction_report`

Each entry carries:

- entry kind;
- ref;
- summary ref;
- source authority;
- visibility;
- redaction state;
- required flag;
- public-safe flag;
- optional caveat ref.

## Projection Split

Public projection exposes only public-safe, public-visible, public-appropriate
entry kinds such as deployment URLs, screenshots, source commits, research
briefs, redaction reports, and receipts.

Customer projection exposes public-safe customer/public entries after redaction
but still hides private-only or blocked evidence.

Operator projection includes all entries and bounded metadata so operators can
debug missing workroom artifacts without leaking raw mechanics to customers or
public pages.

## Guardrails

`createOmniEvidenceBundle`:

- records idempotently by `idempotency_key`;
- requires an existing active workroom;
- requires bundle `workKind` to match the workroom;
- validates all refs as public-safe refs;
- rejects raw provider, run-log, email, payment, wallet, token, invoice,
  preimage, customer-private, and secret-like material in refs or metadata;
- forbids public visibility for non-public-safe entries;
- forbids `private_only` redaction state on public-safe entries;
- requires legal-sensitive customer/public entries to include caveats;
- requires legal-sensitive bundles to include a redaction report.

## Boundaries

This slice does not:

- fetch Exa or build source artifacts;
- send email;
- mutate workroom or order status;
- publish a proof page;
- decide acceptance;
- create settlement or payout claims.

It creates the typed evidence bundle model that later acceptance lifecycle,
Mission Briefing, proof bundle, and economics issues can consume.
