# Archived Laravel Deploy Lane (Frozen)

Status: frozen by OA-RUST-111 Phase B

This directory is retained only for rollback/cutover contingencies while Rust cutover is in progress.

Default policy:

- Do not run these scripts for normal deployments.
- Use Rust deploy lane in `apps/openagents.com/service/deploy/`.

Unfreeze requirements (explicit override):

1. Set `OA_LEGACY_LARAVEL_UNFREEZE=1`.
2. Set `OA_LEGACY_LARAVEL_CHANGE_TICKET=<approved-ticket-id>`.
3. Use only during approved rollback/cutover windows.

Cloud Build override:

- `apps/openagents.com/deploy/archived-laravel/cloudbuild.yaml` requires:
  - `_OA_LEGACY_LARAVEL_UNFREEZE=1`
  - `_OA_LEGACY_LARAVEL_CHANGE_TICKET=<approved-ticket-id>`

Without these flags, build invocation fails intentionally.
