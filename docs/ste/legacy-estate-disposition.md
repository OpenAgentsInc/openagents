# OpenAgents legacy document disposition

- Standard: ASD-STE100 Issue 9
- Policy: `openagents-ste-policy-v2`
- Date: 2026-07-19
- Scope: All governed OpenAgents documents

## Purpose

This document is the STE frame for immutable evidence, generated artifacts, archives, and third-party text.
It also identifies the final review rule for mutable first-party technical text.

The final inventory records each governed path, SHA-256 digest, terminal state, and reason.
Do not use an immutable source-data file as current authority unless a current control document explicitly selects it.

## Source-data categories

Keep source-data bytes without changes.
The source-data state applies to these categories:

- Third-party license, notice, upstream, and vendor text
- Generated artifacts, fixtures, and snapshots
- Formal model and model-configuration source files
- Immutable receipts, evidence, changelogs, audits, analyses, and after-action records
- Archives, backups, transcripts, and reference material
- Historical Sol plans, issue sources, tombstones, and receipts that the Sol manifest classifies as noncurrent

The final inventory is the provenance record for these files.
Its digest binds each classification to exact bytes.
A source-data classification does not make historical content current.

## Mutable first-party text

Internal technical documents are agent-facing records.
They use the controlled agent compact profile when density improves fast and precise agent use.
The profile does not permit semicolons, contractions, ambiguous authority, unsafe terms, or weaker evidence rules.

The final review accepts only the identified structural screening rules.
It does not change commands, identifiers, URLs, issue refs, numeric values, or source-data spans.

## Change control

A changed file has different bytes and invalidates its final inventory entry.
An author must review the complete changed document and regenerate the inventory.
A new governed file must get a profile and a final inventory entry.

Run these commands after the review:

```bash
pnpm run generate:ste-final-inventory
pnpm run generate:ste-ledger
pnpm run check:ste:all
```

The ledger generator rejects a missing path, an extra path, or a stale digest.
Thus, the terminal state cannot silently transfer to different document bytes.
