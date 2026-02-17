# EP212 Laravel Rehearsal Gate Log (2026-02-17)

This file tracks the rehearsal gate state defined in `docs/lightning/runbooks/EP212_LARAVEL_REHEARSAL_GATE.md`.

## Latest Entry

Timestamp: 2026-02-17

Deterministic gate:

- Command: `cd apps/openagents.com && composer test:ep212`
- Result: PASS (4 tests, 31 assertions)
- Command: `cd apps/openagents.com && composer test:ep212:junit`
- Result: PASS (4 tests, 31 assertions)
- Local artifact path: `apps/openagents.com/output/ep212-rehearsal/ep212-deterministic-junit.xml`

CI artifact naming convention:

- `ep212-deterministic-junit-php8.4`
- `ep212-deterministic-junit-php8.5`

Live smoke checklist:

- Status: PENDING
- Runbook section: `EP212_LARAVEL_REHEARSAL_GATE.md` §2
- Expected evidence to capture: chat/API request IDs, `/api/l402/transactions` excerpt, and proof references.

UI checklist:

- Status: PENDING
- Runbook section: `EP212_LARAVEL_REHEARSAL_GATE.md` §3
- Expected evidence to capture: approval intent state, approval action, paid completion, cached repeat, blocked pre-payment, and matching `/l402/*` page data.

Release-gate decision:

- Recording allowed: NO (live + UI rehearsal still pending)
