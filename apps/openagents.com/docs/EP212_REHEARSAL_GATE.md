# EP212 Rehearsal Gate (Laravel App)

This document defines the Laravel app gate for EP212 demo readiness.

Canonical runbook and log live in the top-level Lightning docs:

- Runbook: `docs/lightning/runbooks/EP212_LARAVEL_REHEARSAL_GATE.md`
- Log: `docs/lightning/status/20260217-ep212-laravel-rehearsal-gate-log.md`

## Deterministic matrix

Run from `apps/openagents.com`:

```bash
composer test:ep212
composer test:ep212:junit
```

Covered scenarios:

1. paid success
2. cached repeat
3. blocked pre-payment
4. approval lifecycle

Artifact path:

- `output/ep212-rehearsal/ep212-deterministic-junit.xml`

## Local CI hook

Gate is enforced through local hooks/scripts:

- Install hooks: `./scripts/install-git-hooks.sh`
- Changed-files gate: `./scripts/local-ci.sh changed`
- Full gate: `./scripts/local-ci.sh all`

## Recording rule

Do not record EP212 until deterministic + live smoke + UI checklist all pass and are logged in the status file.
