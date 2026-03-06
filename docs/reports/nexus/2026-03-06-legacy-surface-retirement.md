# Nexus Legacy Surface Retirement

Date: March 6, 2026
Issue: `#3050`

## Scope

Retire the old in-memory relay path and the old stateless public Nexus path now that `nexus.openagents.com` is cut over to the durable production VM.

## Repo cleanup

Removed from the runtime crate:

- the old in-memory relay implementation from `apps/nexus-relay/src/lib.rs`
- the deferred managed-groups harness from `apps/nexus-relay/src/managed_groups.rs`

The crate now exposes only the durable runtime path:

- `apps/nexus-relay/src/durable.rs`

## Infra cleanup

Retired with `scripts/deploy/nexus/06-retire-cloud-run-surface.sh`:

- Cloud Run domain mapping for `nexus.openagents.com`
- Cloud Run service `openagents-nexus-relay`
- Cloud Run service `openagents-nexus-control`

## Validation

### Repo/runtime

- `cargo test -p nexus-relay durable_ -- --nocapture`
  - passed

### Infra

- `gcloud run services list --filter='metadata.name:openagents-nexus*'`
  - returned no services
- `gcloud beta run domain-mappings list`
  - no longer included `nexus.openagents.com`

### Live public host

- `curl https://nexus.openagents.com/healthz`
  - returned `relay_backend = durable-upstream`
- `curl https://nexus.openagents.com/api/stats`
  - returned `receipt_persistence_enabled = true`

This confirms the public hostname remains healthy after removing the old Cloud Run surface.

## Conclusion

There is no remaining production-critical path that depends on the old in-memory relay model or the old stateless Cloud Run Nexus surface.
