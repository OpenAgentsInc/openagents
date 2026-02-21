# Deploy Entry Points (Rust-Only Active Lane)

Active web deploy lane is Rust-only:

- Deploy helper: `apps/openagents.com/service/deploy/deploy-production.sh`
- Canary/rollback helper: `apps/openagents.com/service/deploy/canary-rollout.sh`
- Health smoke check: `apps/openagents.com/service/deploy/smoke-health.sh`

Compatibility wrapper:

- `apps/openagents.com/deploy/deploy-production.sh` forwards to the Rust deploy helper.

Legacy Laravel deploy assets were archived for audit-only reference under:

- `apps/openagents.com/deploy/archived-laravel/`

Do not use archived assets for production deploys.
