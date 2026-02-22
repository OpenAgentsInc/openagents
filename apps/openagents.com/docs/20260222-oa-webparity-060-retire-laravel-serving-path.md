# OA-WEBPARITY-060 Retire Laravel Serving Path and Archive Legacy Lane

Date: 2026-02-22
Status: pass (retirement verifier + archive migration), production serving lane is Rust-only
Issue: OA-WEBPARITY-060

## Deliverables

- Legacy-serving retirement verifier:
  - `apps/openagents.com/service/scripts/verify-laravel-serving-retired.sh`
- Manual verification workflow:
- Active app agent guidance switched to Rust-only:
  - `apps/openagents.com/AGENTS.md`
- Historical Laravel agent guidance archived:
  - `apps/openagents.com/docs/archived/legacy-laravel-deploy/AGENTS.laravel-boost.md`
- Ownership docs updated to classify Laravel tree as archival:
  - `docs/PROJECT_OVERVIEW.md`

## Retirement Invariants Enforced

1. Active deploy entry points are Rust deploy scripts under `apps/openagents.com/service/deploy/`.
2. Top-level deploy wrapper remains a compatibility shim forwarding to Rust deploy lane.
3. Legacy deploy assets remain only under `apps/openagents.com/deploy/archived-laravel/`.
4. Active `apps/openagents.com/AGENTS.md` no longer carries Laravel Boost guidance.

## Verification Executed

```bash
bash -n apps/openagents.com/service/scripts/verify-laravel-serving-retired.sh
apps/openagents.com/service/scripts/verify-laravel-serving-retired.sh
```

Artifact produced:
- `apps/openagents.com/storage/app/legacy-serving-retirement/<timestamp>/summary.json`
