# Forge OpenAgents Import Runbook

Issue #6793 seeds the public `OpenAgentsInc/openagents` repository into the
Forge canonical git/ref store for dogfood visibility before SU-6 makes GitHub a
mirror.

Canonical dogfood refs:

- Tenant: `tenant.openagents`
- Repository: `repo.openagents.openagents`
- Default branch: `refs/heads/main`
- Upstream refresh source, until SU-6: `OpenAgentsInc/openagents` on GitHub

Refresh the import with an admin-scoped Forge control-plane token:

```sh
curl -fsS https://openagents.com/api/forge/admin/import-openagents \
  -H "authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H "content-type: application/json" \
  -d '{"tenantRef":"tenant.openagents","repositoryRef":"repo.openagents.openagents"}'
```

Then inspect the live canonical refs:

```sh
curl -fsS "https://openagents.com/api/forge/refs?tenantRef=tenant.openagents&repositoryRef=repo.openagents.openagents" \
  -H "authorization: Bearer $OPENAGENTS_FORGE_CONTROL_PLANE_TOKEN" \
  -H "x-openagents-forge-scopes: forge:change:read"
```

The import route is intentionally narrow: any tenant or repository other than
`tenant.openagents` / `repo.openagents.openagents` fails closed. Re-running the
same GitHub `main` tip is idempotent and returns `changed: false`; a later
GitHub `main` tip updates only the canonical `refs/heads/main` row and the tip
object metadata.
