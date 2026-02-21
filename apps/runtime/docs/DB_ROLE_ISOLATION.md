# Runtime / Control DB Role Isolation

Status: active
Owner: `owner:runtime`
Issue: OA-RUST-066

## Purpose

Enforce authority-plane boundaries at the database permission layer so cross-plane writes are blocked by default.

This policy applies whether control and runtime use one Postgres instance (separate schemas) or separate Postgres instances.

## Role Matrix

Group roles (no direct login):

- `oa_runtime_owner`
  - Owns `runtime` schema and runtime objects.
  - Used by migration lane only.
- `oa_runtime_rw`
  - Runtime service read/write role for `runtime.*` tables/sequences.
- `oa_khala_ro`
  - Khala replay/projection reader role.
  - Read-only access to runtime sync/projection tables.
- `oa_control_rw`
  - Control-plane schema role (`control.*`).

Required boundaries:

1. `oa_control_rw` has no write privileges in `runtime.*`.
2. `oa_runtime_rw` has no write privileges in `control.*`.
3. `oa_khala_ro` has no write privileges in `runtime.*`.
4. `PUBLIC` has no privileges on runtime schema/tables/sequences.

## Apply Policy

SQL policy file:

- `apps/runtime/deploy/cloudrun/sql/db-role-isolation.sql`

Apply helper:

```bash
DB_URL='postgres://...' \
apps/runtime/deploy/cloudrun/apply-db-role-isolation.sh
```

Optional role overrides:

```bash
DB_URL='postgres://...' \
RUNTIME_OWNER_ROLE=oa_runtime_owner \
RUNTIME_RW_ROLE=oa_runtime_rw \
KHALA_RO_ROLE=oa_khala_ro \
CONTROL_RW_ROLE=oa_control_rw \
apps/runtime/deploy/cloudrun/apply-db-role-isolation.sh
```

Dry-run:

```bash
DB_URL='postgres://...' DRY_RUN=1 \
apps/runtime/deploy/cloudrun/apply-db-role-isolation.sh
```

## Verify Drift

Verification helper:

```bash
DB_URL='postgres://...' \
apps/runtime/deploy/cloudrun/verify-db-role-isolation.sh
```

Checks include:

- required roles exist,
- `runtime` schema owner is runtime owner role,
- control role has zero runtime write grants,
- runtime role has zero control write grants (when `control` schema exists),
- Khala role has zero runtime write grants,
- Khala role has required select access on sync tables.

## Login Role Binding

Bind service login roles to group roles (example):

```sql
GRANT oa_runtime_rw TO runtime_service_login;
GRANT oa_control_rw TO control_service_login;
GRANT oa_khala_ro TO khala_service_login;
```

Keep login credentials out of migration scripts and manage them via secret rotation workflows.

## Rotation / Audit Procedure

1. Rotate service DB credentials in Secret Manager.
2. Redeploy services with updated credentials.
3. Run role drift verification:
   - `apps/runtime/deploy/cloudrun/verify-db-role-isolation.sh`
4. Record verification output in deploy notes.
5. If drift is detected, re-run apply script and repeat verification before traffic promotion.
