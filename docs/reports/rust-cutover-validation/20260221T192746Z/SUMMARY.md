# Rust Staging/Prod Validation Matrix

- Timestamp (UTC): 20260221T192746Z
- Overall: failed
- Total checks: 12
- Passed: 4
- Required failures: 7
- Optional failures: 0
- Required skipped: 0
- Optional skipped: 1

## Environment

- STAGING_CONTROL_BASE_URL=https://openagents-web-staging-ezxz4mgdsq-uc.a.run.app
- PROD_CONTROL_BASE_URL=https://openagents-web-ezxz4mgdsq-uc.a.run.app
- STAGING_RUNTIME_BASE_URL=https://openagents-runtime-ezxz4mgdsq-uc.a.run.app
- PROD_RUNTIME_BASE_URL=https://openagents-runtime-ezxz4mgdsq-uc.a.run.app
- GCP_PROJECT=openagentsgemini
- GCP_REGION=us-central1
- CONTROL_SERVICE=openagents-web
- RUNTIME_SERVICE=openagents-runtime
- MIGRATE_JOB=runtime-migrate

## Check Results

| Check | Required | Status | Description | Log |
| --- | --- | --- | --- | --- |
| control-staging-health | required | failed | staging control health/ready endpoints | docs/reports/rust-cutover-validation/20260221T192746Z/logs/control-staging-health.log |
| control-staging-smoke | required | failed | staging control smoke (static host + auth/session/sync token checks when token provided) | docs/reports/rust-cutover-validation/20260221T192746Z/logs/control-staging-smoke.log |
| control-prod-health | required | failed | production control health/ready endpoints | docs/reports/rust-cutover-validation/20260221T192746Z/logs/control-prod-health.log |
| control-prod-smoke | required | failed | production control smoke (static host + auth/session/sync token checks when token provided) | docs/reports/rust-cutover-validation/20260221T192746Z/logs/control-prod-smoke.log |
| runtime-staging-smoke | required | failed | staging runtime health and authority API smoke | docs/reports/rust-cutover-validation/20260221T192746Z/logs/runtime-staging-smoke.log |
| runtime-prod-smoke | required | failed | production runtime health and authority API smoke | docs/reports/rust-cutover-validation/20260221T192746Z/logs/runtime-prod-smoke.log |
| runtime-migration-drift | required | failed | runtime service/migrate-job image + latest execution drift check | docs/reports/rust-cutover-validation/20260221T192746Z/logs/runtime-migration-drift.log |
| khala-contract-tests | required | passed | runtime khala topic/replay contract tests | docs/reports/rust-cutover-validation/20260221T192746Z/logs/khala-contract-tests.log |
| cross-surface-contract-harness | optional | skipped | web/desktop/ios cross-surface contract harness | docs/reports/rust-cutover-validation/20260221T192746Z/logs/cross-surface-contract-harness.log |
| control-error-log-probe | required | passed | control service error log probe (last 10m) | docs/reports/rust-cutover-validation/20260221T192746Z/logs/control-error-log-probe.log |
| runtime-error-log-probe | required | passed | runtime service error log probe (last 10m) | docs/reports/rust-cutover-validation/20260221T192746Z/logs/runtime-error-log-probe.log |
| control-canary-status | required | passed | control service canary status probe | docs/reports/rust-cutover-validation/20260221T192746Z/logs/control-canary-status.log |

## Commands

- control-staging-health: env OPENAGENTS_BASE_URL=https://openagents-web-staging-ezxz4mgdsq-uc.a.run.app /Users/christopherdavid/code/openagents/apps/openagents.com/service/deploy/smoke-health.sh 
- control-staging-smoke: env OPENAGENTS_BASE_URL=https://openagents-web-staging-ezxz4mgdsq-uc.a.run.app OPENAGENTS_CONTROL_ACCESS_TOKEN= /Users/christopherdavid/code/openagents/apps/openagents.com/service/deploy/smoke-control.sh 
- control-prod-health: env OPENAGENTS_BASE_URL=https://openagents-web-ezxz4mgdsq-uc.a.run.app /Users/christopherdavid/code/openagents/apps/openagents.com/service/deploy/smoke-health.sh 
- control-prod-smoke: env OPENAGENTS_BASE_URL=https://openagents-web-ezxz4mgdsq-uc.a.run.app OPENAGENTS_CONTROL_ACCESS_TOKEN= /Users/christopherdavid/code/openagents/apps/openagents.com/service/deploy/smoke-control.sh 
- runtime-staging-smoke: env SMOKE_BASE_URL=https://openagents-runtime-ezxz4mgdsq-uc.a.run.app cargo run --manifest-path /Users/christopherdavid/code/openagents/apps/runtime/Cargo.toml --bin runtime-smoke 
- runtime-prod-smoke: env SMOKE_BASE_URL=https://openagents-runtime-ezxz4mgdsq-uc.a.run.app cargo run --manifest-path /Users/christopherdavid/code/openagents/apps/runtime/Cargo.toml --bin runtime-smoke 
- runtime-migration-drift: env GCP_PROJECT=openagentsgemini GCP_REGION=us-central1 RUNTIME_SERVICE=openagents-runtime MIGRATE_JOB=runtime-migrate /Users/christopherdavid/code/openagents/apps/runtime/deploy/cloudrun/check-migration-drift.sh 
- khala-contract-tests: cargo test --manifest-path /Users/christopherdavid/code/openagents/apps/runtime/Cargo.toml server::tests::khala_topic_messages -- --nocapture 
- cross-surface-contract-harness: RUN_CROSS_SURFACE=0
- control-error-log-probe: gcloud logging read resource.type=cloud_run_revision\ AND\ resource.labels.service_name=openagents-web\ AND\ severity\>=ERROR --project openagentsgemini --freshness=10m --limit=50 --format=json 
- runtime-error-log-probe: gcloud logging read resource.type=cloud_run_revision\ AND\ resource.labels.service_name=openagents-runtime\ AND\ severity\>=ERROR --project openagentsgemini --freshness=10m --limit=50 --format=json 
- control-canary-status: env PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-web /Users/christopherdavid/code/openagents/apps/openagents.com/service/deploy/canary-rollout.sh status 
