# Tenant-Isolated Deployment Lane

## Isolation Contract

Each tenant has a dedicated runtime lane with hard separation for:
- Configuration files
- Stateful storage
- Credentials and secret namespace
- Runtime identity references (Nostr + wallet + lane ID)
- Network egress/relay allowlists

No tenant may share storage paths, credential namespaces, runtime identities, or secret scopes.

## Provisioning

Provisioning is modeled by `provision_tenant_environment` in `crates/email-agent/src/tenant_isolation.rs`.

Inputs:
- `tenant_id` (ASCII alnum, `-`, `_`)
- `root_dir`
- allowed egress domains
- relay allowlist

Derived per-tenant layout:
- `${root_dir}/${tenant_id}/config/email-agent.toml`
- `${root_dir}/${tenant_id}/state/email-agent.sqlite`
- `${root_dir}/${tenant_id}/audit/pipeline.log`
- `${root_dir}/${tenant_id}/attachments/`

Derived identity + secret boundaries:
- runtime identity: `runtime:{tenant_id}`
- nostr ref: `nostr/{tenant_id}`
- wallet ref: `wallet/{tenant_id}`
- secret scope: `secret-scope:{tenant_id}`
- credentials namespace: `email-agent/{tenant_id}/`

## Rotation

Secret rotation is modeled by `rotate_tenant_secret_scope`.

Behavior:
- Increments tenant secret scope version deterministically.
- Keeps tenant scope ID stable while rolling version.
- Prevents cross-tenant namespace mutation.

## Teardown

Teardown is modeled by `teardown_tenant_environment`.

Output plan includes:
- Runtime identity references to revoke
- Secret scope ID to revoke
- Storage/audit/attachment paths to wipe

Teardown removes tenant lane from active state only after producing this explicit revocation + wipe plan.

## Verification Gate

`verify_hard_tenant_isolation` enforces that active tenants do not share:
- storage paths
- runtime identity refs
- secret scope IDs
- credentials namespaces

Any collision is reported as a hard isolation violation.
