# Historical STE estate disposition

- Date: 2026-07-19
- Original policy: `openagents-ste-policy-v2`
- Current policy: `openagents-ste-policy-v3`
- Status: superseded scope record

## Current meaning

Policy v2 classified nearly every repository text file as governed. It used
this record to assign source-data or inspected-agent dispositions across the
internal documentation estate.

Policy v3 supersedes that broad scope. STE now applies only to public-facing
documents in the publication roots declared by
[`checker-config.v1.json`](./checker-config.v1.json). Internal strategy,
teardowns, roadmaps, audits, plans, specifications, runbooks, receipts, and
agent working documents do not require an STE frame or profile.

The current final inventory contains only governed public paths. The old
internal classifications survive only in historical receipts and dormant
profile overrides. They do not cause a checker, ledger, inventory, baseline,
or semantic gate to apply to an internal document.

## Current change control

When a public governed file changes, review it and run:

```bash
pnpm run generate:ste-final-inventory
pnpm run generate:ste-ledger
pnpm run check:ste:public
```

Internal document changes do not run these commands unless the same change
also updates a configured public documentation path.
