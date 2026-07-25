# Forge Git local clone, push, and restore receipt

- Date: 2026-07-25
- Class: verification receipt
- OpenAgents issue: [OpenAgentsInc/openagents#9244](https://github.com/OpenAgentsInc/openagents/issues/9244)
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: local proof passed. Production acceptance is not complete

## Scope

This receipt records a local test of the owned Forge Git service.
The test used stock `git upload-pack` and `git receive-pack`.
It did not use a TypeScript packfile engine or CGI.

The test used two separate repository roots.
It used an in-memory implementation of the GCS blob interface.
Thus, this receipt does not prove Cloud Run, GCE NFS, or GCS operation.
The test used a static tenant and token-scope session.
It did not test live actor bindings or tombstone revocation.

## Result

The test completed these steps:

1. It pushed commit A to an empty bare repository.
2. It cloned commit A through Smart HTTP.
3. It pushed commit B and cloned commit B from a second work area.
4. It completed `git fsck --full --strict`.
5. It found the `filter` capability and the two SHA-1 want capabilities.
6. It completed a partial clone with a promisor remote.
7. It made a native Git bundle backup.
8. It restored the bundle to a second repository root.
9. It verified the restored repository with `git fsck`.
10. It cloned the restored repository and pushed commit C to it.

The machine-readable receipt contains the object IDs and SHA-256 values:

[`2026-07-25-forge-git-local-clone-push-restore.json`](./2026-07-25-forge-git-local-clone-push-restore.json)

## Acceptance limit

This local receipt is not the FORGE-02 acceptance receipt.
The production service needs a POSIX repository store.
The admitted design uses one persistent disk, one GCE NFS host, and one Cloud
Run instance.
The estimated fixed cost is USD 47.90 each month.
Stop before apply if the reviewed estimate can exceed USD 100 each month.
FORGE-04 in issue #9246 owns the live actor-binding and tombstone-revocation
integration.
Keep issue #9244 open until the production clone, push, and isolated restore
drill passes.
